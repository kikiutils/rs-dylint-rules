use anyhow::Result;

use super::support;

#[test]
fn renamed_dependencies_without_the_canonical_crate_root_are_skipped() -> Result<()> {
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
    assert!(!crate::support::has_lint(&output, support::LINT), "{output:?}");
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
    assert!(!crate::support::has_lint(&output, support::LINT), "{output:?}");

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
        format!(
            "use std::process;\n{}",
            source.replace("std::process::id", "process::id")
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
fn accepts_direct_imports_below_their_use_without_moving_them() -> Result<()> {
    support::accepted(
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
    support::accepted(
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
    assert_eq!(
        fixed,
        format!("use std::process;\n{}", source.replace("{ id()", "{ process::id()"))
    );

    Ok(())
}

#[test]
fn namespace_conflicts_use_full_paths_for_modules_and_generic_parameters() -> Result<()> {
    for (source, expected) in [
        (
            "use std::process::id;\npub fn current<process>() -> u32 { id() }\n",
            "use std::process::id;\npub fn current<process>() -> u32 { std::process::id() }\n",
        ),
        (
            r#"use std::io::{Error as IoError, Result as IoResult};
use std::process::id;
mod io {}
mod process {}
pub fn error() -> IoResult<()> { Err(IoError::other("example")) }
pub fn pid() -> u32 { id() }
"#,
            r#"use std::io::{Error as IoError, Result as IoResult};
use std::process::id;
mod io {}
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
fn permits_imports_in_glob_sources_when_consumers_have_no_conflicting_references() -> Result<()> {
    let source = "mod producer {\n    pub fn result() -> std::io::Result<()> { Ok(()) }\n    mod consumer { use super::*; pub fn run() { let _ = result(); } }\n}\n";
    let (_, fixed) = support::fixed(source, "")?;
    assert_eq!(
        fixed,
        source
            .replace("mod producer {", "mod producer {use std::io;\n")
            .replace("std::io::Result", "io::Result")
    );

    Ok(())
}

#[test]
fn rejects_imports_whose_crate_root_is_shadowed() -> Result<()> {
    support::accepted(
        "pub fn current() { mod std {} let _ = ::std::env::current_dir(); }\n",
        "",
    )
}

#[test]
fn module_imports_do_not_conflict_with_an_external_module_file() -> Result<()> {
    let source = "use std::io::Error as IoError;\nmod io;\npub fn error() -> IoError { IoError::other(\"example\") }\n";
    let directory = crate::support::fixture_with(source, &[("src/io.rs", "pub struct Local;\n")])?;
    let output = crate::support::run(&directory, true, support::LINT)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        crate::support::source(&directory)?,
        source
            .replace("-> IoError", "-> std::io::Error")
            .replace("{ IoError::", "{ std::io::Error::")
    );

    support::clean(&directory)
}

#[test]
fn new_module_imports_preserve_references_in_super_glob_consumers() -> Result<()> {
    let source = "mod provider { pub mod io { pub struct Local; } }\nmod parent {\n    use crate::provider::*;\n    use std::io::Error as IoError;\n    mod consumer {\n        use super::*;\n        pub fn local() -> io::Local { io::Local }\n    }\n    pub fn error() -> IoError { IoError::other(\"example\") }\n}\n";
    let (_, fixed) = support::fixed(source, "")?;
    // An explicit std::io import would capture the consumer's glob-resolved io.
    assert_eq!(
        fixed,
        source
            .replace("-> IoError", "-> std::io::Error")
            .replace("{ IoError::", "{ std::io::Error::")
    );

    Ok(())
}

#[test]
fn inactive_same_named_modules_block_new_imports() -> Result<()> {
    support::accepted(
        "#[cfg(any())]\nmod io;\npub fn result() -> std::io::Result<()> { Ok(()) }\n",
        "",
    )
}

#[test]
fn inactive_crate_root_definitions_prevent_import_rewrites() -> Result<()> {
    support::accepted(
        "use ::std::process::id as pid;\n#[cfg(any())]\nmod std {}\npub fn current() -> u32 { pid() }\n",
        "",
    )
}

#[test]
fn inactive_grouped_root_aliases_prevent_import_rewrites() -> Result<()> {
    support::accepted(
        "use ::std::process::id as pid;\n#[cfg(any())]\nuse ::std::{ffi as std};\npub fn current() -> u32 { pid() }\n",
        "",
    )
}

#[test]
fn equivalent_explicit_crate_roots_allow_canonical_imports() -> Result<()> {
    for declaration in ["use ::std;", "use ::std as std;", "use ::std::{self};"] {
        let source = format!("{declaration}\npub fn current() -> u32 {{ std::process::id() }}\n");
        let (_, fixed) = support::fixed(&source, "")?;
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
fn function_local_crate_aliases_do_not_justify_module_imports() -> Result<()> {
    support::unchanged(
        "pub fn error() -> errors::Error { use errors as anyhow; anyhow::anyhow!(\"example\") }\n",
        &format!(
            "errors={{package=\"anyhow\",version=\"{}\"}}",
            crate::support::ANYHOW_VERSION
        ),
    )
}

#[test]
fn inactive_grouped_self_imports_prevent_duplicate_namespace_bindings() -> Result<()> {
    support::accepted(
        "#[cfg(any())]\nuse std::io::{self};\npub fn error() -> std::io::Error { std::io::Error::other(\"example\") }\n",
        "",
    )
}

#[test]
fn reusing_active_imports_still_checks_inactive_same_named_definitions() -> Result<()> {
    support::accepted(
        "#[cfg(all())]\nuse std::io;\n#[cfg(any())]\nmod io { pub struct Local; }\npub fn error() -> std::io::Error { std::io::Error::other(\"example\") }\n",
        "",
    )
}

#[test]
fn ancestor_textual_macros_do_not_capture_rewritten_calls() -> Result<()> {
    let source = r#"macro_rules! anyhow { ($($tokens:tt)*) => { "wrong" }; }
mod child {
    pub fn error() -> anyhow::Error { anyhow::anyhow!("expected") }
}
#[test]
fn preserves_error_message() { assert_eq!(child::error().to_string(), "expected"); }
"#;

    let directory = support::fixture(source, &support::anyhow())?;
    let output = crate::support::run(&directory, true, support::LINT)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(crate::support::source(&directory)?, source);
    support::clean(&directory)?;
    support::behavior(&directory)
}

#[test]
fn new_imports_do_not_capture_qualified_glob_binding_references() -> Result<()> {
    for prefix in ["self", "crate"] {
        support::accepted(
            &format!(
                "mod provider {{ pub mod io {{ pub struct Local; }} }}\nuse provider::*;\npub fn local() -> {prefix}::io::Local {{ {prefix}::io::Local }}\npub fn error() -> std::io::Error {{ std::io::Error::other(\"example\") }}\n"
            ),
            "",
        )?;
    }

    Ok(())
}

#[test]
fn textual_macros_block_new_macro_imports() -> Result<()> {
    support::accepted(
        "pub fn error() -> anyhow::Error { macro_rules! anyhow { () => { 1 }; } let _ = anyhow!(); anyhow::anyhow!(\"example\") }\n",
        &support::anyhow(),
    )
}

#[test]
fn block_imports_do_not_capture_glob_references_in_earlier_statements() -> Result<()> {
    support::accepted(
        "mod custom { pub fn sleep() {} }\nuse custom::*;\n#[cfg(all())]\npub async fn run() { sleep(); tokio::time::sleep(std::time::Duration::ZERO).await; }\n",
        &support::tokio(),
    )
}

#[test]
fn indirect_dependencies_do_not_become_unresolvable_import_roots() -> Result<()> {
    let source = "pub fn value(_: bridge::Value) {}\n";
    let directory = crate::support::fixture_with(
        source,
        &[
            (
                "bridge/Cargo.toml",
                "[package]\nname=\"bridge\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[dependencies]\nserde_json=\"=1.0.154\"\n",
            ),
            ("bridge/src/lib.rs", "pub use serde_json::Value;\n"),
        ],
    )?;

    crate::support::dependency(&directory, "bridge={path=\"bridge\"}")?;
    let output = crate::support::run(&directory, true, support::LINT)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(crate::support::source(&directory)?, source);
    support::clean(&directory)
}
