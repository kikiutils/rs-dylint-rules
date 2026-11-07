use anyhow::Result;

use super::support;

#[test]
fn standalone_imports_preserve_the_resolved_local_module_route() -> Result<()> {
    let source = r#"use anyhow::Result;
mod errors { pub use anyhow::Context; }
use errors::{Context};
pub fn run() -> Result<()> {
    mod errors { pub use anyhow::bail; }
    errors::bail!("example")
}
"#;

    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(
        fixed,
        source
            .replace("    mod errors", "    use errors::bail;\n    mod errors")
            .replace("errors::bail!", "bail!")
    );

    Ok(())
}

#[test]
fn imports_a_multiline_raw_function_path_without_a_redundant_alias() -> Result<()> {
    let source = r"use std::time::Duration;
pub async fn run() {
    tokio ::
        time ::
        r#sleep(Duration::from_millis(1)).await;
}
";

    let (_, fixed) = support::fixed(source, &support::tokio())?;
    assert_eq!(
        fixed,
        source
            .replace("tokio ::\n        time ::\n        r#sleep", "sleep")
            .replace(
                "    sleep(",
                "    use tokio ::\n        time ::\n        r#sleep;\n    sleep("
            )
    );

    Ok(())
}

#[test]
fn diagnoses_redundant_extern_crate_aliases_used_by_managed_apis() -> Result<()> {
    support::diagnosed(
        r#"extern crate anyhow as anyhow;
pub fn error() -> anyhow::Error { anyhow::Error::msg("example") }
"#,
        &support::anyhow(),
    )
}

#[test]
fn accepts_extern_crate_aliases_used_only_by_unmanaged_apis() -> Result<()> {
    support::accepted(
        r#"extern crate std as standard;
pub fn command() -> standard::process::Command { standard::process::Command::new("example") }
"#,
        "",
    )
}
