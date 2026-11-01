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
