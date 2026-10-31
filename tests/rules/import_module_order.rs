use anyhow::Result;

use crate::support;

const CHILD: (&str, &str) = ("src/child.rs", "pub struct Value;\n");

#[test]
fn native_diagnostics_offer_fix_and_dylint_applies_it() -> Result<()> {
    let source = "use crate::child::{/* local */ Value};\nmod child;\nuse std::{fmt, /* external */ io};\n";
    let directory = support::fixture_with(source, &[CHILD])?;
    let output = support::run(&directory, false, "kiki_import_module_order")?;
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let messages = support::diagnostics(&output, "kiki_import_module_order");
    assert!(!messages.is_empty(), "{output:?}");
    assert!(messages.iter().any(|message| {
        message["message"]["children"].as_array().is_some_and(|children| {
            children.iter().any(|child| {
                child["spans"].as_array().is_some_and(|spans| {
                    spans
                        .iter()
                        .any(|span| span["suggestion_applicability"] == "MachineApplicable")
                })
            })
        })
    }));

    assert_eq!(support::source(&directory)?, source);
    let output = support::run(&directory, true, "kiki_import_module_order")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        support::source(&directory)?,
        "use std::{fmt, /* external */ io};\n\nmod child;\n\nuse crate::child::{/* local */ Value};\n"
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
    let fixed = support::source(&directory)?;
    assert!(
        fixed
            .find("use std::fmt")
            .is_some_and(|index| index < fixed.find("use crate::child").unwrap_or(0))
    );

    assert!(fixed.contains("#[cfg(any())] mod missing;"));
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
    let fixed = support::source(&directory)?;
    assert_ne!(fixed, source);
    assert!(
        fixed
            .find("use std::fmt;")
            .zip(fixed.find("use crate::Value;"))
            .is_some_and(|(external, local)| external < local),
        "{fixed}"
    );

    Ok(())
}
