use anyhow::Result;

use crate::support;

const CHILD: (&str, &str) = ("src/child.rs", "pub struct Value;\n");

#[test]
fn relative_and_absolute_crate_imports_are_classified_and_fixed() -> Result<()> {
    for prefix in ["", "::"] {
        let source = format!(
            "use crate::child::Value;\nmod child;\nuse {prefix}std as Standard;\nuse {prefix}core as Core;\nuse {prefix}std::fmt;\n"
        );

        let directory = support::fixture_with(&source, &[CHILD])?;
        let output = support::run(&directory, false, "kiki_import_module_order")?;
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            support::diagnostics(&output, "kiki_import_module_order").len(),
            1,
            "{output:?}"
        );

        let output = support::run(&directory, true, "kiki_import_module_order")?;
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            support::source(&directory)?,
            format!(
                "use {prefix}std as Standard;\nuse {prefix}core as Core;\nuse {prefix}std::fmt;\n\nmod child;\n\nuse crate::child::Value;\n"
            )
        );

        let output = support::run(&directory, false, "kiki_import_module_order")?;
        assert!(output.status.success(), "{output:?}");
        assert!(!support::has_lint(&output, "kiki_import_module_order"), "{output:?}");
    }

    Ok(())
}

#[test]
fn native_diagnostics_offer_fix_and_dylint_applies_it() -> Result<()> {
    let source = "pub /* visibility */ use /* local */ crate::child::{/* local tree */ Value} /* local end */;\nmod child;\nuse // external\nstd::{fmt, /* external */ io} /* external end */;\n";
    let directory = support::fixture_with(source, &[CHILD])?;
    let output = support::run(&directory, false, "kiki_import_module_order")?;
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let messages = support::diagnostics(&output, "kiki_import_module_order");
    assert_eq!(messages.len(), 1, "{output:?}");

    assert_eq!(support::source(&directory)?, source);
    let output = support::run(&directory, true, "kiki_import_module_order")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        support::source(&directory)?,
        "use // external\nstd::{fmt, /* external */ io} /* external end */;\n\nmod child;\n\npub /* visibility */ use /* local */ crate::child::{/* local tree */ Value} /* local end */;\n"
    );

    let output = support::run(&directory, false, "kiki_import_module_order")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        support::diagnostics(&output, "kiki_import_module_order").is_empty(),
        "{output:?}"
    );

    Ok(())
}

#[test]
fn fixes_nested_scopes_and_ignores_inactive_code() -> Result<()> {
    let source =
        "mod child;\nfn run() {\n    use crate::child::Value;\n    use std::fmt;\n}\n#[cfg(any())] mod missing;\n";

    let directory = support::fixture_with(source, &[CHILD])?;
    let output = support::run(&directory, true, "kiki_import_module_order")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        support::source(&directory)?,
        source.replace(
            "use crate::child::Value;\n    use std::fmt;",
            "use std::fmt;\n\nuse crate::child::Value;"
        )
    );

    Ok(())
}

#[test]
fn visible_imports_are_resolved_reordered_and_keep_their_visibility() -> Result<()> {
    let source = "pub use crate::child::Value as PublicValue;\npub(crate) use crate::child::Value as CrateValue;\npub(self) use crate::child::Value as PrivateValue;\nmod child;\npub use std::fmt::Debug;\npub(crate) use std::fmt::Display;\nmod nested {\n    pub(super) use crate::child::Value as ParentValue;\n    pub(in crate::nested) use crate::child::Value as NestedValue;\n    pub use std::fmt::Write;\n}\n";
    let directory = support::fixture_with(source, &[CHILD])?;
    let output = support::run(&directory, true, "kiki_import_module_order")?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
    assert_eq!(
        fixed,
        "pub use std::fmt::Debug;\npub(crate) use std::fmt::Display;\n\nmod child;\n\npub use crate::child::Value as PublicValue;\npub(crate) use crate::child::Value as CrateValue;\npub(self) use crate::child::Value as PrivateValue;\nmod nested {\n    pub use std::fmt::Write;\n\npub(super) use crate::child::Value as ParentValue;\npub(in crate::nested) use crate::child::Value as NestedValue;\n}\n"
    );

    let output = support::run(&directory, false, "kiki_import_module_order")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        support::diagnostics(&output, "kiki_import_module_order").is_empty(),
        "{output:?}"
    );

    Ok(())
}

