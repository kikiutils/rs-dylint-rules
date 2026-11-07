use super::support;

#[test]
fn qualifies_the_original_expression_inside_pathkit_and_a_closure() -> anyhow::Result<()> {
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
        source
            .replace("    let root =", "    use std::env;\n    let root =")
            .replace("path!(current_dir()", "path!(env::current_dir()")
    );

    Ok(())
}

#[test]
fn qualifies_resolved_expression_arguments_in_standard_macros() -> anyhow::Result<()> {
    for source in [
        r#"pub fn value() -> String { format!("pid={}", std::process::id()) }
"#,
        "pub fn values() -> Vec<u32> { vec![std::process::id(); 2] }\n",
    ] {
        let (_, fixed) = support::fixed(source, "")?;
        assert_eq!(
            fixed,
            source
                .replace("std::process::id", "process::id")
                .replacen("{ ", "{ \nuse std::process;\n", 1)
        );
    }

    Ok(())
}

#[test]
fn rewrites_repeated_arguments_once_without_rewriting_the_macro_definition() -> anyhow::Result<()> {
    let source = r"macro_rules! evaluate { ($value:expr) => { ($value, $value, std::process::id()) }; }
pub fn values() -> (u32, u32, u32) { evaluate!(std::process::id()) }
";

    let (_, fixed) = support::fixed(source, "")?;
    assert_eq!(
        fixed,
        source.replace(
            "evaluate!(std::process::id())",
            "\nuse std::process;\nevaluate!(process::id())"
        )
    );
    Ok(())
}

#[test]
fn does_not_guess_api_paths_from_unevaluated_tokens() -> anyhow::Result<()> {
    support::accepted(
        r"pub fn text() -> &'static str { stringify!(std::process::id()) }
",
        "",
    )
}

#[test]
fn expression_fragments_use_the_callers_rust_edition() -> anyhow::Result<()> {
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
        source
            .replace("std::process::id", "process::id")
            .replace("{ format!", "{ \nuse std::process;\nformat!")
    );

    support::clean(&directory)
}

#[test]
fn does_not_project_outer_imports_into_nested_input_modules() -> anyhow::Result<()> {
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
fn does_not_import_outside_modules_created_by_macro_expansion() -> anyhow::Result<()> {
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
fn macro_arguments_still_respect_namespace_conflicts() -> anyhow::Result<()> {
    support::unchanged(
        r#"pub fn text<process>() -> String { format!("{}", std::process::id()) }
"#,
        "",
    )
}

#[test]
fn repeated_arguments_require_safe_names_in_every_expansion_scope() -> anyhow::Result<()> {
    support::unchanged(
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
fn repeated_bare_arguments_preserve_different_original_resolutions() -> anyhow::Result<()> {
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
fn macro_arguments_do_not_hoist_imports_out_of_conditional_statements() -> anyhow::Result<()> {
    let sources = [
        r#"pub fn run() {
    #[cfg(feature = "timers")]
    let delays = vec![tokio::time::sleep(std::time::Duration::ZERO)];
}
"#,
        r#"macro_rules! evaluate { ($value:expr) => { $value }; }
pub fn run() {
    evaluate!({
        #[cfg(feature = "timers")]
        let delay = tokio::time::sleep(std::time::Duration::ZERO);
    });
}
"#,
    ];

    for source in sources {
        let directory = support::fixture(source, &support::timers_feature())?;
        let output = crate::support::run(&directory, true, support::LINT)?;
        assert!(output.status.success(), "{output:?}");
        assert!(
            crate::support::has_lint(&output, support::LINT),
            "{source}\nfixed={}\n{output:?}",
            crate::support::source(&directory)?
        );

        assert_eq!(crate::support::source(&directory)?, source, "{output:?}");

        support::check_without_timers(&directory)?;
    }

    Ok(())
}
