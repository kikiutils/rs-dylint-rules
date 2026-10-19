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
        .write_sync("[package]\nname=\"derive-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n")?;
    (path!(directory.path()) / "src" / "lib.rs").write_sync(source)?;
    Ok(directory)
}

fn run(directory: &TempDir, fix: bool) -> Result<Output> {
    let mut command = Command::new("cargo");
    command
        .current_dir(directory.path())
        .env("DYLINT_RUSTFLAGS", "-A unused")
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
    command.args(["--", "--lib", "--quiet", "--message-format=json"]);
    if fix {
        command.args(["--allow-no-vcs", "--allow-dirty"]);
    }
    Ok(command.output()?)
}

fn has_lint(output: &Output) -> bool {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .any(|message| message["message"]["code"]["code"] == "kiki_grouped_derive_order")
}

#[test]
fn fixes_builtin_lists_independently_and_resolves_aliases() -> Result<()> {
    let source = "use core::fmt::Debug as FormatDebug;\n#[derive(Copy, Clone)]\n#[derive(Hash, Eq, PartialEq, FormatDebug)]\npub struct Value(u32);\nfn nested() {\n    #[derive(Eq, PartialEq, Debug)]\n    struct Nested;\n}\n";
    let directory = fixture(source)?;
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(has_lint(&output), "{output:?}");
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        "use core::fmt::Debug as FormatDebug;\n#[derive(Clone, Copy)]\n#[derive(FormatDebug, PartialEq, Eq, Hash)]\npub struct Value(u32);\nfn nested() {\n    #[derive(Debug, PartialEq, Eq)]\n    struct Nested;\n}\n"
    );
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(!has_lint(&output), "{output:?}");
    Ok(())
}

#[test]
fn honors_lint_levels_and_inactive_items() -> Result<()> {
    let source = "#![deny(kiki_grouped_derive_order)]\n#[derive(Eq, PartialEq)] pub struct Value;\n";
    let directory = fixture(source)?;
    let output = run(&directory, false)?;
    assert!(!output.status.success());
    assert!(has_lint(&output), "{output:?}");
    let source = "#![allow(kiki_grouped_derive_order)]\n#[derive(Eq, PartialEq)] pub struct Value;\n#[cfg(any())] #[derive(Missing, Eq, PartialEq)] struct Inactive;\n";
    (path!(directory.path()) / "src" / "lib.rs").write_sync(source)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert!(!has_lint(&output), "{output:?}");
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        source
    );
    Ok(())
}

#[test]
fn custom_macro_named_like_builtin_does_not_enable_sorting_its_list() -> Result<()> {
    let source = "#[derive(Eq, custom::Clone, PartialEq)]\npub struct Protected;\n#[derive(Hash, Eq, PartialEq)]\npub struct Plain;\n";
    let directory = fixture(source)?;
    let root = path!(directory.path());
    (&root / "custom" / "src").create_dir_all_sync()?;
    (&root / "custom" / "Cargo.toml")
        .write_sync("[package]\nname=\"custom\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\nproc-macro=true\n")?;
    (&root / "custom" / "src" / "lib.rs").write_sync("extern crate proc_macro;\n#[proc_macro_derive(Clone)] pub fn clone(_: proc_macro::TokenStream) -> proc_macro::TokenStream { proc_macro::TokenStream::new() }\n")?;
    let manifest = &root / "Cargo.toml";
    let text = manifest.read_to_string_sync()?;
    manifest.write_sync(format!("{text}\n[dependencies]\ncustom={{path=\"custom\"}}\n"))?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        (&root / "src" / "lib.rs").read_to_string_sync()?,
        "#[derive(Eq, custom::Clone, PartialEq)]\npub struct Protected;\n#[derive(PartialEq, Eq, Hash)]\npub struct Plain;\n"
    );
    Ok(())
}

#[test]
fn all_builtin_derives_preserve_behavior_and_raw_source_offsets() -> Result<()> {
    let source = "const LABEL: &str = \"型別\";\r\n#[derive(Hash, Ord, Eq, Copy, PartialOrd, Default, Clone, Debug, PartialEq)]\r\npub struct Value(u32);\r\n#[test] fn behavior() { let value = Value::default(); assert_eq!(value, value.clone()); assert!(value < Value(1)); assert_eq!(format!(\"{value:?}\"), \"Value(0)\"); use std::hash::{Hash, Hasher}; let mut hasher = std::collections::hash_map::DefaultHasher::new(); value.hash(&mut hasher); assert_ne!(hasher.finish(), 0); }\r\n";
    let directory = fixture(source)?;
    for fix in [false, true] {
        let output = run(&directory, fix)?;
        assert!(output.status.success(), "{output:?}");
        let behavior = Command::new("cargo")
            .current_dir(directory.path())
            .args(["+nightly-2026-10-01", "test", "--lib", "--quiet"])
            .output()?;
        assert!(behavior.status.success(), "{behavior:?}");
    }
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        source.replace(
            "Hash, Ord, Eq, Copy, PartialOrd, Default, Clone, Debug, PartialEq",
            "Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash"
        )
    );
    Ok(())
}

#[test]
fn shared_source_module_instances_keep_their_own_lint_levels() -> Result<()> {
    let directory = fixture(
        "#[allow(kiki_grouped_derive_order)]\n#[path=\"shared.rs\"] mod permitted;\n#[deny(kiki_grouped_derive_order)]\n#[path=\"shared.rs\"] mod forbidden;\n",
    )?;
    (path!(directory.path()) / "src" / "shared.rs").write_sync("#[derive(Eq, PartialEq)]\npub struct Value;\n")?;
    let output = run(&directory, false)?;
    assert!(!output.status.success());
    assert!(has_lint(&output), "{output:?}");
    Ok(())
}
