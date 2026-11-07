use anyhow::Result;

use super::support;

#[test]
fn places_new_imports_after_preamble_without_detaching_outer_docs() -> Result<()> {
    let source = r"// Shared fixture helpers.
//! Module documentation.
#![allow(dead_code)]

// Keep this comment with the function.
/// Function documentation.
pub fn io() -> std::io::Result<()> { Ok(()) }
pub fn channels(_: std::sync::mpsc::Sender<u8>) {}
";

    let (_, fixed) = support::fixed(source, "")?;
    assert_eq!(
        fixed,
        source
            .replace("std::io::Result", "io::Result")
            .replace("std::sync::mpsc::Sender", "std_mpsc::Sender")
            .replace(
                "// Keep this comment",
                "use std::io;\nuse std::sync::mpsc as std_mpsc;\n// Keep this comment"
            )
    );

    Ok(())
}

#[test]
fn reuses_visible_imports_and_keeps_a_second_fix_idempotent() -> Result<()> {
    let source = r#"use anyhow::anyhow;
pub fn error() -> anyhow::Error { anyhow::anyhow!("example") }
"#;

    let (directory, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(fixed, source.replace("{ anyhow::anyhow!", "{ anyhow!"));
    let output = crate::support::run(&directory, true, support::LINT)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(crate::support::source(&directory)?, fixed);
    Ok(())
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
        source.replace("{ anyhow::anyhow!", "{ \nuse anyhow::anyhow;\nanyhow!")
    );

    Ok(())
}

#[test]
fn fixes_original_function_bodies_wrapped_by_tokio_main() -> Result<()> {
    let dependencies = format!(
        "{}\ntokio={{version=\"{}\",features=[\"rt\",\"time\",\"macros\"]}}",
        support::anyhow(),
        support::TOKIO_VERSION
    );

    let source = r#"#[tokio::main(flavor = "current_thread")]
pub async fn error() -> anyhow::Error { anyhow::anyhow!("example") }
"#;

    let (_, fixed) = support::fixed(source, &dependencies)?;
    assert_eq!(
        fixed,
        source.replace("{ anyhow::anyhow!", "{ \nuse anyhow::anyhow;\nanyhow!")
    );

    Ok(())
}

#[test]
fn leaves_paths_with_internal_comments_unchanged() -> Result<()> {
    let source = "pub fn id() -> u32 { std::process /* origin */ ::id() }\n";
    support::unchanged(source, "")
}

#[test]
fn preserves_bom_and_crlf_when_reusing_existing_namespace_imports() -> Result<()> {
    let source = "\u{feff}use std::process;\r\npub fn id() -> u32 { std::process::id() }\r\n";
    let (_, fixed) = support::fixed(source, "")?;
    assert_eq!(fixed, source.replace("{ std::process::id()", "{ process::id()"));
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
