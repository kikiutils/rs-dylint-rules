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
    support::check_without_timers(&directory)
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

    support::check_without_timers(&directory)
}

#[test]
fn full_paths_in_nested_cfg_statements_remain_unchanged_despite_existing_imports() -> Result<()> {
    let source = r#"#[cfg(feature = "timers")]
pub fn run() {
    use tokio::time::sleep;
    #[cfg(feature = "timers")]
    let delay = tokio::time::sleep(std::time::Duration::ZERO);
}
"#;

    let directory = support::fixture(source, &support::timers_feature())?;
    support::clean(&directory)?;
    support::check_without_timers(&directory)
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
