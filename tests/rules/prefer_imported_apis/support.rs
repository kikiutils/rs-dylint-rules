use anyhow::Result;
use tempfile::TempDir;

use crate::support;
pub(super) use crate::support::behavior;

pub(super) const LINT: &str = "kiki_prefer_imported_apis";
pub(super) const TOKIO_VERSION: &str = "=1.54.1";

pub(super) fn anyhow() -> String {
    format!("anyhow=\"{}\"", support::ANYHOW_VERSION)
}

pub(super) fn tokio() -> String {
    format!("tokio={{version=\"{TOKIO_VERSION}\",features=[\"rt\",\"sync\",\"time\",\"process\"]}}")
}

pub(super) fn timers_feature() -> String {
    format!(
        r#"tokio={{version="{TOKIO_VERSION}",optional=true,features=["time"]}}

[features]
default=["timers"]
timers=["dep:tokio"]"#
    )
}

pub(super) fn check_without_timers(directory: &TempDir) -> Result<()> {
    let output = support::cargo(directory)
        .args([
            "check",
            "--lib",
            "--no-default-features",
            "--message-format=short",
            "--quiet",
        ])
        .output()?;

    assert!(output.status.success(), "{output:?}");
    Ok(())
}

pub(super) fn fixture(source: &str, dependencies: &str) -> Result<TempDir> {
    let directory = support::fixture(source)?;
    if !dependencies.is_empty() {
        support::dependency(&directory, dependencies)?;
    }

    Ok(directory)
}

pub(super) fn accepted(source: &str, dependencies: &str) -> Result<()> {
    let directory = fixture(source, dependencies)?;
    clean(&directory)
}

pub(super) fn clean(directory: &TempDir) -> Result<()> {
    let output = support::run(directory, false, LINT)?;
    assert!(output.status.success(), "{output:?}");
    assert!(!support::has_lint(&output, LINT), "{output:?}");
    Ok(())
}

pub(super) fn fixed(source: &str, dependencies: &str) -> Result<(TempDir, String)> {
    let (directory, fixed) = rewritten(source, dependencies)?;
    clean(&directory)?;
    Ok((directory, fixed))
}

pub(super) fn rewritten(source: &str, dependencies: &str) -> Result<(TempDir, String)> {
    let directory = fixture(source, dependencies)?;
    let before = support::run(&directory, false, LINT)?;
    assert!(before.status.success(), "{before:?}");
    assert!(support::has_lint(&before, LINT), "{before:?}");
    let output = support::run(&directory, true, LINT)?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
    Ok((directory, fixed))
}

pub(super) fn diagnosed(source: &str, dependencies: &str) -> Result<()> {
    let directory = fixture(source, dependencies)?;
    let output = support::run(&directory, false, LINT)?;
    assert!(output.status.success(), "{output:?}");
    assert!(support::has_lint(&output, LINT), "{output:?}");
    Ok(())
}

pub(super) fn unchanged(source: &str, dependencies: &str) -> Result<()> {
    let directory = fixture(source, dependencies)?;
    let output = support::run(&directory, true, LINT)?;
    assert!(output.status.success(), "{output:?}");
    assert!(support::has_lint(&output, LINT), "{output:?}");
    assert_eq!(support::source(&directory)?, source, "{output:?}");
    Ok(())
}
