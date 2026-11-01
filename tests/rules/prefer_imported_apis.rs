use anyhow::Result;
use tempfile::TempDir;

use crate::support;

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
    assert_eq!(
        support::diagnostics(&output, "kiki_prefer_imported_apis").len(),
        6,
        "{output:?}"
    );

    let output = support::run(&directory, true, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
    assert_eq!(
        fixed,
        r#"use anyhow::Result as AnyhowResult;
pub fn run(valid: bool) -> AnyhowResult<()> {
use anyhow::ensure;
use anyhow::Error as AnyhowError;

    let _: Option<AnyhowError> = None;
    let _ = AnyhowError::msg("example");
    ensure!(valid, "invalid");
    if !valid {
use anyhow::bail;
bail!("failed"); }
    let _ = || {
use anyhow::anyhow;
anyhow!("closure") };
    Ok(())
}
#[test] fn behavior() { assert!(run(true).is_ok()); assert!(run(false).is_err()); }
"#
    );

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
    let lines: Vec<_> = reports
        .iter()
        .map(|report| report["message"]["spans"][0]["line_start"].as_u64().unwrap_or_default())
        .collect();
    assert_eq!(lines, [1, 2, 3, 4, 5, 6, 7, 8, 8, 9], "{reports:?}");

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
#[test] fn behavior() { assert!(valid(true).is_ok()); assert!(valid(false).is_err()); }
"#;

    let directory = fixture(source)?;
    let output = support::run(&directory, true, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
    assert_eq!(
        fixed,
        r#"use anyhow::{Error as AnyhowError, Result as AnyhowResult, anyhow, bail, ensure};
fn valid(flag: bool) -> AnyhowResult<()> {
    let _ = AnyhowError::msg("example");
    ensure!(flag, "bad");
    if !flag { bail!("bad"); }
    let _ = anyhow!("example");
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
#[test] fn behavior() { assert!(valid(true).is_ok()); assert!(valid(false).is_err()); }
"#
    );

    let output = support::cargo(&directory).args(["test", "--lib", "--quiet"]).output()?;
    assert!(output.status.success(), "{output:?}");
    Ok(())
}

#[test]
fn renamed_dependency_local_lookalikes_and_macro_inputs() -> Result<()> {
    let source = r#"use errors::Result;
mod local { pub type Result<T> = core::result::Result<T, ()>; pub struct Error; }
fn other() -> local::Result<()> { let _: Option<local::Error> = None; Ok(()) }
fn run() -> errors::Result<()> { errors::bail!("bad"); }
mod expansion {
use errors::bail;
macro_rules! passthrough { ($($tokens:tt)*) => { $($tokens)* } }
use passthrough as pass;
self::pass! { fn generated() -> errors::Result<()> { errors::bail!("bad"); } }
}
"#;
    let directory = support::fixture(source)?;

    support::dependency(
        &directory,
        &format!("errors={{package=\"anyhow\",version=\"{}\"}}", support::ANYHOW_VERSION),
    )?;

    let output = support::run(&directory, true, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
    assert_eq!(
        fixed,
        r#"use errors::Result;
mod local { pub type Result<T> = core::result::Result<T, ()>; pub struct Error; }
fn other() -> local::Result<()> { let _: Option<local::Error> = None; Ok(()) }
fn run() -> Result<()> {
use errors::bail;
 bail!("bad"); }
mod expansion {
use errors::bail;
macro_rules! passthrough { ($($tokens:tt)*) => { $($tokens)* } }
use passthrough as pass;
self::pass! { fn generated() -> errors::Result<()> { errors::bail!("bad"); } }
}
"#
    );

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
    assert_eq!(
        fixed,
        "mod plain {\nuse anyhow::Result;\n pub fn run() -> Result<()> { Ok(()) } }\nfn expression() { let _ = include!(\"expression.rs\"); }\ninclude!(\"items.rs\");\n"
    );
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

        assert_eq!(reports.len(), usize::from(accessible), "{visibility}: {output:?}");
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
    assert_eq!(
        fixed,
        source
            .replace("use anyhow::{Result, bail};", "use anyhow::{Result, bail, anyhow};")
            .replace("anyhow::anyhow!", "anyhow!")
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
    assert_eq!(
        fixed,
        r#"mod conditional_fn {
    use anyhow::{Result, bail};
    #[cfg(all())]
    fn run() -> Result<()> {
use anyhow::anyhow;
 Err(anyhow!("conditional function")) }
}
mod conditional_block {
    use anyhow::{Result, bail};
    fn run() -> Result<()> {
        #[cfg(all())]
        {
use anyhow::anyhow;
 return Err(anyhow!("conditional block")); }
        #[cfg(any())]
        { missing::unresolved!(); }
    }
}
#[cfg(all())]
mod inline_existing {
    use anyhow::{Result, bail, anyhow,};
    fn first() -> Result<()> { Err(anyhow!("first")) }
    fn second() -> Result<()> { Err(anyhow!("second")) }
}
mod inline_missing {
    fn run() {
use anyhow::anyhow;
 let _ = anyhow!("local"); }
}
mod conditional_missing {
    #[cfg(all())]
    fn run() {
use anyhow::anyhow;
 let _ = anyhow!("conditional local"); }
}
"#
    );

    let output = support::run(&directory, false, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        support::diagnostics(&output, "kiki_prefer_imported_apis").is_empty(),
        "{output:?}"
    );

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
    assert_eq!(
        fixed,
        r#"mod mixed {
use anyhow::Error as AnyhowError;

    use anyhow::{Result, anyhow};
    fn error() -> AnyhowError { anyhow!("mixed") }
}
mod routes {
    mod facade { pub use anyhow::{anyhow, Result}; }
    use facade::{Result, anyhow};
    use anyhow::{bail, ensure};
    fn run() -> Result<()> {
        let _ = anyhow!("facade");
        let _ = anyhow!("direct route to same macro");
        ensure!(true, "direct");
        Ok(())
    }
}
mod static_context {
use anyhow::anyhow;
use anyhow::Error as AnyhowError;

    static ERROR: std::sync::LazyLock<AnyhowError> = std::sync::LazyLock::new(|| anyhow!("static"));
    use anyhow::{bail};
    fn run() { let _ = anyhow!("function"); }
}
mod documented {
    //! Module documentation must stay before inserted items.
use anyhow::Result;

    fn run() -> Result<()> { Ok(()) }
}
mod conditional {
    #[cfg(feature = "conditional")]
    use anyhow::anyhow;
    fn run() { let _ = anyhow::anyhow!("conditional import"); }
}
"#
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
    assert_eq!(
        fixed,
        r#"use anyhow::{Context, Result, ensure, anyhow};
#[derive(Default)] struct Source;
macro_rules! path { ($value:expr) => { std::path::Path::new($value) }; }
fn parse() -> Result<()> {
    let _ = format!("{}", 1);
    let _ = path!("example");
    None.ok_or_else(|| anyhow!("unrelated macros and derives"))
}
mod imported_parent {
    use anyhow::bail;
    mod child {
use anyhow::Result;
 fn run() -> Result<()> {
use anyhow::bail;
 bail!("not inherited import"); } }
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
use anyhow::Error as AnyhowError;

    struct Named { anyhow: u32 }
    macro_rules! methods { () => { impl Named { fn anyhow() {} } }; }
    methods!();
    fn run() -> AnyhowError {
use anyhow::anyhow;
 anyhow!("field and method names") }
    fn value_local() { let AnyhowError = 1; let _: Option<AnyhowError> = None; }
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
"#
    );

    Ok(())
}

#[test]
fn inner_docs_and_planned_imports_preserve_scope_and_shadowing() -> Result<()> {
    let source = r#"//! Crate docs.
#![allow(dead_code)]
pub fn top() -> anyhow::Result<()> {
    let _: anyhow::Result<()> = Ok(());
    Ok(())
}
mod nested {
    /*! Module docs. */
    #![allow(dead_code)]
    pub fn error() -> anyhow::Error { anyhow::Error::msg("nested") }
    pub fn shadowed() -> anyhow::Error {
        struct AnyhowError;
        let _: Option<anyhow::Error> = None;
        anyhow::Error::msg("shadowed")
    }
}
mod isolated { pub fn run() -> anyhow::Result<()> { Ok(()) } }
mod macros {
    use anyhow::{Result};
    #[cfg(all())]
    pub fn conditional() -> Result<()> { Err(anyhow::anyhow!("conditional")) }
    pub fn unconditional() -> Result<()> { Err(anyhow::anyhow!("unconditional")) }
}
#[test] fn behavior() {
    assert!(top().is_ok());
    assert_eq!(nested::error().to_string(), "nested");
    assert_eq!(nested::shadowed().to_string(), "shadowed");
    assert!(isolated::run().is_ok());
    assert_eq!(macros::conditional().unwrap_err().to_string(), "conditional");
    assert_eq!(macros::unconditional().unwrap_err().to_string(), "unconditional");
}
"#;
    let directory = fixture(source)?;

    for fix in [false, true] {
        let output = support::run(&directory, fix, "kiki_prefer_imported_apis")?;
        assert!(output.status.success(), "{output:?}");

        let behavior = support::cargo(&directory).args(["test", "--lib", "--quiet"]).output()?;
        assert!(behavior.status.success(), "{behavior:?}");
    }

    assert_eq!(
        support::source(&directory)?,
        source
            .replace(
                "#![allow(dead_code)]\npub fn top() -> anyhow::Result",
                "#![allow(dead_code)]\nuse anyhow::Result;\npub fn top() -> Result"
            )
            .replace("let _: anyhow::Result", "let _: Result")
            .replace(
                "    #![allow(dead_code)]\n    pub fn error",
                "    #![allow(dead_code)]\nuse anyhow::Error as AnyhowError;\n\n    pub fn error"
            )
            .replace(
                "pub fn error() -> anyhow::Error { anyhow::Error::msg",
                "pub fn error() -> AnyhowError { AnyhowError::msg"
            )
            .replace("pub fn shadowed() -> anyhow::Error", "pub fn shadowed() -> AnyhowError")
            .replace(
                "mod isolated { pub fn run() -> anyhow::Result",
                "mod isolated {\nuse anyhow::Result;\n pub fn run() -> Result"
            )
            .replace("use anyhow::{Result};", "use anyhow::{Result, anyhow};")
            .replace("anyhow::anyhow!", "anyhow!")
    );

    Ok(())
}

#[test]
fn compliant_globs_support_reuse_and_late_import_diagnostics() -> Result<()> {
    let source = r#"mod facade { pub use anyhow::{Error as AnyhowError, Result as AnyhowResult, anyhow}; }
mod plain { pub use anyhow::Result; }
mod reuse {
    use crate::facade::*;
    pub fn run() -> anyhow::Result<()> {
        let _ = anyhow::Error::msg("error");
        let _ = anyhow::anyhow!("macro");
        Ok(())
    }
}
mod shadow {
    use crate::facade::*;
    struct AnyhowError;
    fn run() { let _: Option<anyhow::Error> = None; }
}
mod late {
    fn run() -> AnyhowResult<()> { let _ = anyhow!("late"); Ok(()) }
    use crate::facade::*;
}
mod late_result {
    fn run() -> Result<()> { Ok(()) }
    use crate::plain::*;
}
mod conditional {
    #[cfg(feature = "conditional")]
    use crate::plain::*;
    fn run() -> anyhow::Result<()> { Ok(()) }
}
#[test] fn behavior() { assert!(reuse::run().is_ok()); }
"#;
    let directory = fixture(source)?;
    support::manifest(&directory, "[features]\ndefault=[\"conditional\"]\nconditional=[]")?;

    for fix in [false, true] {
        let output = support::run(&directory, fix, "kiki_prefer_imported_apis")?;
        assert!(output.status.success(), "{output:?}");
        let diagnostics = support::diagnostics(&output, "kiki_prefer_imported_apis");
        let mut actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic["message"]["spans"][0]["line_start"]
                        .as_u64()
                        .unwrap_or_default(),
                    diagnostic["message"]["message"].as_str().unwrap_or_default(),
                )
            })
            .collect();
        let mut expected = vec![
            (14, "import this anyhow API first and use `AnyhowError`"),
            (17, "place the import for `AnyhowResult` above this use"),
            (17, "place the import for `anyhow` above this use"),
            (21, "place the import for `Result` above this use"),
            (27, "import this anyhow API first and use `Result`"),
        ];
        if !fix {
            expected.extend([
                (5, "import this anyhow API first and use `Result`"),
                (6, "import this anyhow API first and use `AnyhowError`"),
                (7, "import this anyhow API first and use `anyhow`"),
            ]);
        }

        actual.sort_unstable();
        expected.sort_unstable();
        assert_eq!(actual, expected, "{output:?}");

        let behavior = support::cargo(&directory).args(["test", "--lib", "--quiet"]).output()?;
        assert!(behavior.status.success(), "{behavior:?}");
    }

    assert_eq!(
        support::source(&directory)?,
        source
            .replace("pub fn run() -> anyhow::Result", "pub fn run() -> AnyhowResult")
            .replace("let _ = anyhow::Error::msg", "let _ = AnyhowError::msg")
            .replace("let _ = anyhow::anyhow!", "let _ = anyhow!")
    );

    let output = support::cargo(&directory)
        .args(["check", "--no-default-features", "--quiet"])
        .output()?;
    assert!(output.status.success(), "{output:?}");
    Ok(())
}

