use anyhow::Result;

use super::support;

#[test]
fn recognizes_renamed_dependencies_and_reexports_by_their_resolved_identity() -> Result<()> {
    let source = r#"mod bridge { pub use errors::anyhow; }
pub fn error() -> errors::Error { bridge::anyhow!("example") }
"#;

    let directory = crate::support::fixture(source)?;
    crate::support::dependency(
        &directory,
        &format!(
            "errors={{package=\"anyhow\",version=\"{}\"}}",
            crate::support::ANYHOW_VERSION
        ),
    )?;

    let output = crate::support::run(&directory, false, support::LINT)?;
    assert!(output.status.success(), "{output:?}");
    assert!(crate::support::has_lint(&output, support::LINT), "{output:?}");
    Ok(())
}

#[test]
fn does_not_reuse_a_crate_alias_from_an_unrelated_nested_scope() -> Result<()> {
    let source = r#"pub fn outside() -> errors::Error {
    { use errors as anyhow; let _ = anyhow::Error::msg("inner"); }
    errors::Error::msg("outer")
}
"#;

    let directory = support::fixture(
        source,
        &format!(
            "errors={{package=\"anyhow\",version=\"{}\"}}",
            crate::support::ANYHOW_VERSION
        ),
    )?;

    let output = crate::support::run(&directory, false, support::LINT)?;
    assert!(output.status.success(), "{output:?}");
    let diagnostics = crate::support::diagnostics(&output, support::LINT);
    assert!(!diagnostics.is_empty(), "{output:?}");
    for diagnostic in diagnostics {
        for child in diagnostic["message"]["children"].as_array().into_iter().flatten() {
            let spans: Vec<_> = child["spans"].as_array().into_iter().flatten().collect();
            let replaces_outer_path = spans.iter().any(|span| {
                span["suggested_replacement"]
                    .as_str()
                    .is_some_and(|text| text.starts_with("anyhow::Error"))
            });

            if replaces_outer_path {
                assert!(
                    spans.iter().any(|span| span["suggested_replacement"]
                        .as_str()
                        .is_some_and(|text| text.contains("as anyhow"))),
                    "the inner alias cannot justify an outer replacement: {child}"
                );
            }
        }
    }

    Ok(())
}

#[test]
fn does_not_confuse_same_named_local_functions_or_unrelated_namespaces() -> Result<()> {
    support::accepted(
        r"mod local {
    pub fn sleep() {}
    pub struct Error;
}
pub fn run() { local::sleep(); let _ = local::Error; }
",
        "",
    )
}

#[test]
fn rejects_new_direct_imports_that_would_capture_parameters_or_pattern_bindings() -> Result<()> {
    for source in [
        r"use std::time::Duration;
pub async fn run(sleep: fn()) {
    tokio::time::sleep(Duration::from_millis(1)).await;
    sleep();
}
",
        r"use std::time::Duration;
pub async fn run() {
    let (sleep, _) = (|| {}, 0);
    tokio::time::sleep(Duration::from_millis(1)).await;
    sleep();
}
",
    ] {
        support::unchanged(source, &support::tokio())?;
    }

    Ok(())
}

#[test]
fn permits_module_imports_despite_same_named_value_bindings() -> Result<()> {
    let source = r"pub fn id(process: u8) -> (u8, u32) { (process, std::process::id()) }
";

    let (_, fixed) = support::fixed(source, "")?;
    assert_eq!(
        fixed,
        source.replace(
            "{ (process, std::process::id()) }",
            "{ use std::process;\n(process, process::id()) }"
        )
    );

    Ok(())
}

#[test]
fn accepts_full_paths_when_short_module_names_conflict_in_the_type_namespace() -> Result<()> {
    support::accepted(
        r"pub fn id<process>() -> u32 { std::process::id() }
",
        "",
    )
}

#[test]
fn reports_direct_imports_below_their_use_without_moving_them() -> Result<()> {
    support::unchanged(
        r"use std::time::Duration;
pub async fn run() {
    sleep(Duration::from_millis(1)).await;
    use tokio::time::sleep;
}
",
        &support::tokio(),
    )
}

#[test]
fn preserves_public_reexports_and_disallowed_aliases() -> Result<()> {
    support::unchanged(
        r"pub use anyhow::Error as AppError;
pub use anyhow::Result as AppResult;
",
        &support::anyhow(),
    )
}

#[test]
fn qualifies_bare_glob_imports_without_expanding_the_glob() -> Result<()> {
    let source = r"use std::process::*;
pub fn current() -> u32 { id() }
";

    let (_, fixed) = support::rewritten(source, "")?;
    assert_eq!(fixed, source.replace("{ id()", "{ use std::process;\nprocess::id()"));
    Ok(())
}

#[test]
fn namespace_conflicts_use_full_paths_for_modules_and_generic_parameters() -> Result<()> {
    for (source, expected) in [
        (
            "use std::process::id;\npub fn current<process>() -> u32 { id() }\n",
            "pub fn current<process>() -> u32 { std::process::id() }\n",
        ),
        (
            r#"use std::io::{Error as IoError, Result as IoResult};
use std::process::id;
mod io {}
mod process {}
pub fn error() -> IoResult<()> { Err(IoError::other("example")) }
pub fn pid() -> u32 { id() }
"#,
            r#"mod io {}
mod process {}
pub fn error() -> std::io::Result<()> { Err(std::io::Error::other("example")) }
pub fn pid() -> u32 { std::process::id() }
"#,
        ),
    ] {
        let (_, fixed) = support::fixed(source, "")?;
        assert_eq!(fixed, expected);
    }

    Ok(())
}

#[test]
fn does_not_insert_bindings_into_modules_consumed_by_globs() -> Result<()> {
    support::accepted(
        r"mod producer {
    pub fn result() -> std::io::Result<()> { Ok(()) }
    mod consumer { use super::*; pub fn run() { let _ = result(); } }
}
",
        "",
    )
}

#[test]
fn rejects_imports_whose_crate_root_is_shadowed() -> Result<()> {
    support::unchanged(
        "pub fn current() { mod std {} let _ = ::std::env::current_dir(); }\n",
        "",
    )
}
