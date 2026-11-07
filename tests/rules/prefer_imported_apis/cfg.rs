use super::support;

#[test]
fn does_not_hoist_imports_out_of_conditional_statements() -> anyhow::Result<()> {
    for attribute in [
        r#"#[cfg(feature = "timers")]"#,
        r#"#[cfg_attr(not(feature = "timers"), cfg(any()))]"#,
    ] {
        let source = format!(
            "pub fn run() {{\n    {attribute}\n    let delay = tokio::time::sleep(std::time::Duration::ZERO);\n}}\n"
        );

        let directory = support::fixture(&source, &support::timers_feature())?;
        let output = crate::support::run(&directory, true, support::LINT)?;
        assert!(output.status.success(), "{output:?}");
        assert!(crate::support::has_lint(&output, support::LINT), "{output:?}");
        assert_eq!(crate::support::source(&directory)?, source, "{output:?}");
        support::check_without_timers(&directory)?;
    }

    Ok(())
}

#[test]
fn keeps_new_imports_inside_conditional_initializer_blocks() -> anyhow::Result<()> {
    let source = r#"pub fn run() {
    #[cfg(feature = "timers")]
    let delay = { tokio::time::sleep(std::time::Duration::ZERO) };
}
"#;

    let (directory, fixed) = support::fixed(source, &support::timers_feature())?;
    assert_eq!(
        fixed,
        source.replace("{ tokio::time::sleep", "{ \nuse tokio::time::sleep;\nsleep")
    );

    support::check_without_timers(&directory)
}

#[test]
fn reuses_existing_bindings_inside_conditional_statements() -> anyhow::Result<()> {
    let source = r#"#[cfg(feature = "timers")]
pub fn run() {
    use tokio::time::sleep;
    #[cfg(feature = "timers")]
    let delay = tokio::time::sleep(std::time::Duration::ZERO);
}
"#;

    let (directory, fixed) = support::fixed(source, &support::timers_feature())?;
    assert_eq!(
        fixed,
        source.replace("let delay = tokio::time::sleep", "let delay = sleep")
    );

    support::check_without_timers(&directory)
}
