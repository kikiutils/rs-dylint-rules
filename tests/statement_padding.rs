use std::process::Output;

use anyhow::Result;
use pathkit::{
    SyncFsOps,
    path,
};
use tempfile::TempDir;

mod support;

use self::support::fixture;

fn run(directory: &TempDir, fix: bool) -> Result<Output> {
    support::run(directory, fix, "kiki_statement_padding")
}

fn has_lint(output: &Output) -> bool {
    !support::diagnostics(output, "kiki_statement_padding").is_empty()
}

#[test]
fn fixes_statements_items_and_cfg_disabled_code_and_is_idempotent() -> Result<()> {
    let source = "fn one() {}\nfn two() {}\n#[cfg(any())]\nfn inactive() {\n    let v = vec![\n        1,\n    ];\n    drop(v);\n}\npub fn run() {\n    let x = two(\n    );\n    drop(x);\n}\n";
    let directory = fixture(source)?;
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(has_lint(&output), "{output:?}");
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        "fn one() {}\n\nfn two() {}\n\n#[cfg(any())]\nfn inactive() {\n    let v = vec![\n        1,\n    ];\n\n    drop(v);\n}\n\npub fn run() {\n    let x = two(\n    );\n\n    drop(x);\n}\n"
    );

    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(!has_lint(&output), "{output:?}");
    Ok(())
}

#[test]
fn honors_allow_attributes_on_the_enclosing_item() -> Result<()> {
    let source = "#[allow(kiki_statement_padding)]\nfn allowed() {\n    let s = String::from(\n        \"a\",\n    );\n    drop(s);\n}\n\nfn other() {\n    let s = String::from(\n        \"b\",\n    );\n    drop(s);\n}\n";
    let directory = fixture(source)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        "#[allow(kiki_statement_padding)]\nfn allowed() {\n    let s = String::from(\n        \"a\",\n    );\n    drop(s);\n}\n\nfn other() {\n    let s = String::from(\n        \"b\",\n    );\n\n    drop(s);\n}\n"
    );

    Ok(())
}
