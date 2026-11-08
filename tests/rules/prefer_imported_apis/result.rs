use anyhow::Result;

use super::support;

#[test]
fn standard_result_import_does_not_conflict_with_display_result() -> Result<()> {
    let source = r#"use std::fmt;
use std::result::Result;
pub struct Value;
impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { formatter.write_str("value") }
}
pub fn standard() -> Result<(), u8> { Ok(()) }
"#;

    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(fixed, source.replace("-> Result<(), u8>", "-> anyhow::Result<(), u8>"));

    Ok(())
}

#[test]
fn imports_anyhow_result_for_the_standard_prelude_type() -> Result<()> {
    let source = "pub fn standard() -> Result<(), u8> { Ok(()) }\n";
    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(fixed, format!("use anyhow::Result;\n{source}"));
    Ok(())
}

#[test]
fn rewrites_standard_result_spellings_and_preserves_private_aliases() -> Result<()> {
    let source = r"use std::result::Result as StandardResult;
pub fn standard() -> std::result::Result<(), u8> { std::result::Result::Ok(()) }
pub fn core() -> core::result::Result<(), u16> { Ok(()) }
pub fn aliased() -> StandardResult<(), u32> { Ok(()) }
";

    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(
        fixed,
        format!(
            "use anyhow::Result;\n{}",
            source
                .replace("-> std::result::Result", "-> Result")
                .replace("{ std::result::Result::", "{ Result::")
                .replace("core::result::Result", "Result")
                .replace("-> StandardResult", "-> Result")
        )
    );

    Ok(())
}

#[test]
fn private_standard_result_import_forces_the_full_anyhow_path() -> Result<()> {
    let source = "use std::result::Result;\npub fn standard() -> Result<(), u8> { Ok(()) }\n";
    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(fixed, source.replace("-> Result", "-> anyhow::Result"));

    Ok(())
}

#[test]
fn grouped_standard_result_imports_force_the_full_anyhow_path() -> Result<()> {
    let source = "use std::{fmt::Debug, result::Result};\npub fn standard() -> Result<(), u8> { Ok(()) }\n";
    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(fixed, source.replace("-> Result", "-> anyhow::Result"));

    Ok(())
}

#[test]
fn enum_variant_imports_keep_the_standard_binding() -> Result<()> {
    let source = "use std::result::Result;\nuse Result::Ok;\npub fn standard() -> Result<(), u8> { Ok(()) }\n";
    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(fixed, source.replace("-> Result", "-> anyhow::Result"));
    Ok(())
}

#[test]
fn macro_generated_standard_result_imports_force_the_full_anyhow_path() -> Result<()> {
    let source = "macro_rules! imports { () => { use std::result::Result; } }\nimports!();\npub fn standard() -> Result<(), u8> { Ok(()) }\n";
    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(fixed, source.replace("-> Result", "-> anyhow::Result"));
    Ok(())
}

#[test]
fn protected_result_imports_do_not_block_unrelated_api_fixes() -> Result<()> {
    let source = "use std::result::Result; // preserve\npub fn standard() -> Result<(), u8> { Ok(()) }\npub fn pid() -> u32 { std::process::id() }\n";
    let (_, fixed) = support::rewritten(source, &support::anyhow())?;
    assert_eq!(
        fixed,
        format!(
            "use std::process;\n{}",
            source
                .replace("-> Result", "-> anyhow::Result")
                .replace("std::process::id", "process::id")
        )
    );

    Ok(())
}

#[test]
fn result_migrations_distinguish_same_definition_bindings_by_scope() -> Result<()> {
    for source in [
        "use std::result::Result;\npub fn outer() -> Result<(), u8> { Ok(()) }\nmod inner {\n    use std::result::Result; // preserve inner binding\n    pub fn inner() -> Result<(), u8> { Ok(()) }\n}\n",
        "use std::result::Result; // preserve module binding\npub fn module() -> self::Result<(), u8> { Ok(()) }\npub fn local() {\n    use std::result::Result;\n    let _: Result<(), u8> = Ok(());\n}\n",
        "use std::result::Result; // preserve module binding\npub fn outer() -> Result<(), u8> { Ok(()) }\npub fn inner() { let _: std::result::Result<(), u8> = Ok(()); }\n",
    ] {
        let (_, fixed) = support::fixed(source, &support::anyhow())?;
        let expected = source
            .replace("-> Result", "-> anyhow::Result")
            .replace("-> self::Result", "-> anyhow::Result")
            .replace("let _: Result", "let _: anyhow::Result")
            .replace("let _: std::result::Result", "let _: anyhow::Result");

        assert_eq!(fixed, expected);
    }

    Ok(())
}

#[test]
fn reuses_imported_anyhow_result_without_changing_the_error_type() -> Result<()> {
    let source = r"use anyhow::Result;
pub fn application() -> Result<()> { Ok(()) }
pub fn standard() -> std::result::Result<(), u8> { Ok(()) }
";

    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(fixed, source.replace("std::result::Result", "Result"));
    Ok(())
}

#[test]
fn conflicting_local_result_names_force_the_full_anyhow_path() -> Result<()> {
    let source = "pub struct Result;\npub fn standard() -> std::result::Result<(), u8> { Ok(()) }\n";
    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(fixed, source.replace("std::result::Result", "anyhow::Result"));
    Ok(())
}

#[test]
fn standard_result_without_an_anyhow_dependency_is_skipped() -> Result<()> {
    for source in [
        "pub fn standard() -> Result<(), u8> { Ok(()) }\n",
        "#![no_std]\npub fn standard() -> core::result::Result<(), u8> { Ok(()) }\n",
    ] {
        support::unchanged(source, "")?;
    }

    Ok(())
}

#[test]
fn renamed_anyhow_dependencies_do_not_justify_an_unavailable_crate_root() -> Result<()> {
    support::unchanged(
        "pub fn standard() -> std::result::Result<(), u8> { Ok(()) }\n",
        &format!(
            "errors={{package=\"anyhow\",version=\"{}\"}}",
            crate::support::ANYHOW_VERSION
        ),
    )
}

#[test]
fn explicit_anyhow_crate_aliases_allow_result_replacement() -> Result<()> {
    let source = "use errors as anyhow;\npub fn standard() -> std::result::Result<(), u8> { Ok(()) }\n";
    let (_, fixed) = support::fixed(
        source,
        &format!(
            "errors={{package=\"anyhow\",version=\"{}\"}}",
            crate::support::ANYHOW_VERSION
        ),
    )?;

    assert_eq!(
        fixed,
        format!(
            "use anyhow::Result;\n{}",
            source.replace("std::result::Result", "Result")
        )
    );

    Ok(())
}