#[test]
fn imports_inside_a_test_module_can_be_fixed() -> Result<()> {
    let source = "pub struct Value;\n#[cfg(test)] mod tests {\n    use crate::Value;\n    use std::fmt;\n}\n";
    let directory = support::fixture(source)?;
    let output = support::run_with_args(&directory, true, &["kiki_import_module_order"], &["--tests"])?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        support::source(&directory)?,
        source.replace(
            "use crate::Value;\n    use std::fmt;",
            "use std::fmt;\n\nuse crate::Value;"
        )
    );

    Ok(())
}

#[test]
fn resolved_aliases_globs_and_underscore_imports_keep_group_order_and_behavior() -> Result<()> {
    let imports = "use child::{self as LocalModule, Value as LocalValue};\nuse child::ChildTrait as _;\nuse crate::sibling::*;\nuse LocalModule::Value as SecondValue;\nmod sibling;\nmod child;\nuse library::fmt;\nuse library::fmt::Debug as _;\nuse std as library;\nuse core::option::Option as Optional;\nuse library::cmp::*;\n";
    let sorted = "use library::fmt;\nuse library::fmt::Debug as _;\nuse std as library;\nuse core::option::Option as Optional;\nuse library::cmp::*;\n\nmod sibling;\nmod child;\n\nuse child::{self as LocalModule, Value as LocalValue};\nuse child::ChildTrait as _;\nuse crate::sibling::*;\nuse LocalModule::Value as SecondValue;\n";
    let source = [
        imports,
        r"
fn resolve(_: LocalValue, _: SecondValue, _: Sibling) -> Optional<Ordering> {
    let _: fmt::Result = Ok(());
    Some(Ordering::Equal)
}
#[test] fn behavior() {
    assert_eq!(resolve(LocalValue, SecondValue, Sibling), Some(Ordering::Equal));
    assert_eq!(LocalValue::marker(), 7);
}
",
    ]
    .concat();

    let directory = support::fixture_with(
        &source,
        &[
            (
                "src/child.rs",
                "pub struct Value;\npub trait ChildTrait { fn marker() -> u8 { 7 } }\nimpl ChildTrait for Value {}\n",
            ),
            ("src/sibling.rs", "pub struct Sibling;\n"),
        ],
    )?;

    for fix in [false, true] {
        let output = support::run(&directory, fix, "kiki_import_module_order")?;
        assert!(output.status.success(), "{output:?}");
        if !fix {
            assert_eq!(
                support::diagnostics(&output, "kiki_import_module_order").len(),
                1,
                "{output:?}"
            );
        }

        support::behavior(&directory)?;
    }

    assert_eq!(support::source(&directory)?, source.replacen(imports, sorted, 1));
    let output = support::run(&directory, false, "kiki_import_module_order")?;
    assert!(output.status.success(), "{output:?}");
    assert!(!support::has_lint(&output, "kiki_import_module_order"), "{output:?}");
    Ok(())
}

#[test]
fn real_compiler_barriers_preserve_sources_without_suppressing_diagnostics() -> Result<()> {
    for source in [
        "use crate::child::Value;\n// attached\nmod child;\nuse std::fmt;\n",
        "use crate::child::Value;\nmod child;\nuse std::fmt; // edge\n",
        "#[allow(unused_imports)] use crate::child::Value;\nmod child;\nuse std::fmt;\n",
        "use crate::child::Value;\nfn barrier() {}\nmod child;\nuse std::fmt;\n",
        "use crate::child::Value;\n#[macro_use] mod child;\nuse std::fmt;\nfn invoke() { child_macro!(); }\n",
        "use {std::fmt::Debug as _, crate::child::ChildTrait as _};\nmod child;\n",
    ] {
        let directory = support::fixture_with(
            source,
            &[(
                "src/child.rs",
                "macro_rules! child_macro { () => {} }\npub struct Value;\npub trait ChildTrait {}\n",
            )],
        )?;

        let output = support::run(&directory, true, "kiki_import_module_order")?;
        assert!(output.status.success(), "{output:?}");
        assert!(support::has_lint(&output, "kiki_import_module_order"), "{output:?}");
        assert_eq!(support::source(&directory)?, source);
    }

    Ok(())
}
