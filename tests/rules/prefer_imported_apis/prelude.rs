use anyhow::Result;

use super::support;

#[test]
fn no_std_does_not_expose_std_roots_loaded_by_dependencies() -> Result<()> {
    support::unchanged(
        "#![no_std]\npub use anyhow::Error;\npub fn take(value: &mut u8) -> u8 { core::mem::take(value) }\n",
        &support::anyhow(),
    )
}

#[test]
fn active_cfg_attr_no_std_does_not_expose_std_roots_loaded_by_dependencies() -> Result<()> {
    support::unchanged(
        "#![cfg_attr(all(), no_std)]\npub use anyhow::Error;\npub fn take(value: &mut u8) -> u8 { core::mem::take(value) }\n",
        &support::anyhow(),
    )
}

#[test]
fn disabled_implicit_prelude_does_not_prove_bare_crate_roots() -> Result<()> {
    for source in [
        r#"#[no_implicit_prelude]
pub mod m {
    use ::anyhow as errors;
    pub fn error() -> errors::Error { errors::Error::msg("example") }
}
"#,
        r#"#![no_implicit_prelude]
use ::anyhow as errors;
pub fn error() -> errors::Error { errors::Error::msg("example") }
"#,
        r#"#![no_implicit_prelude]
extern crate anyhow;
pub mod m {
    use ::anyhow as errors;
    pub fn error() -> errors::Error { errors::Error::msg("example") }
}
"#,
        r#"#[no_implicit_prelude]
pub mod outer {
    pub mod inner {
        use ::anyhow as errors;
        pub fn error() -> errors::Error { errors::Error::msg("example") }
    }
}
"#,
        r#"#[cfg_attr(all(), no_implicit_prelude)]
pub mod m {
    use ::anyhow as errors;
    pub fn error() -> errors::Error { errors::Error::msg("example") }
}
"#,
        r#"#[no_implicit_prelude]
pub mod m {
    pub fn error() -> ::anyhow::Error { ::anyhow::Error::msg("example") }
}
"#,
    ] {
        support::unchanged(source, &support::anyhow())?;
    }

    Ok(())
}

#[test]
fn explicit_canonical_import_remains_available_without_the_prelude() -> Result<()> {
    let source = r#"#[no_implicit_prelude]
pub mod m {
    use ::anyhow;
    use ::anyhow as errors;
    pub fn error() -> errors::Error { errors::Error::msg("example") }
}
"#;

    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(fixed, source.replace("errors::Error", "anyhow::Error"));

    Ok(())
}

#[test]
fn ordinary_and_inactive_conditional_preludes_allow_bare_crate_roots() -> Result<()> {
    for attribute in ["", "#[cfg_attr(any(), no_implicit_prelude)]\n"] {
        let source = format!(
            "{attribute}pub mod m {{\n    use ::anyhow as errors;\n    pub fn error() -> errors::Error {{ errors::Error::msg(\"example\") }}\n}}\n"
        );

        let (_, fixed) = support::fixed(&source, &support::anyhow())?;
        assert_eq!(fixed, source.replace("errors::Error", "anyhow::Error"));
    }

    Ok(())
}
