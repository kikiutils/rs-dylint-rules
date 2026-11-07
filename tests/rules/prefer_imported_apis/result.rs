use super::support;

#[test]
fn accepts_imported_anyhow_result_alongside_standard_result_namespace() -> anyhow::Result<()> {
    support::accepted(
        r"use std::result;
use anyhow::Result;
pub fn application() -> Result<()> { Ok(()) }
pub fn standard() -> result::Result<(), u8> { Ok(()) }
",
        &support::anyhow(),
    )
}

#[test]
fn qualifies_prelude_result() -> anyhow::Result<()> {
    let source = "pub fn standard() -> Result<(), u8> { Ok(()) }\n";
    let (_, fixed) = support::fixed(source, "")?;
    assert_eq!(
        fixed,
        "use std::result;\npub fn standard() -> result::Result<(), u8> { Ok(()) }\n"
    );
    Ok(())
}

#[test]
fn qualifies_core_result_without_std() -> anyhow::Result<()> {
    let source = "#![no_std]\npub fn standard() -> Result<(), u8> { Ok(()) }\n";
    let (_, fixed) = support::fixed(source, "")?;
    assert_eq!(
        fixed,
        "#![no_std]\nuse core::result;\npub fn standard() -> result::Result<(), u8> { Ok(()) }\n"
    );
    Ok(())
}

#[test]
fn does_not_import_result_when_a_type_parameter_conflicts() -> anyhow::Result<()> {
    support::unchanged("pub fn standard<result>() -> Result<(), result> { Ok(()) }\n", "")
}