#[test]
fn planned_group_imports_do_not_leave_earlier_macro_calls_above_their_import() -> Result<()> {
    let source = r#"use anyhow::Result;
fn a() -> Result<()> { anyhow::bail!("x") }
use anyhow::{ensure};
fn b() -> Result<()> { ensure!(true); anyhow::bail!("y") }
#[test] fn behavior() {
    assert_eq!(a().unwrap_err().to_string(), "x");
    assert_eq!(b().unwrap_err().to_string(), "y");
}
"#;
    let directory = fixture(source)?;

    for fix in [false, true] {
        let output = support::run(&directory, fix, "kiki_prefer_imported_apis")?;
        assert!(output.status.success(), "{output:?}");

        let behavior = support::cargo(&directory).args(["test", "--lib", "--quiet"]).output()?;
        assert!(behavior.status.success(), "{behavior:?}");
    }

    assert_eq!(
        support::source(&directory)?,
        source
            .replace("{ anyhow::bail!(\"x\") }", "{\nuse anyhow::bail;\n bail!(\"x\") }")
            .replace("use anyhow::{ensure};", "use anyhow::{ensure, bail};")
            .replace("anyhow::bail!(\"y\")", "bail!(\"y\")")
    );

    let output = support::run(&directory, false, "kiki_prefer_imported_apis")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        support::diagnostics(&output, "kiki_prefer_imported_apis").is_empty(),
        "{output:?}"
    );
    Ok(())
}
