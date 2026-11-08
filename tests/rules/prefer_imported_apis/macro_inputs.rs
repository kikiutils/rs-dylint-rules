use anyhow::Result;

use super::support;

#[test]
fn qualifies_errno_in_nested_match_guards_and_preserves_behavior() -> Result<()> {
    let import = "use rustix::io::Errno;\n";
    let body = r"pub fn retry(error: Option<Errno>) -> bool {
    matches!(error, Some(error) if matches!(error,
        code if code == Errno::INTR || code == Errno::AGAIN || code == Errno::WOULDBLOCK,
    ))
}
#[test]
fn behavior() {
    assert!(retry(Some(Errno::INTR)));
    assert!(retry(Some(Errno::AGAIN)));
    assert!(retry(Some(Errno::WOULDBLOCK)));
    assert!(!retry(Some(Errno::INVAL)));
    assert!(!retry(None));
}
";

    let directory = support::fixture(&format!("{import}{body}"), "rustix=\"=1.1.6\"")?;
    support::behavior(&directory)?;
    let output = crate::support::run(&directory, true, support::LINT)?;
    assert!(output.status.success(), "{output:?}");
    let fixed = crate::support::source(&directory)?;
    assert_eq!(
        fixed,
        format!(
            "{import}{}",
            body.replace("Option<Errno>", "Option<rustix::io::Errno>")
                .replace("code == Errno::", "code == rustix::io::Errno::")
        )
    );

    support::clean(&directory)?;
    let output = crate::support::run(&directory, true, support::LINT)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(crate::support::source(&directory)?, fixed);
    support::behavior(&directory)
}

#[test]
fn qualifies_the_original_expression_inside_pathkit_and_a_closure() -> Result<()> {
    let source = r"use std::{cell::OnceCell, env::current_dir};
use pathkit::{Path, SyncFsOps, path};
pub fn root() -> Option<Path> {
    let root = OnceCell::new();
    root.get_or_init(|| path!(current_dir().ok()?).canonicalize_sync().ok()).clone()
}
";

    let (_, fixed) = support::rewritten(source, "pathkit=\"=3.1.0\"")?;
    assert_eq!(
        fixed,
        format!(
            "use std::env;\n{}",
            source.replace("path!(current_dir()", "path!(env::current_dir()")
        )
    );

    Ok(())
}

#[test]
fn qualifies_resolved_expression_arguments_in_standard_macros() -> Result<()> {
    for source in [
        r#"pub fn value() -> String { format!("pid={}", std::process::id()) }
"#,
        "pub fn values() -> Vec<u32> { vec![std::process::id(); 2] }\n",
    ] {
        let (_, fixed) = support::fixed(source, "")?;
        assert_eq!(
            fixed,
            format!(
                "use std::process;\n{}",
                source.replace("std::process::id", "process::id")
            )
        );
    }

    Ok(())
}

#[test]
fn rewrites_repeated_arguments_once_without_rewriting_the_macro_definition() -> Result<()> {
    let source = r"macro_rules! evaluate { ($value:expr) => { ($value, $value, std::process::id()) }; }
pub fn values() -> (u32, u32, u32) { evaluate!(std::process::id()) }
";

    let (_, fixed) = support::fixed(source, "")?;
    assert_eq!(
        fixed,
        format!(
            "use std::process;\n{}",
            source.replace("evaluate!(std::process::id())", "evaluate!(process::id())")
        )
    );

    Ok(())
}

#[test]
fn does_not_guess_api_paths_from_unevaluated_tokens() -> Result<()> {
    support::accepted(
        r"pub fn text() -> &'static str { stringify!(std::process::id()) }
",
        "",
    )
}

#[test]
fn expression_fragments_use_the_callers_rust_edition() -> Result<()> {
    let source = r#"fn gen() -> u8 { 7 }
pub fn text() -> String { format!("{}:{}", gen(), std::process::id()) }
"#;

    let directory = support::fixture(source, "")?;
    let manifest = crate::support::read(&directory, "Cargo.toml")?.replace("edition=\"2024\"", "edition=\"2021\"");
    crate::support::write(&directory, "Cargo.toml", &manifest)?;
    let output = crate::support::run(&directory, true, support::LINT)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        crate::support::source(&directory)?,
        format!(
            "use std::process;\n{}",
            source.replace("std::process::id", "process::id")
        )
    );

    support::clean(&directory)
}

#[test]
fn does_not_project_outer_imports_into_nested_input_modules() -> Result<()> {
    support::accepted(
        r"macro_rules! evaluate { ($value:expr) => { $value }; }
pub fn id() -> u32 {
    evaluate!({
        mod inner { pub fn id() -> u32 { std::process::id() } }
        inner::id()
    })
}
",
        "",
    )
}

#[test]
fn does_not_import_outside_modules_created_by_macro_expansion() -> Result<()> {
    for source in [
        r"macro_rules! wrap { ($value:expr) => { mod inner { pub fn id() -> u32 { $value } } }; }
wrap!(std::process::id());
",
        r"macro_rules! wrap { ($value:expr) => { mod inner { pub fn id() -> u32 { $value } } }; }
pub fn id() -> u32 { wrap!(std::process::id()); inner::id() }
",
    ] {
        support::unchanged(source, "")?;
    }

    Ok(())
}

#[test]
fn macro_arguments_keep_full_paths_when_short_names_conflict() -> Result<()> {
    support::accepted(
        r#"pub fn text<process>() -> String { format!("{}", std::process::id()) }
"#,
        "",
    )
}

#[test]
fn repeated_arguments_keep_the_full_path_when_a_short_name_would_be_captured() -> Result<()> {
    support::accepted(
        r"macro_rules! twice { ($value:expr) => {{
    let first = $value;
    let second = { mod process { pub fn id() -> u32 { 0 } } $value };
    (first, second)
}}; }
pub fn values() -> (u32, u32) { twice!(std::process::id()) }
",
        "",
    )
}

#[test]
fn repeated_bare_arguments_preserve_different_original_resolutions() -> Result<()> {
    support::unchanged(
        r"use std::process::id;
macro_rules! twice { ($value:expr) => {{
    let first = $value;
    let second = { fn id() -> u32 { 7 } $value };
    (first, second)
}}; }
pub fn values() -> (u32, u32) { twice!(id()) }
",
        "",
    )
}

#[test]
fn macro_arguments_keep_full_paths_under_call_site_cfg_statements() -> Result<()> {
    let source = r#"pub fn run() {
    #[cfg(feature = "timers")]
    let delays = vec![tokio::time::sleep(std::time::Duration::ZERO)];
}
"#;

    let directory = support::fixture(source, &support::timers_feature())?;
    support::clean(&directory)?;
    support::check_without_timers(&directory)
}

#[test]
fn cfg_tokens_inside_macro_arguments_use_the_call_site_target() -> Result<()> {
    let source = r#"macro_rules! evaluate { ($value:expr) => { $value }; }
pub fn run() {
    evaluate!({
        #[cfg(feature = "timers")]
        let delay = tokio::time::sleep(std::time::Duration::ZERO);
    });
}
"#;

    let (_, fixed) = support::fixed(source, &support::timers_feature())?;
    assert_eq!(
        fixed,
        format!(
            "use tokio::time::sleep;\n{}",
            source.replace(
                "tokio::time::sleep(std::time::Duration::ZERO)",
                "sleep(std::time::Duration::ZERO)"
            )
        )
    );

    Ok(())
}
