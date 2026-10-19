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
        .write_sync("[package]\nname=\"api-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n")?;
    (path!(directory.path()) / "src" / "lib.rs").write_sync(source)?;
    let manifest = path!(directory.path()) / "Cargo.toml";
    let text = manifest.read_to_string_sync()?;
    manifest.write_sync(format!("{text}\n[dependencies]\nanyhow=\"=1.0.104\"\n"))?;
    Ok(directory)
}

fn run(directory: &TempDir, fix: bool) -> Result<Output> {
    let mut command = Command::new("cargo");
    command
        .env(
            "DYLINT_RUSTFLAGS",
            "-A unused -A kiki_import_module_order -A kiki_import_path_prefix",
        )
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
        .filter(|message| message["message"]["code"]["code"] == "kiki_prefer_imported_apis")
        .collect()
}

#[test]
fn fixes_types_associated_functions_and_macros_in_the_nearest_scope() -> Result<()> {
    let source = r#"use anyhow::Result as AnyhowResult;
pub fn run(valid: bool) -> anyhow::Result<()> {
    let _: Option<anyhow::Error> = None;
    let _ = anyhow::Error::msg("example");
    anyhow::ensure!(valid, "invalid");
    if !valid {anyhow::bail!("failed"); }
    let _ = || {anyhow::anyhow!("closure") };
    Ok(())
}
#[test] fn behavior() { assert!(run(true).is_ok()); assert!(run(false).is_err()); }
"#;
    let directory = fixture(source)?;
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(diagnostics(&output).len() >= 5, "{output:?}");
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    let fixed = (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?;
    assert!(fixed.contains("-> AnyhowResult<()>"), "{fixed}");
    assert!(fixed.contains("use anyhow::Error as AnyhowError;"), "{fixed}");
    assert!(fixed.contains("AnyhowError::msg("), "{fixed}");
    assert!(fixed.contains("use anyhow::ensure;"), "{fixed}");
    assert!(fixed.contains("ensure!(valid"), "{fixed}");
    assert!(fixed.contains("use anyhow::bail;"), "{fixed}");
    assert!(fixed.contains("use anyhow::anyhow;"), "{fixed}");
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(diagnostics(&output).is_empty(), "{output:?}");
    let output = Command::new("cargo")
        .current_dir(directory.path())
        .args(["+nightly-2026-10-01", "test", "--lib", "--quiet"])
        .output()?;
    assert!(output.status.success(), "{output:?}");
    Ok(())
}

#[test]
fn aliases_globs_and_public_reexports_are_diagnostic_only() -> Result<()> {
    let source = r#"extern crate anyhow as legacy;
pub use anyhow as ah;
pub use anyhow::bail as fail;
use anyhow::Result as AppResult;
use anyhow::Error;
use anyhow::Result as Result;
use anyhow::ensure as ensure;
mod wildcard { use anyhow::*; }
mod late { fn before() -> AnyhowResult<()> { Ok(()) } use anyhow::Result as AnyhowResult; }
fn run() -> AppResult<()> { let _: Option<Error> = None; fail!("failed"); }
"#;
    let directory = fixture(source)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        source
    );
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    let reports = diagnostics(&output);
    for line in 1..=8 {
        assert!(
            reports
                .iter()
                .any(|report| report["message"]["spans"][0]["line_start"] == line),
            "{reports:?}"
        );
    }
    Ok(())
}

