use super::support;

#[test]
fn imports_qualified_anyhow_macro_and_context_trait() -> anyhow::Result<()> {
    let source = r#"pub fn check(valid: bool) -> anyhow::Result<String> {
    anyhow::ensure!(valid, "invalid");
    anyhow::Context::context(std::fs::read_to_string("missing"), "read config")
}
"#;

    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(
        fixed,
        source
            .replace("anyhow::ensure!", "ensure!")
            .replace("anyhow::Context::", "Context::")
            .replace(
                "    ensure!",
                "    use anyhow::ensure;\n    use anyhow::Context;\n    ensure!"
            )
    );
    Ok(())
}

#[test]
fn preserves_anyhow_error_and_result_namespaces_while_importing_macros() -> anyhow::Result<()> {
    let source = r#"pub fn error() -> anyhow::Error { anyhow::anyhow!("error") }
pub fn result() -> anyhow::Result<()> { Ok(()) }
"#;

    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(
        fixed,
        source.replace("{ anyhow::anyhow!", "{ \nuse anyhow::anyhow;\nanyhow!")
    );
    Ok(())
}

#[test]
fn adds_standalone_macro_imports_without_extending_an_existing_group() -> anyhow::Result<()> {
    let source = r#"use anyhow::{Context, anyhow};
pub fn check(valid: bool) -> anyhow::Result<()> {
    anyhow::ensure!(valid, "invalid");
    anyhow::bail!(anyhow::anyhow!("example"));
}
"#;

    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(
        fixed,
        source
            .replace("anyhow::ensure!", "ensure!")
            .replace("anyhow::bail!", "bail!")
            .replace(
                "    ensure!",
                "    use anyhow::ensure;\n    use anyhow::bail;\n    ensure!"
            )
    );
    Ok(())
}

#[test]
fn keeps_new_imports_local_when_an_existing_group_is_between_calls() -> anyhow::Result<()> {
    let source = r#"pub fn before() -> anyhow::Result<()> { anyhow::bail!("before") }
use anyhow::{ensure};
pub fn after() -> anyhow::Result<()> { ensure!(true); anyhow::bail!("after") }
"#;

    let (_, fixed) = support::fixed(source, &support::anyhow())?;
    assert_eq!(
        fixed,
        source
            .replace("anyhow::bail!(\"before\")", "\nuse anyhow::bail;\nbail!(\"before\")")
            .replace("ensure!(true);", "\nuse anyhow::bail;\nensure!(true);")
            .replace("anyhow::bail!(\"after\")", "bail!(\"after\")")
    );

    Ok(())
}

#[test]
fn accepts_context_import_used_only_through_extension_methods() -> anyhow::Result<()> {
    support::accepted(
        r#"use anyhow::Context;
pub fn read() -> anyhow::Result<String> {
    std::fs::read_to_string("missing").context("read config")
}
"#,
        &support::anyhow(),
    )
}

#[test]
fn diagnoses_context_alias_used_only_through_extension_methods() -> anyhow::Result<()> {
    support::diagnosed(
        r#"use anyhow::Context as ErrorContext;
pub fn read() -> anyhow::Result<String> {
    std::fs::read_to_string("missing").context("read config")
}
"#,
        &support::anyhow(),
    )
}

#[test]
fn diagnoses_renamed_anyhow_macros() -> anyhow::Result<()> {
    support::diagnosed(
        r#"use anyhow::bail as fail;
pub fn run() -> anyhow::Result<()> { fail!("example") }
"#,
        &support::anyhow(),
    )
}

#[test]
fn leaves_macro_generated_paths_untouched() -> anyhow::Result<()> {
    support::accepted(
        r#"macro_rules! error { () => { anyhow::anyhow!("generated") }; }
pub fn generated() -> anyhow::Error { error!() }
"#,
        &support::anyhow(),
    )
}
