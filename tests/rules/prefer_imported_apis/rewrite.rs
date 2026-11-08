use anyhow::Result;

use super::support;

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
fn unconditional_function_imports_are_inserted_at_the_module_start() -> Result<()> {
    let source = "pub fn current() -> u32 { std::process::id() }\n";
    let (_, fixed) = support::fixed(source, "")?;
    assert_eq!(fixed, "use std::process;\npub fn current() -> u32 { process::id() }\n");
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
fn multiple_import_insertions_form_one_file_suggestion() -> Result<()> {
    let source = "pub fn current() { let _ = std::process::id(); let _ = std::env::current_dir(); }\n";
    let directory = support::fixture(source, "")?;
    let output = crate::support::run(&directory, false, support::LINT)?;
    assert!(output.status.success(), "{output:?}");
    let suggestions = crate::support::diagnostics(&output, support::LINT)
        .into_iter()
        .flat_map(|diagnostic| {
            diagnostic["message"]["children"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter(|child| {
            child["spans"]
                .as_array()
                .is_some_and(|spans| spans.iter().any(|span| span["suggested_replacement"].is_string()))
        })
        .count();

    assert_eq!(suggestions, 1, "{output:?}");
    let (_, fixed) = support::fixed(source, "")?;
    assert!(fixed.starts_with("use std::process;\nuse std::env;\n"), "{fixed}");
    Ok(())
}

#[test]
fn unused_noncanonical_imports_are_not_diagnosed_or_rewritten() -> Result<()> {
    support::accepted("use std::io::Error as IoError;\n", "")
}

#[test]
fn canonical_uses_accept_imports_declared_after_the_use() -> Result<()> {
    support::accepted("pub fn current() -> u32 { process::id() }\nuse std::process;\n", "")
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
