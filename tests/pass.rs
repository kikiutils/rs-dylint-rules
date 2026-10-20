use anyhow::Result;

mod support;

// All rules share one lint pass; enabling them together must not change any rule's output.
#[test]
fn inline_module_diagnostics_match_when_rules_are_enabled_together() -> Result<()> {
    let directory = support::fixture(
        r#"pub mod nested {
    use crate::nested::Value as Local;
    use std::fmt;
    #[derive(Eq, PartialEq)] pub struct Value;
    pub fn error() -> anyhow::Error { anyhow::anyhow!("error") }
}
"#,
    )?;

    support::anyhow(&directory)?;
    let combined = support::run_lints(&directory, false, support::LINTS)?;
    assert!(combined.status.success(), "{combined:?}");
    for lint in support::LINTS {
        let single = support::run(&directory, false, lint)?;
        assert!(single.status.success(), "{single:?}");
        let signature = |output: &std::process::Output| {
            support::diagnostics(output, lint)
                .into_iter()
                .map(|message| {
                    (
                        message["message"]["message"].clone(),
                        message["message"]["spans"].clone(),
                    )
                })
                .collect::<Vec<_>>()
        };

        let expected = signature(&single);
        assert!(!expected.is_empty(), "{lint}: {single:?}");
        assert_eq!(signature(&combined), expected, "{lint}");
    }

    Ok(())
}