#[test]
fn prescribed_aliases_reuse_imports_and_conflicts_do_not_get_fixes() -> Result<()> {
    let source = r#"macro_rules! bail { ($($tokens:tt)*) => { return Ok(()) }; }
use anyhow::{Error as AnyhowError, Result as AnyhowResult, anyhow, bail, ensure};
fn valid(flag: bool) -> anyhow::Result<()> {
    let _ = anyhow::Error::msg("example");
    anyhow::ensure!(flag, "bad");
    if !flag { anyhow::bail!("bad"); }
    let _ = anyhow::anyhow!("example");
    Ok(())
}
mod inherited { pub fn run() -> anyhow::Result<()> { anyhow::bail!("expected"); } }
mod conflict {
    type Result<T> = Option<T>;
    fn run() -> anyhow::Result<()> { Ok(()) }
    fn local() {
        struct AnyhowError;
        let _: Option<anyhow::Error> = None;
    }
}
"#;
    let directory = fixture(source)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    let fixed = (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?;
    assert!(fixed.contains("fn valid(flag: bool) -> AnyhowResult<()>"), "{fixed}");
    assert!(fixed.contains("let _ = AnyhowError::msg"), "{fixed}");
    assert!(fixed.contains("fn run() -> anyhow::Result<()>"), "{fixed}");
    assert!(fixed.contains("Option<anyhow::Error>"), "{fixed}");
    assert_eq!(fixed.matches("use anyhow::{").count(), 1, "{fixed}");
    assert!(fixed.contains("anyhow::bail!(\"expected\")"), "{fixed}");
    (path!(directory.path()) / "src" / "lib.rs").write_sync(format!(
        "{fixed}\n#[test] fn behavior() {{ assert!(inherited::run().is_err()); }}\n"
    ))?;
    let output = Command::new("cargo")
        .current_dir(directory.path())
        .args(["+nightly-2026-10-01", "test", "--lib", "--quiet"])
        .output()?;
    assert!(output.status.success(), "{output:?}");
    Ok(())
}

#[test]
fn renamed_dependency_local_lookalikes_and_macro_inputs() -> Result<()> {
    let directory = fixture(
        r#"use errors::Result;
mod local { pub type Result<T> = core::result::Result<T, ()>; pub struct Error; }
fn other() -> local::Result<()> { let _: Option<local::Error> = None; Ok(()) }
fn run() -> errors::Result<()> { errors::bail!("bad"); }
mod expansion {
use errors::bail;
macro_rules! passthrough { ($($tokens:tt)*) => { $($tokens)* } }
use passthrough as pass;
self::pass! { fn generated() -> errors::Result<()> { errors::bail!("bad"); } }
}
"#,
    )?;
    let manifest = path!(directory.path()) / "Cargo.toml";
    let text = manifest.read_to_string_sync()?.replace(
        "anyhow=\"=1.0.104\"",
        "errors={package=\"anyhow\",version=\"=1.0.104\"}",
    );
    manifest.write_sync(text)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    let fixed = (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?;
    assert!(fixed.contains("use errors::Result;"), "{fixed}");
    assert!(fixed.contains("use errors::bail;"), "{fixed}");
    assert!(fixed.contains("fn run() -> Result<()>"), "{fixed}");
    assert!(fixed.contains("fn other() -> local::Result<()>"), "{fixed}");
    assert!(fixed.contains("fn generated() -> errors::Result<()>"), "{fixed}");
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(diagnostics(&output).is_empty(), "{output:?}");
    Ok(())
}

#[test]
fn lint_levels_and_attributes_preserve_diagnostic_boundaries() -> Result<()> {
    let directory = fixture(
        r"#[allow(kiki_prefer_imported_apis)]
fn allowed() -> anyhow::Result<()> { Ok(()) }
#[deny(kiki_prefer_imported_apis)]
fn denied() -> anyhow::Result<()> { Ok(()) }
#[cfg(any())] fn inactive() -> missing::Result<()> { unreachable!() }
",
    )?;
    let output = run(&directory, false)?;
    assert!(!output.status.success(), "{output:?}");
    let messages = diagnostics(&output);
    assert_eq!(messages.len(), 1, "{output:?}");
    assert_eq!(messages[0]["message"]["level"], "error");
    Ok(())
}

#[test]
fn inserts_default_result_import_and_skips_standalone_include_fragments() -> Result<()> {
    let directory = fixture(
        r#"mod plain { pub fn run() -> anyhow::Result<()> { Ok(()) } }
fn expression() { let _ = include!("expression.rs"); }
include!("items.rs");
"#,
    )?;
    let root = path!(directory.path()) / "src";
    let expression = &root / "expression.rs";
    let items = &root / "items.rs";
    expression.write_sync("anyhow::anyhow!(\"included\")")?;
    items.write_sync("fn included() -> anyhow::Result<()> { anyhow::bail!(\"included\"); }\n")?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    let fixed = (&root / "lib.rs").read_to_string_sync()?;
    assert!(fixed.contains("use anyhow::Result;"), "{fixed}");
    assert!(fixed.contains("pub fn run() -> Result<()>"), "{fixed}");
    assert_eq!(expression.read_to_string_sync()?, "anyhow::anyhow!(\"included\")");
    assert_eq!(
        items.read_to_string_sync()?,
        "fn included() -> anyhow::Result<()> { anyhow::bail!(\"included\"); }\n"
    );
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(diagnostics(&output).is_empty(), "{output:?}");
    Ok(())
}

#[test]
fn glob_checks_only_bindings_visible_from_the_importing_module() -> Result<()> {
    let source = r"#![deny(kiki_prefer_imported_apis)]
#[allow(kiki_prefer_imported_apis)]
mod hidden { use anyhow::Error; }
use hidden::*;
";
    let directory = fixture(source)?;
    let file = path!(directory.path()) / "src" / "lib.rs";
    for visibility in [
        "",
        "pub(self) ",
        "pub(in crate::hidden) ",
        "pub(super) ",
        "pub(crate) ",
        "pub ",
    ] {
        file.write_sync(source.replace("use anyhow::Error", &format!("{visibility}use anyhow::Error")))?;
        let output = run(&directory, false)?;
        let accessible = matches!(visibility, "pub(super) " | "pub(crate) " | "pub ");
        assert_eq!(output.status.success(), !accessible, "{visibility}: {output:?}");
        let reports = diagnostics(&output);
        assert_eq!(reports.is_empty(), !accessible, "{visibility}: {output:?}");
        if accessible {
            assert_eq!(reports[0]["message"]["spans"][0]["line_start"], 4);
        }
    }
    Ok(())
}

#[test]
fn builtin_attributes_do_not_block_closure_macro_fixes() -> Result<()> {
    let source = r#"#![allow(dead_code)]
use anyhow::{Result, bail};
mod child {}
struct Context {
    #[cfg(unix)] field: (),
}
#[allow(dead_code)]
fn unrelated() {}
#[must_use]
fn context() -> Result<()> {
    #![allow(unused_variables)]
    None.ok_or_else(|| anyhow::anyhow!("runtime context is not initialized"))
}
"#;
    let directory = fixture(source)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    let fixed = (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?;
    assert!(fixed.contains("use anyhow::{Result, bail, anyhow};"), "{fixed}");
    assert!(!fixed.contains("use anyhow::anyhow;"), "{fixed}");
    assert!(
        fixed.contains("|| anyhow!(\"runtime context is not initialized\")"),
        "{fixed}"
    );
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(diagnostics(&output).is_empty(), "{output:?}");

    let source = r#"#[cfg_attr(all(), inline)]
fn protected() -> anyhow::Result<()> {
    None.ok_or_else(|| anyhow::anyhow!("keep attribute boundary"))
}
"#;
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
fn module_merges_respect_cfg_boundaries_and_inline_modules() -> Result<()> {
    let source = r#"mod conditional_fn {
    use anyhow::{Result, bail};
    #[cfg(all())]
    fn run() -> Result<()> { Err(anyhow::anyhow!("conditional function")) }
}
mod conditional_block {
    use anyhow::{Result, bail};
    fn run() -> Result<()> {
        #[cfg(all())]
        { return Err(anyhow::anyhow!("conditional block")); }
        #[cfg(any())]
        { missing::unresolved!(); }
    }
}
#[cfg(all())]
mod inline_existing {
    use anyhow::{Result, bail,};
    fn first() -> Result<()> { Err(anyhow::anyhow!("first")) }
    fn second() -> Result<()> { Err(anyhow::anyhow!("second")) }
}
mod inline_missing {
    fn run() { let _ = anyhow::anyhow!("local"); }
}
mod conditional_missing {
    #[cfg(all())]
    fn run() { let _ = anyhow::anyhow!("conditional local"); }
}
"#;
    let directory = fixture(source)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    let fixed = (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?;
    assert_eq!(fixed.matches("use anyhow::{Result, bail};").count(), 2, "{fixed}");
    assert_eq!(
        fixed.matches("use anyhow::{Result, bail, anyhow,};").count(),
        1,
        "{fixed}"
    );
    assert_eq!(fixed.matches("use anyhow::anyhow;").count(), 4, "{fixed}");
    assert!(!fixed.contains("anyhow::anyhow!("), "{fixed}");
    assert!(
        fixed.contains("fn run() -> Result<()> {\nuse anyhow::anyhow;"),
        "{fixed}"
    );
    assert!(
        fixed.contains("{\nuse anyhow::anyhow;\n return Err(anyhow!(\"conditional block\")"),
        "{fixed}"
    );
    assert!(fixed.contains("missing::unresolved!()"), "{fixed}");
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(diagnostics(&output).is_empty(), "{output:?}");
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        fixed
    );
    Ok(())
}
