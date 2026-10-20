use anyhow::Result;

mod support;

#[test]
fn fixes_statements_items_and_cfg_disabled_code_and_is_idempotent() -> Result<()> {
    let source = "fn one() {}\nfn two() {}\n#[cfg(any())]\nfn inactive() {\n    let v = vec![\n        1,\n    ];\n    drop(v);\n}\npub fn run() {\n    let x = two(\n    );\n    drop(x);\n}\n";
    let directory = support::fixture(source)?;
    let output = support::run(&directory, false, "kiki_statement_padding")?;
    assert!(output.status.success(), "{output:?}");
    assert!(support::has_lint(&output, "kiki_statement_padding"), "{output:?}");
    let output = support::run(&directory, true, "kiki_statement_padding")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        support::source(&directory)?,
        "fn one() {}\n\nfn two() {}\n\n#[cfg(any())]\nfn inactive() {\n    let v = vec![\n        1,\n    ];\n\n    drop(v);\n}\n\npub fn run() {\n    let x = two(\n    );\n\n    drop(x);\n}\n"
    );

    let output = support::run(&directory, false, "kiki_statement_padding")?;
    assert!(output.status.success(), "{output:?}");
    assert!(!support::has_lint(&output, "kiki_statement_padding"), "{output:?}");
    Ok(())
}
