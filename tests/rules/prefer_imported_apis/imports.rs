use anyhow::Result;

use super::support;

#[test]
fn direct_imports_use_the_canonical_dependency_route() -> Result<()> {
    let source = r#"use anyhow::Result;
mod errors { pub use anyhow::Context; }
use errors::{Context};
pub fn run() -> Result<()> {
    mod errors { pub use anyhow::bail; }
    errors::bail!("example")
}
"#;

    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(
        fixed,
        format!("use anyhow::bail;\n{}", source.replace("errors::bail!", "bail!"))
    );

    Ok(())
}

#[test]
fn imports_a_multiline_raw_function_path_without_a_redundant_alias() -> Result<()> {
    let source = r"use std::time::Duration;
pub async fn run() {
    tokio ::
        time ::
        r#sleep(Duration::from_millis(1)).await;
}
";

    let (_, fixed) = support::fixed(source, &support::tokio())?;
    assert_eq!(
        fixed,
        format!(
            "use tokio::time::sleep;\n{}",
            source.replace("tokio ::\n        time ::\n        r#sleep", "sleep")
        )
    );

    Ok(())
}

#[test]
fn accepts_redundant_extern_crate_aliases_used_by_managed_apis() -> Result<()> {
    support::accepted(
        r#"extern crate anyhow as anyhow;
pub fn error() -> anyhow::Error { anyhow::Error::msg("example") }
"#,
        &support::anyhow(),
    )
}

#[test]
fn accepts_extern_crate_aliases_used_only_by_unmanaged_apis() -> Result<()> {
    support::accepted(
        r"extern crate std as standard;
pub fn map() -> standard::collections::BTreeMap<u8, u8> { standard::collections::BTreeMap::new() }
",
        "",
    )
}

#[test]
fn private_alias_uses_are_rewritten_without_changing_grouped_imports() -> Result<()> {
    let source = "use serde_json::{Value as JsonValue,\n    // Number import comment.\n    Number};\npub fn value(_: JsonValue, _: Number) {}\n";
    let (_, fixed) = support::fixed(source, "serde_json=\"=1.0.154\"")?;
    assert_eq!(
        fixed,
        source
            .replace("_: JsonValue", "_: serde_json::Value")
            .replace("_: Number", "_: serde_json::Number")
    );

    Ok(())
}

#[test]
fn alias_imports_referenced_by_inactive_code_or_macro_tokens_are_preserved() -> Result<()> {
    for other in [
        "#[cfg(any())] pub fn inactive(_: JsonValue) {}",
        "pub const NAME: &str = stringify!(JsonValue);",
    ] {
        let source = format!("use serde_json::Value as JsonValue;\npub fn active(_: JsonValue) {{}}\n{other}\n");
        let (_, fixed) = support::rewritten(&source, "serde_json=\"=1.0.154\"")?;
        assert_eq!(
            fixed,
            source.replace("pub fn active(_: JsonValue)", "pub fn active(_: serde_json::Value)")
        );
    }

    Ok(())
}

#[test]
fn public_reexports_and_import_owned_trailing_comments_are_preserved() -> Result<()> {
    let anyhow = support::anyhow();
    for (source, dependencies, expected) in [
        (
            "pub use serde_json::Value as JsonValue;\npub fn active(_: JsonValue) {}\n",
            "serde_json=\"=1.0.154\"",
            "pub use serde_json::Value as JsonValue;\npub fn active(_: serde_json::Value) {}\n",
        ),
        (
            "use serde_json::Value as JsonValue; // import explanation\npub fn active(_: JsonValue) {}\n",
            "serde_json=\"=1.0.154\"",
            "use serde_json::Value as JsonValue; // import explanation\npub fn active(_: serde_json::Value) {}\n",
        ),
        (
            "pub use std::result::Result as PublicResult;\npub fn standard() -> PublicResult<(), u8> { Ok(()) }\n",
            anyhow.as_str(),
            "use anyhow::Result;\npub use std::result::Result as PublicResult;\npub fn standard() -> Result<(), u8> { Ok(()) }\n",
        ),
    ] {
        let (_, fixed) = support::rewritten(source, dependencies)?;
        assert_eq!(fixed, expected);
    }

    Ok(())
}

