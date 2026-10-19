use std::process::{
    Command,
    Output,
};

use anyhow::Result;
use pathkit::{
    SyncFsOps,
    path,
};
use tempfile::TempDir;

fn fixture(source: &str) -> Result<TempDir> {
    let directory = tempfile::tempdir()?;
    (path!(directory.path()) / "src").create_dir_sync()?;
    (path!(directory.path()) / "Cargo.toml")
        .write_sync("[package]\nname=\"lint-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n")?;
    (path!(directory.path()) / "src" / "lib.rs").write_sync(source)?;
    (path!(directory.path()) / "src" / "child.rs").write_sync("pub struct Value;\n")?;
    Ok(directory)
}

fn run(directory: &TempDir, fix: bool) -> Result<Output> {
    let mut command = Command::new("cargo");
    command
        .env("DYLINT_RUSTFLAGS", "-A unused -A kiki_import_path_prefix")
        .current_dir(directory.path())
        .args([
            "+nightly-2026-10-01",
            "dylint",
            "--path",
            env!("CARGO_MANIFEST_DIR"),
            "--fail-on-no-libraries",
        ]);
    if fix {
        command.arg("--fix");
    }
    command.args(["--", "--lib", "--message-format=json", "--quiet"]);
    if fix {
        command.args(["--allow-no-vcs", "--allow-dirty"]);
    }
    Ok(command.output()?)
}

fn diagnostics(output: &Output) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| message["message"]["code"]["code"] == "kiki_import_module_order")
        .collect()
}

#[test]
fn native_diagnostics_offer_fix_and_dylint_applies_it() -> Result<()> {
    let source = "use crate::child::Value;\nmod child;\nuse std::fmt;\n";
    let directory = fixture(source)?;
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let messages = diagnostics(&output);
    assert!(!messages.is_empty(), "{output:?}");
    assert!(messages.iter().any(|message| {
        message["message"]["children"].as_array().is_some_and(|children| {
            children.iter().any(|child| {
                child["spans"].as_array().is_some_and(|spans| {
                    spans
                        .iter()
                        .any(|span| span["suggestion_applicability"] == "MachineApplicable")
                })
            })
        })
    }));
    let file = path!(directory.path()) / "src" / "lib.rs";
    assert_eq!(file.read_to_string_sync()?, source);
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        file.read_to_string_sync()?,
        "use std::fmt;\n\nmod child;\n\nuse crate::child::Value;\n"
    );
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(diagnostics(&output).is_empty(), "{output:?}");
    Ok(())
}

#[test]
fn comment_barrier_is_diagnostic_only() -> Result<()> {
    let source = "mod child;\nuse crate::child::Value;\n// keep attached\nuse std::fmt;\n";
    let directory = fixture(source)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert!(!diagnostics(&output).is_empty(), "{output:?}");
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        source
    );
    Ok(())
}

#[test]
fn lint_levels_are_honored() -> Result<()> {
    let directory =
        fixture("#![deny(kiki_import_module_order)]\nuse crate::child::Value;\nmod child;\nuse std::fmt;\n")?;
    let output = run(&directory, false)?;
    assert!(!output.status.success());
    assert!(!diagnostics(&output).is_empty(), "{output:?}");
    let source = "#![allow(kiki_import_module_order, unused)]\nuse crate::child::Value;\nmod child;\nuse std::fmt;\n";
    (path!(directory.path()) / "src" / "lib.rs").write_sync(source)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert!(diagnostics(&output).is_empty(), "{output:?}");
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        source
    );
    Ok(())
}

#[test]
fn fixes_nested_scopes_and_ignores_inactive_code() -> Result<()> {
    let source =
        "mod child;\nfn run() {\n    use crate::child::Value;\n    use std::fmt;\n}\n#[cfg(any())] mod missing;\n";
    let directory = fixture(source)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    let fixed = (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?;
    assert!(
        fixed
            .find("use std::fmt")
            .is_some_and(|index| index < fixed.find("use crate::child").unwrap_or(0))
    );
    assert!(fixed.contains("#[cfg(any())] mod missing;"));
    Ok(())
}

#[test]
fn visible_imports_are_resolved_reordered_and_keep_their_visibility() -> Result<()> {
    let source = "pub use crate::child::Value as PublicValue;\npub(crate) use crate::child::Value as CrateValue;\npub(self) use crate::child::Value as PrivateValue;\nmod child;\npub use std::fmt::Debug;\npub(crate) use std::fmt::Display;\nmod nested {\n    pub(super) use crate::child::Value as ParentValue;\n    pub(in crate::nested) use crate::child::Value as NestedValue;\n    pub use std::fmt::Write;\n}\n";
    let directory = fixture(source)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    let fixed = (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?;
    assert_eq!(
        fixed,
        "pub use std::fmt::Debug;\npub(crate) use std::fmt::Display;\n\nmod child;\n\npub use crate::child::Value as PublicValue;\npub(crate) use crate::child::Value as CrateValue;\npub(self) use crate::child::Value as PrivateValue;\nmod nested {\n    pub use std::fmt::Write;\n\n    pub(super) use crate::child::Value as ParentValue;\n    pub(in crate::nested) use crate::child::Value as NestedValue;\n}\n"
    );
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(diagnostics(&output).is_empty(), "{output:?}");
    Ok(())
}
