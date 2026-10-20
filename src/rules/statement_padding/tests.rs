use super::analysis;
use crate::{
    diagnostic::RuleOutput,
    test_support::{
        apply,
        parse,
    },
};

fn check(source: &str) -> RuleOutput {
    let root = parse(source);
    analysis::analyze(&root, source)
}

#[test]
fn pads_multiline_statements_and_items_in_nested_scopes_and_is_idempotent() {
    for (source, expected) in [
        (
            "unsafe extern \"C\" {\n    fn a();\n    fn b();\n}\n",
            "unsafe extern \"C\" {\n    fn a();\n\n    fn b();\n}\n",
        ),
        (
            "trait T {\n    fn a(&self) {\n        work();\n    }\n    fn b(&self) {}\n}\n",
            "trait T {\n    fn a(&self) {\n        work();\n    }\n\n    fn b(&self) {}\n}\n",
        ),
        (
            "fn f() {\n    let a = unsafe {\n        1\n    };\n    let b = async {\n        2\n    };\n    let c = const {\n        3\n    };\n    let _ = (a, b, c);\n}\n",
            "fn f() {\n    let a = unsafe {\n        1\n    };\n\n    let b = async {\n        2\n    };\n\n    let c = const {\n        3\n    };\n\n    let _ = (a, b, c);\n}\n",
        ),
        (
            "fn f(x: u8) -> u8 {\n    match x {\n        0 => {\n            let y = g(\n                1,\n            );\n            y\n        }\n        _ => 2,\n    }\n}\n",
            "fn f(x: u8) -> u8 {\n    match x {\n        0 => {\n            let y = g(\n                1,\n            );\n\n            y\n        }\n        _ => 2,\n    }\n}\n",
        ),
        (
            "#[cfg(any())]\nfn inactive() {\n    let v = vec![\n        1,\n    ];\n    drop(v);\n}\n",
            "#[cfg(any())]\nfn inactive() {\n    let v = vec![\n        1,\n    ];\n\n    drop(v);\n}\n",
        ),
    ] {
        let output = check(source);
        assert!(!output.issues.is_empty(), "{source}");
        let fixed = apply(source, &output);
        assert_eq!(fixed, expected);
        assert!(check(&fixed).issues.is_empty(), "{:?}", check(&fixed));
    }
}

#[test]
fn keeps_comments_and_attributes_with_the_following_element() {
    let source = "fn a() {\n    work(\n        1,\n    );\n    // about b\n    #[allow(unused)]\n    let b = 2;\n}\n";
    let output = check(source);
    assert_eq!(output.issues.len(), 1);
    assert_eq!(
        apply(source, &output),
        "fn a() {\n    work(\n        1,\n    );\n\n    // about b\n    #[allow(unused)]\n    let b = 2;\n}\n"
    );
}

#[test]
fn reports_same_line_neighbors_without_a_fix() {
    let source = "fn one() {} fn two() {}\n";
    let output = check(source);
    assert_eq!(output.issues.len(), 1);
    assert!(output.issues[0].fix.is_none());
    assert_eq!(apply(source, &output), source);
}

#[test]
fn inserts_crlf_after_crlf_lines() {
    let source = "fn one() {}\r\nfn two() {}\r\n";
    let output = check(source);
    assert_eq!(apply(source, &output), "fn one() {}\r\n\r\nfn two() {}\r\n");
}

#[test]
fn leaves_declarations_and_adjacent_single_line_constants_alone() {
    let source = "use a::B;\nfn one() {}\nuse c::D;\nconst X: u8 = 1;\nconst Y: u8 = 2;\nmod m;\n";
    assert_eq!(check(source), RuleOutput::default());
}

#[test]
fn ignores_macro_rules_bodies() {
    let source = "macro_rules! m {\n    () => {\n        fn a() {}\n        fn b() {}\n    };\n}\n";
    assert_eq!(check(source), RuleOutput::default());
}

#[test]
fn attributes_and_doc_comments_do_not_make_a_single_line_element_multiline() {
    for source in [
        "#[allow(dead_code)]\nconst A: u8 = 1;\nconst B: u8 = 2;\n",
        "/// doc for C\nconst C: u8 = 3;\nconst D: u8 = 4;\n",
        "pub fn f() -> u8 {\n    #[allow(unused)]\n    let x = 1;\n    let y = 2;\n    x + y\n}\n",
    ] {
        assert_eq!(check(source), RuleOutput::default(), "{source}");
    }

    let source = "#[allow(dead_code)]\nconst A: &[u8] = &[\n    1,\n];\nconst B: u8 = 2;\n";
    let output = check(source);
    assert_eq!(output.issues.len(), 1);
    assert_eq!(
        apply(source, &output),
        "#[allow(dead_code)]\nconst A: &[u8] = &[\n    1,\n];\n\nconst B: u8 = 2;\n"
    );
}

#[test]
fn single_line_constants_do_not_require_padding_before_multiline_constants() {
    for source in [
        r#"pub static CONFIG_DIR: LazyLock<Path> = LazyLock::new(|| path!("/etc/{COMPANY_DIR_NAME}/{APP_DIR_NAME}"));
pub static DATA_DIR: LazyLock<Path> = LazyLock::new(|| path!("/var/lib/{COMPANY_DIR_NAME}/{APP_DIR_NAME}"));
pub static ENROLLMENT_CONTEXT_FILE: LazyLock<Path> =
    LazyLock::new(|| path!("/run/{COMPANY_DIR_NAME}/{APP_DIR_NAME}/{ENROLLMENT_CONTEXT_FILE_NAME}"));
"#,
        r#"pub(crate) const MINIMAL_EXECUTION: [(&str, &str); 2] = [("LC_ALL", "C"), ("PATH", MINIMAL_EXECUTION_PATH)];
pub(crate) const MINIMAL_EXECUTION_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
pub(crate) const PACKAGE_MANAGER_EXECUTION: [(&str, &str); 3] = [
    MINIMAL_EXECUTION[0],
    MINIMAL_EXECUTION[1],
    ("DEBIAN_FRONTEND", "noninteractive"),
];
"#,
        "type A = u8;\ntype B = (\n    u8,\n    u16,\n);\n",
    ] {
        assert_eq!(check(source), RuleOutput::default(), "{source}");
    }
}

#[test]
fn expression_attributes_do_not_make_single_line_statements_multiline() {
    for source in [
        r#"fn run() {
    #[cfg(target_os = "linux")]
    process.process_group(0);
    let mut child = process
        .spawn()
        .map_err(|error| format!("failed to execute {}: {error}", command.display()))?;
}
"#,
        "fn run() {\n    #[allow(\n        unused\n    )]\n    process.process_group(0);\n    finish();\n}\n",
        "fn run() {\n    /// About the call.\n    work();\n    finish();\n}\n",
    ] {
        assert_eq!(check(source), RuleOutput::default(), "{source}");
    }

    let source = "fn run() {\n    #[allow(unused)]\n    work(\n        1,\n    );\n    finish();\n}\n";
    let output = check(source);
    assert_eq!(output.issues.len(), 1);
    let fixed = apply(source, &output);
    assert_eq!(
        fixed,
        "fn run() {\n    #[allow(unused)]\n    work(\n        1,\n    );\n\n    finish();\n}\n"
    );

    assert_eq!(check(&fixed), RuleOutput::default());
}
