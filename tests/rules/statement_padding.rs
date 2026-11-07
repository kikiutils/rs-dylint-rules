use anyhow::Result;

use crate::support;

#[test]
fn fixes_statements_items_and_cfg_disabled_code_and_is_idempotent() -> Result<()> {
    let source = r"fn one() {} // trailing
fn two() {}
#[cfg(any())]
fn inactive() {
    let v = vec![
        1,
    ];
    drop(v);
}
pub fn run() {
    let x = String::new(
    ); /* trailing

    comment */
    drop(x);
}
";

    let expected = r"fn one() {} // trailing

fn two() {}

#[cfg(any())]
fn inactive() {
    let v = vec![
        1,
    ];

    drop(v);
}

pub fn run() {
    let x = String::new(
    ); /* trailing

    comment */

    drop(x);
}
";

    for newline in ["\n", "\r\n"] {
        let directory = support::fixture(&source.replace('\n', newline))?;
        let output = support::run(&directory, false, "kiki_statement_padding")?;
        assert!(output.status.success(), "{output:?}");
        let lines: Vec<_> = support::diagnostics(&output, "kiki_statement_padding")
            .iter()
            .map(|diagnostic| diagnostic["message"]["spans"][0]["line_start"].as_u64())
            .collect();

        assert_eq!(lines, [Some(1), Some(2), Some(3), Some(5), Some(11)], "{output:?}");

        let output = support::run(&directory, true, "kiki_statement_padding")?;
        assert!(output.status.success(), "{output:?}");
        assert_eq!(support::source(&directory)?, expected.replace('\n', newline));

        let output = support::run(&directory, false, "kiki_statement_padding")?;
        assert!(output.status.success(), "{output:?}");
        assert!(
            support::diagnostics(&output, "kiki_statement_padding").is_empty(),
            "{output:?}"
        );
    }

    Ok(())
}
