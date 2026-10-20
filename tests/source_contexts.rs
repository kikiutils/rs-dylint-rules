use anyhow::Result;

mod support;

#[test]
fn rules_preserve_shared_module_sources_and_include_fragments() -> Result<()> {
    let shared = r#"#[derive(Eq, PartialEq)] pub struct Value;
use crate::a::Value as Imported;
use std::fmt;
pub fn origin() -> &'static str { std::any::type_name::<Imported>() }
pub fn error() -> anyhow::Error { anyhow::anyhow!("shared") }
"#;

    let fragment = r#"{ use crate::a::Value; use std::fmt;
#[derive(Eq, PartialEq)] struct Nested;
let _ = anyhow::anyhow!("fragment"); 0 }
"#;

    let directory = support::fixture_with(
        r#"#[path = "shared.rs"] pub mod a;
#[path = "shared.rs"] pub mod c;
pub fn expression() -> u8 { include!("expression.rs") }
"#,
        &[("src/shared.rs", shared), ("src/expression.rs", fragment)],
    )?;

    support::anyhow(&directory)?;
    for lint in support::LINTS {
        let output = support::run(&directory, true, lint)?;
        assert!(output.status.success(), "{lint}: {output:?}");
        assert!(support::has_lint(&output, lint), "{lint}: {output:?}");
        assert_eq!(support::read(&directory, "src/shared.rs")?, shared, "{lint}");
        assert_eq!(support::read(&directory, "src/expression.rs")?, fragment, "{lint}");
    }

    Ok(())
}

#[test]
fn bom_offsets_point_at_the_original_tokens_and_fixes_preserve_the_prefix() -> Result<()> {
    for newline in ["\n", "\r\n"] {
        let source = format!("\u{feff}// 型別{newline}#[derive(Eq, PartialEq)]{newline}pub struct Value;{newline}");
        let directory = support::fixture(&source)?;
        let output = support::run(&directory, false, "kiki_grouped_derive_order")?;
        assert!(output.status.success(), "{output:?}");
        let diagnostics = support::diagnostics(&output, "kiki_grouped_derive_order");
        assert_eq!(diagnostics.len(), 1, "{output:?}");
        let primary = diagnostics[0]["message"]["spans"]
            .as_array()
            .and_then(|spans| spans.iter().find(|span| span["is_primary"] == true));

        assert_eq!(
            primary.and_then(|span| span["byte_start"].as_u64()),
            source.find("#[derive").and_then(|start| u64::try_from(start).ok()),
            "{diagnostics:?}"
        );

        let output = support::run(&directory, true, "kiki_grouped_derive_order")?;
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            support::source(&directory)?,
            source.replace("Eq, PartialEq", "PartialEq, Eq")
        );
    }

    Ok(())
}

#[test]
fn a_source_changed_by_a_macro_warns_once_without_failing_the_build() -> Result<()> {
    let directory = support::fixture_with(
        "#[change::source]\npub fn first() {}\npub fn second() {}\n",
        &[
            (
                "change/Cargo.toml",
                "[package]\nname=\"change\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\nproc-macro=true\n",
            ),
            (
                "change/src/lib.rs",
                r#"
#[proc_macro_attribute]
pub fn source(_: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib.rs");
    std::fs::write(file, "pub fn changed() {}\n").unwrap();
    item
}
"#,
            ),
        ],
    )?;

    support::dependency(&directory, "change={path=\"change\"}")?;
    let output = support::run_lints(&directory, false, support::LINTS)?;
    assert!(output.status.success(), "{output:?}");
    let warnings = support::messages(&output)
        .into_iter()
        .filter(|message| {
            message["message"]["level"] == "warning"
                && message["message"]["message"]
                    .as_str()
                    .is_some_and(|text| text.contains("Source changed since rustc loaded"))
        })
        .count();

    assert_eq!(warnings, 1, "{output:?}");
    Ok(())
}
