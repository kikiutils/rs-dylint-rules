use anyhow::Result;

use super::support;

#[test]
fn cfg_statements_without_blocks_fall_back_to_compliant_full_paths() -> Result<()> {
    let source = r#"use std::process::id;
pub fn current() -> u32 {
    #[cfg(all())]
    let pid = id();
    pid
}
pub fn run() {
    #[cfg(feature = "timers")]
    let delay = tokio::time::sleep(std::time::Duration::ZERO);
}
"#;

    let (directory, fixed) = support::fixed(source, &support::timers_feature())?;
    assert_eq!(fixed, source.replace("let pid = id()", "let pid = std::process::id()"));
    support::check_without_default_features(&directory)
}

#[test]
fn keeps_new_imports_inside_conditional_initializer_blocks() -> Result<()> {
    let source = r#"pub fn run() {
    #[cfg(feature = "timers")]
    let delay = { tokio::time::sleep(std::time::Duration::ZERO) };
}
"#;

    let (directory, fixed) = support::fixed(source, &support::timers_feature())?;
    assert_eq!(
        fixed,
        source.replace("{ tokio::time::sleep", "{use tokio::time::sleep;\n sleep")
    );

    support::check_without_default_features(&directory)
}

#[test]
fn reuses_existing_imports_in_nested_cfg_statements() -> Result<()> {
    let source = r#"#[cfg(feature = "timers")]
pub fn run() {
    use tokio::time::sleep;
    #[cfg(feature = "timers")]
    let delay = tokio::time::sleep(std::time::Duration::ZERO);
}
"#;

    let (directory, fixed) = support::fixed(source, &support::timers_feature())?;
    assert_eq!(
        fixed,
        source.replace("let delay = tokio::time::sleep", "let delay = sleep")
    );

    support::check_without_default_features(&directory)
}

#[test]
fn keeps_new_imports_inside_conditional_function_boundaries() -> Result<()> {
    let source = r#"#[cfg(feature = "enabled")]
pub fn error() -> anyhow::Error { anyhow::anyhow!("conditional") }
"#;

    let dependencies = format!("{}\n[features]\ndefault=[\"enabled\"]\nenabled=[]", support::anyhow());
    let (_, fixed) = support::fixed(source, &dependencies)?;
    assert_eq!(
        fixed,
        source.replace("{ anyhow::anyhow!", "{use anyhow::anyhow;\n anyhow!")
    );

    Ok(())
}

#[test]
fn ignores_cfg_disabled_items_and_unloaded_external_modules() -> Result<()> {
    support::accepted(
        r"#[cfg(any())]
mod missing;
#[cfg(any())]
pub async fn unused() { tokio::time::sleep(std::time::Duration::ZERO).await; }
pub fn active() {}
",
        "",
    )
}

#[test]
fn reuses_module_imports_in_conditional_signatures_and_statements() -> Result<()> {
    let source = r"use std::{io, process};
use anyhow::Result;
#[cfg(all())]
pub fn result() -> std::io::Result<()> { Ok(()) }
#[cfg(all())]
pub trait Operations {
    fn run() -> std::io::Result<()>;
}
pub struct Runner;
impl Operations for Runner {
    #[cfg(all())]
    fn run() -> std::io::Result<()> { Ok(()) }
}
#[cfg(all())]
pub fn other() -> anyhow::Result<()> { Ok(()) }
pub fn pid() -> u32 {
    #[cfg(all())]
    let value = std::process::id();
    value
}
";

    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(
        fixed,
        source
            .replace("std::io::Result", "io::Result")
            .replace("other() -> anyhow::Result", "other() -> Result")
            .replace("let value = std::process::id()", "let value = process::id()")
    );

    Ok(())
}

#[test]
fn conditional_reuse_preserves_shadowing_and_missing_import_fallbacks() -> Result<()> {
    let source = r"use std::io;
#[cfg(all())]
pub fn shadowed<io>(value: io) -> std::io::Result<io> { Ok(value) }
#[cfg(all())]
pub fn missing() -> std::fmt::Result { Ok(()) }
pub fn pid() -> u32 {
    #[cfg(all())]
    let value = std::process::id();
    value
}
";

    support::unchanged(source, "")
}

#[test]
fn does_not_reuse_conditional_imports() -> Result<()> {
    let source = r"#[cfg(all())]
use std::io;
pub fn read() -> std::io::Result<()> { Ok(()) }
pub fn local() {
    #[cfg(all())]
    use std::io;
    let _: std::io::Result<()> = Ok(());
}
";

    support::unchanged(source, "")?;

    let source = r#"#[cfg(feature = "enabled")]
use std::*;
#[cfg(all())]
pub fn read() -> std::io::Result<()> { Ok(()) }
"#;

    support::unchanged(source, "[features]\ndefault=[\"enabled\"]\nenabled=[]")
}

#[test]
fn conditional_globs_allow_new_explicit_imports() -> Result<()> {
    let source = r#"#[cfg(feature = "enabled")]
use std::{fmt::*, *};
pub fn read() -> std::io::Result<()> { Ok(()) }
"#;

    let features = "[features]\ndefault=[\"enabled\"]\nenabled=[]";
    let (directory, fixed) = support::fixed(source, features)?;
    assert_eq!(
        fixed,
        format!("use std::io;\n{}", source.replace("std::io::Result", "io::Result"))
    );

    support::check_without_default_features(&directory)
}
