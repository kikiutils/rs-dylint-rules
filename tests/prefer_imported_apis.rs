use anyhow::Result;
use tempfile::TempDir;

mod support;

fn fixture(source: &str) -> Result<TempDir> {
    let directory = support::fixture(source)?;
    support::anyhow(&directory)?;
    Ok(directory)
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
    let output = support::run(&directory, false, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        support::diagnostics(&output, "kiki_prefer_imported_apis").len() >= 5,
        "{output:?}"
    );

    let output = support::run(&directory, true, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
    assert!(fixed.contains("-> AnyhowResult<()>"), "{fixed}");
    assert!(
        fixed.contains("use anyhow::Error as AnyhowError;"),
        "{fixed}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(fixed.contains("AnyhowError::msg("), "{fixed}");
    assert!(fixed.contains("use anyhow::ensure;"), "{fixed}");
    assert!(fixed.contains("ensure!(valid"), "{fixed}");
    assert!(fixed.contains("use anyhow::bail;"), "{fixed}");
    assert!(fixed.contains("use anyhow::anyhow;"), "{fixed}");
    let output = support::run(&directory, false, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        support::diagnostics(&output, "kiki_prefer_imported_apis").is_empty(),
        "{output:?}"
    );

    let output = support::cargo(&directory).args(["test", "--lib", "--quiet"]).output()?;
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
    let output = support::run(&directory, true, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(support::source(&directory)?, source);

    let output = support::run(&directory, false, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    let reports = support::diagnostics(&output, "kiki_prefer_imported_apis");
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
    let source = r#"use anyhow::{Error as AnyhowError, Result as AnyhowResult, anyhow, bail, ensure};
fn valid(flag: bool) -> anyhow::Result<()> {
    let _ = anyhow::Error::msg("example");
    anyhow::ensure!(flag, "bad");
    if !flag { anyhow::bail!("bad"); }
    let _ = anyhow::anyhow!("example");
    Ok(())
}
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
    let output = support::run(&directory, true, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
    assert!(fixed.contains("fn valid(flag: bool) -> AnyhowResult<()>"), "{fixed}");
    assert!(fixed.contains("let _ = AnyhowError::msg"), "{fixed}");
    assert!(fixed.contains("fn run() -> anyhow::Result<()>"), "{fixed}");
    assert!(fixed.contains("Option<anyhow::Error>"), "{fixed}");
    assert_eq!(fixed.matches("use anyhow::{").count(), 1, "{fixed}");
    support::write(
        &directory,
        "src/lib.rs",
        &format!(
            "{fixed}\n#[test] fn behavior() {{ assert!(valid(true).is_ok()); assert!(valid(false).is_err()); }}\n"
        ),
    )?;

    let output = support::cargo(&directory).args(["test", "--lib", "--quiet"]).output()?;
    assert!(output.status.success(), "{output:?}");
    Ok(())
}

#[test]
fn renamed_dependency_local_lookalikes_and_macro_inputs() -> Result<()> {
    let directory = support::fixture(
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

    support::dependency(
        &directory,
        &format!("errors={{package=\"anyhow\",version=\"{}\"}}", support::ANYHOW_VERSION),
    )?;

    let output = support::run(&directory, true, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
    assert!(fixed.contains("use errors::Result;"), "{fixed}");
    assert!(fixed.contains("use errors::bail;"), "{fixed}");
    assert!(fixed.contains("fn run() -> Result<()>"), "{fixed}");
    assert!(fixed.contains("fn other() -> local::Result<()>"), "{fixed}");
    assert!(fixed.contains("fn generated() -> errors::Result<()>"), "{fixed}");
    let output = support::run(&directory, false, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        support::diagnostics(&output, "kiki_prefer_imported_apis").is_empty(),
        "{output:?}"
    );

    Ok(())
}

#[test]
fn inserts_default_result_import_and_skips_standalone_include_fragments() -> Result<()> {
    let expression = "anyhow::anyhow!(\"included\")";
    let items = "fn included() -> anyhow::Result<()> { anyhow::bail!(\"included\"); }\n";
    let directory = support::fixture_with(
        r#"mod plain { pub fn run() -> anyhow::Result<()> { Ok(()) } }
fn expression() { let _ = include!("expression.rs"); }
include!("items.rs");
"#,
        &[("src/expression.rs", expression), ("src/items.rs", items)],
    )?;

    support::anyhow(&directory)?;
    let output = support::run(&directory, true, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
    assert!(fixed.contains("use anyhow::Result;"), "{fixed}");
    assert!(fixed.contains("pub fn run() -> Result<()>"), "{fixed}");
    assert_eq!(support::read(&directory, "src/expression.rs")?, expression);
    assert_eq!(support::read(&directory, "src/items.rs")?, items);

    let output = support::run(&directory, false, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        support::diagnostics(&output, "kiki_prefer_imported_apis").is_empty(),
        "{output:?}"
    );

    Ok(())
}

#[test]
fn glob_checks_only_bindings_visible_from_the_importing_module() -> Result<()> {
    let directory = fixture("mod hidden { use anyhow::Error; }\nuse hidden::*;\n")?;
    for visibility in [
        "",
        "pub(self) ",
        "pub(in crate::hidden) ",
        "pub(super) ",
        "pub(crate) ",
        "pub ",
    ] {
        support::write(
            &directory,
            "src/lib.rs",
            &format!("mod hidden {{ {visibility}use anyhow::Error; }}\nuse hidden::*;\n"),
        )?;

        let output = support::run(&directory, false, "kiki_prefer_imported_apis")?;
        assert!(output.status.success(), "{visibility}: {output:?}");
        let accessible = matches!(visibility, "pub(super) " | "pub(crate) " | "pub ");
        let reports: Vec<_> = support::diagnostics(&output, "kiki_prefer_imported_apis")
            .into_iter()
            .filter(|message| message["message"]["spans"][0]["line_start"] == 2)
            .collect();

        assert_eq!(!reports.is_empty(), accessible, "{visibility}: {output:?}");
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
    let output = support::run(&directory, true, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
    assert!(fixed.contains("use anyhow::{Result, bail, anyhow};"), "{fixed}");
    assert!(!fixed.contains("use anyhow::anyhow;"), "{fixed}");
    assert!(
        fixed.contains("|| anyhow!(\"runtime context is not initialized\")"),
        "{fixed}"
    );

    let output = support::run(&directory, false, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        support::diagnostics(&output, "kiki_prefer_imported_apis").is_empty(),
        "{output:?}"
    );

    // Unknown attributes stay barriers, even in an inactive cfg_attr branch.
    let source = r#"#[cfg_attr(any(), observe)]
fn protected() -> anyhow::Result<()> {
    None.ok_or_else(|| anyhow::anyhow!("keep attribute boundary"))
}
"#;

    let directory = fixture(source)?;
    let output = support::run(&directory, true, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        !support::diagnostics(&output, "kiki_prefer_imported_apis").is_empty(),
        "{output:?}"
    );

    assert_eq!(support::source(&directory)?, source);

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
    let output = support::run(&directory, true, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
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
    let output = support::run(&directory, false, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        support::diagnostics(&output, "kiki_prefer_imported_apis").is_empty(),
        "{output:?}"
    );

    assert_eq!(support::source(&directory)?, fixed);

    Ok(())
}

#[test]
fn import_targets_keep_routes_conditions_and_preambles_independent() -> Result<()> {
    let source = r#"mod mixed {
    use anyhow::{Result};
    fn error() -> anyhow::Error { anyhow::anyhow!("mixed") }
}
mod routes {
    mod facade { pub use anyhow::{anyhow, Result}; }
    use facade::{Result};
    use anyhow::{bail};
    fn run() -> Result<()> {
        let _ = facade::anyhow!("facade");
        let _ = anyhow::anyhow!("direct route to same macro");
        anyhow::ensure!(true, "direct");
        Ok(())
    }
}
mod static_context {
    static ERROR: std::sync::LazyLock<anyhow::Error> = std::sync::LazyLock::new(|| anyhow::anyhow!("static"));
    use anyhow::{bail};
    fn run() { let _ = anyhow::anyhow!("function"); }
}
mod documented {
    //! Module documentation must stay before inserted items.
    fn run() -> anyhow::Result<()> { Ok(()) }
}
mod conditional {
    #[cfg(feature = "conditional")]
    use anyhow::anyhow;
    fn run() { let _ = anyhow::anyhow!("conditional import"); }
}
"#;

    let directory = fixture(source)?;
    support::manifest(&directory, "[features]\ndefault=[\"conditional\"]\nconditional=[]")?;

    let output = support::run(&directory, true, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
    assert!(
        fixed.contains("use anyhow::Error as AnyhowError;"),
        "{fixed}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(fixed.contains("use anyhow::{Result, anyhow};"), "{fixed}");
    assert!(fixed.contains("use facade::{Result, anyhow};"), "{fixed}");
    assert!(fixed.contains("use anyhow::{bail, ensure};"), "{fixed}");
    assert!(fixed.contains("fn run() -> anyhow::Result<()> { Ok(()) }"), "{fixed}");
    assert!(fixed.contains("anyhow::anyhow!(\"conditional import\")"), "{fixed}");
    let static_module = fixed
        .split("mod static_context")
        .nth(1)
        .and_then(|text| text.split("mod documented").next());

    assert_eq!(
        static_module.map(|text| text.matches("use anyhow::anyhow;").count()),
        Some(1),
        "{fixed}"
    );

    let output = support::cargo(&directory)
        .args(["check", "--no-default-features", "--quiet"])
        .output()?;

    assert!(output.status.success(), "{output:?}");
    Ok(())
}

#[test]
fn compiler_names_distinguish_namespaces_scopes_and_macro_expansions() -> Result<()> {
    let source = r#"use anyhow::{Context, Result, ensure};
#[derive(Default)] struct Source;
macro_rules! path { ($value:expr) => { std::path::Path::new($value) }; }
fn parse() -> Result<()> {
    let _ = format!("{}", 1);
    let _ = path!("example");
    None.ok_or_else(|| anyhow::anyhow!("unrelated macros and derives"))
}
mod imported_parent {
    use anyhow::bail;
    mod child { fn run() -> anyhow::Result<()> { anyhow::bail!("not inherited import"); } }
}
mod generated_type {
    macro_rules! declare { () => { struct AnyhowError; }; }
    declare!();
    fn run() { let _: Option<anyhow::Error> = None; }
}
mod generated_block_type {
    macro_rules! declare { () => { struct AnyhowError; }; }
    fn run() { declare!(); let _: Option<anyhow::Error> = None; }
}
mod other_namespaces {
    struct Named { anyhow: u32 }
    macro_rules! methods { () => { impl Named { fn anyhow() {} } }; }
    methods!();
    fn run() -> anyhow::Error { anyhow::anyhow!("field and method names") }
    fn value_local() { let AnyhowError = 1; let _: Option<anyhow::Error> = None; }
    fn generic<AnyhowError>() { let _: Option<anyhow::Error> = None; }
}
mod glob {
    mod other { pub struct AnyhowError; }
    macro_rules! imports { () => { use other::*; }; }
    fn direct() { use other::*; let _: Option<anyhow::Error> = None; }
    fn generated() { imports!(); let _: Option<anyhow::Error> = None; }
}
mod glob_parent {
    use anyhow::{Result};
    pub mod child { use super::*; }
    fn run() -> Result<()> { Err(anyhow::anyhow!("glob consumer")) }
}
mod prelude {
    fn existing() -> Result<(), ()> { Ok(()) }
    fn target() -> anyhow::Result<()> { Ok(()) }
}
"#;

    let directory = fixture(source)?;
    let output = support::run(&directory, true, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
    assert!(
        fixed.contains("use anyhow::{Context, Result, ensure, anyhow};"),
        "{fixed}"
    );

    assert!(
        fixed.contains("|| anyhow!(\"unrelated macros and derives\")"),
        "{fixed}"
    );

    assert!(fixed.contains("anyhow!(\"field and method names\")"), "{fixed}");
    assert!(fixed.contains("bail!(\"not inherited import\")"), "{fixed}");
    assert_eq!(fixed.matches("Option<anyhow::Error>").count(), 5, "{fixed}");
    assert!(
        fixed.contains("let AnyhowError = 1; let _: Option<AnyhowError>"),
        "{fixed}"
    );

    assert!(fixed.contains("anyhow::anyhow!(\"glob consumer\")"), "{fixed}");
    assert!(fixed.contains("fn target() -> anyhow::Result<()>"), "{fixed}");
    Ok(())
}