#[test]
fn existing_imports_used_through_other_module_paths_are_preserved() -> Result<()> {
    let parent = "use serde_json::Value as JsonValue;\npub fn active(_: JsonValue) {}\nmod child;\n";
    for child in [
        "pub(super) use super::JsonValue;\npub fn value(_: JsonValue) {}\n",
        "pub(super) use super::JsonValue::Null;\npub fn value() -> serde_json::Value { Null }\n",
        "pub(super) use super::{JsonValue::Null};\npub fn value() -> serde_json::Value { Null }\n",
        "pub(super) use super::JsonValue::{Null};\npub fn value() -> serde_json::Value { Null }\n",
    ] {
        let directory = crate::support::fixture_with(
            "mod parent;\n",
            &[("src/parent.rs", parent), ("src/parent/child.rs", child)],
        )?;

        crate::support::dependency(&directory, "serde_json=\"=1.0.154\"")?;
        let output = crate::support::run(&directory, true, support::LINT)?;
        assert!(output.status.success(), "{output:?}");
        let fixed = crate::support::read(&directory, "src/parent.rs")?;
        // The child's retained re-export still depends on the original private binding.
        assert_eq!(
            fixed,
            parent.replace("pub fn active(_: JsonValue)", "pub fn active(_: serde_json::Value)")
        );

        // Successful cargo-fix status alone is insufficient: it can roll back invalid edits.
        let output = crate::support::run(&directory, false, support::LINT)?;
        assert!(output.status.success(), "{output:?}");
    }

    Ok(())
}

#[test]
fn canonical_direct_imports_preserve_existing_namespace_aliases() -> Result<()> {
    let source = r#"use anyhow as a;
pub fn fail() -> a::Result<()> { a::bail!("example") }
"#;

    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(
        fixed,
        format!(
            "use anyhow::Result;\nuse anyhow::bail;\n{}",
            source
                .replace("-> a::Result", "-> Result")
                .replace("{ a::bail!", "{ bail!")
        )
    );

    Ok(())
}

#[test]
fn canonical_direct_imports_preserve_explicit_ancestor_aliases() -> Result<()> {
    for path in ["super::a", "crate::a"] {
        let source = format!(
            "use anyhow as a;\npub fn root() -> a::Error {{ a::Error::msg(\"example\") }}\nmod child {{\n    pub fn fail() -> anyhow::Result<()> {{ {path}::bail!(\"example\") }}\n}}\n"
        );

        let (_, fixed) = support::fixed(&source, &support::anyhow())?;
        assert_eq!(
            fixed,
            source
                .replace("a::Error", "anyhow::Error")
                .replace("mod child {", "mod child {use anyhow::Result;\nuse anyhow::bail;\n")
                .replace("-> anyhow::Result", "-> Result")
                .replace(&format!("{path}::bail!"), "bail!")
        );
    }

    Ok(())
}

#[test]
fn cross_file_macro_routes_keep_parent_aliases() -> Result<()> {
    for (dependencies, parent, child, original, canonical) in [
        (
            support::anyhow(),
            "mod child;\nuse anyhow as a;\npub fn error() -> a::Error { a::Error::msg(\"example\") }\n",
            "pub fn fail() -> Result<(), anyhow::Error> { super::a::bail!(\"example\") }\n",
            "a::Error",
            "anyhow::Error",
        ),
        (
            "serde_json=\"=1.0.154\"".to_owned(),
            "mod child;\nuse serde_json as j;\npub fn value(_: j::Value) {}\n",
            "pub fn value() -> serde_json::Value { super::j::json!({\"x\": 1}) }\n",
            "j::Value",
            "serde_json::Value",
        ),
        (
            "tokio={version=\"=1.54.1\",features=[\"rt\",\"macros\",\"io-util\"]}".to_owned(),
            "mod child;\nuse tokio as runtime;\npub fn stream() -> runtime::io::DuplexStream { runtime::io::duplex(32).0 }\n",
            "#[super::runtime::main(flavor=\"current_thread\")]\npub async fn child() {}\n",
            "runtime::io",
            "tokio::io",
        ),
        (
            "tokio={version=\"=1.54.1\",features=[\"rt\",\"macros\",\"io-util\"]}".to_owned(),
            "mod child;\nuse tokio as runtime;\npub fn stream() -> runtime::io::DuplexStream { runtime::io::duplex(32).0 }\n",
            "#[cfg_attr(all(), super::runtime::main(flavor=\"current_thread\"))]\npub async fn child() {}\n",
            "runtime::io",
            "tokio::io",
        ),
        (
            "confique={version=\"=0.4.0\",features=[\"toml\"]}".to_owned(),
            "mod child;\nuse confique as config;\npub fn options() -> config::toml::FormatOptions { config::toml::FormatOptions::default() }\n",
            "#[derive(super::config::Config)]\npub struct Settings { enabled: bool }\n",
            "config::toml",
            "confique::toml",
        ),
    ] {
        let directory = crate::support::fixture_with(
            "mod parent;\n",
            &[("src/parent.rs", parent), ("src/parent/child.rs", child)],
        )?;

        crate::support::dependency(&directory, &dependencies)?;
        let output = crate::support::run(&directory, true, support::LINT)?;
        assert!(output.status.success(), "{output:?}");
        let fixed = crate::support::read(&directory, "src/parent.rs")?;
        assert_eq!(fixed, parent.replace(original, canonical));
        support::clean(&directory)?;
    }

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
