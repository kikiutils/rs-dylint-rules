use anyhow::Result;
use pathkit::{
    SyncFsOps,
    path,
};

mod support;

#[test]
fn rules_preserve_shared_module_sources_and_include_fragments() -> Result<()> {
    let directory = support::fixture(
        r#"#[path = "shared.rs"] pub mod a;
#[path = "shared.rs"] pub mod c;
pub fn expression() -> u8 { include!("expression.rs") }
"#,
    )?;
    let root = path!(directory.path()) / "src";
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
    (&root / "shared.rs").write_sync(shared)?;
    (&root / "expression.rs").write_sync(fragment)?;
    let manifest = path!(directory.path()) / "Cargo.toml";
    manifest.write_sync(format!(
        "{}\n[dependencies]\nanyhow=\"=1.0.104\"\n",
        manifest.read_to_string_sync()?
    ))?;
    for lint in [
        "kiki_import_module_order",
        "kiki_import_path_prefix",
        "kiki_grouped_derive_order",
        "kiki_prefer_imported_apis",
        "kiki_statement_padding",
    ] {
        let output = support::run(&directory, true, lint)?;
        assert!(output.status.success(), "{lint}: {output:?}");
        assert!(!support::diagnostics(&output, lint).is_empty(), "{lint}: {output:?}");
        assert_eq!((&root / "shared.rs").read_to_string_sync()?, shared, "{lint}");
        assert_eq!((&root / "expression.rs").read_to_string_sync()?, fragment, "{lint}");
    }
    Ok(())
}
