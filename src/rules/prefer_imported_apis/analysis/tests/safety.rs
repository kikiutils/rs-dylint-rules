use rustc_hir::def::Namespace;

use super::*;

#[test]
fn binding_conflicts_keep_diagnostics_without_suggestions() {
    let source = "fn run() { tokio::time::sleep(); }\n";
    let facts = [(
        range(source, "tokio::time::sleep"),
        Event::Use {
            kind: UseKind::Path,
            api: api("sleep", Namespace::ValueNS, Form::Direct),
        },
    )];

    let output = analyze(source, &facts, false);
    assert_eq!(output.issues.len(), 1);
    assert_eq!(apply(source, &output), source);
}

#[test]
fn file_banners_allow_both_external_reuse_and_new_namespace_imports() {
    let source =
        "// file banner\n\nfn json() -> ::serde_json::Value { todo!() }\nfn io() -> std::io::Result<()> { Ok(()) }\n";

    let facts = [
        (
            range(source, "::serde_json::Value"),
            Event::Use {
                kind: UseKind::Path,
                api: api(
                    "Value",
                    Namespace::TypeNS,
                    Form::Qualified {
                        qualifier: qualifier("serde_json", "serde_json", true),
                        member: "Value".to_owned(),
                    },
                ),
            },
        ),
        (
            range(source, "std::io::Result"),
            Event::Use {
                kind: UseKind::Path,
                api: api(
                    "Result",
                    Namespace::TypeNS,
                    Form::Qualified {
                        qualifier: qualifier("std::io", "io", false),
                        member: "Result".to_owned(),
                    },
                ),
            },
        ),
    ];

    let output = analyze(source, &facts, true);
    assert_eq!(
        apply(source, &output),
        source
            .replace("fn json", "use std::io;\nfn json")
            .replace("::serde_json::Value", "serde_json::Value")
            .replace("std::io::Result", "io::Result")
    );
}

#[test]
fn leaf_alias_imports_and_uses_are_rewritten_together() {
    for (suffix, removable) in [
        ("", true),
        (r#"const NAME: &str = "AnyhowError";"#, true),
        ("#[cfg(any())] fn inactive(_: self::AnyhowError) {}", false),
        ("#[cfg(any())] fn inactive(_: crate::AnyhowError) {}", false),
        (
            r#"#[cfg_attr(any(), serde(with = "AnyhowError"))] struct Other;"#,
            false,
        ),
    ] {
        let source =
            format!("use anyhow::Error as AnyhowError;\nfn run() {{ let _: Option<AnyhowError> = None; }}\n{suffix}");

        let api = api(
            "Error",
            Namespace::TypeNS,
            Form::Qualified {
                qualifier: qualifier("anyhow", "anyhow", true),
                member: "Error".to_owned(),
            },
        );

        let import_range = range(&source, "AnyhowError;");
        let access_range = range(&source, "AnyhowError>");
        let facts = [
            (
                TextRange::new(import_range.start(), import_range.end() - TextSize::from(1)),
                Event::Import {
                    api: api.clone(),
                    name: "AnyhowError".to_owned(),
                    renamed: true,
                },
            ),
            (
                TextRange::new(access_range.start(), access_range.end() - TextSize::from(1)),
                Event::Use {
                    kind: UseKind::Path,
                    api,
                },
            ),
        ];

        let output = analyze(&source, &facts, true);
        let expected = source.replace("Option<AnyhowError>", "Option<anyhow::Error>");
        let expected = if removable {
            expected.replace("use anyhow::Error as AnyhowError;", "")
        } else {
            expected
        };

        assert_eq!(apply(&source, &output), expected, "{suffix}");
    }
}

#[test]
fn comments_inside_the_replaced_path_are_not_erased() {
    let source = "fn run() { std::process /* reason */ ::id(); }\n";
    let api = api(
        "id",
        Namespace::ValueNS,
        Form::Qualified {
            qualifier: qualifier("std::process", "process", false),
            member: "id".to_owned(),
        },
    );

    let facts = [(
        range(source, "std::process /* reason */ ::id"),
        Event::Use {
            kind: UseKind::Path,
            api,
        },
    )];

    let output = analyze(source, &facts, true);
    assert_eq!(output.issues.len(), 1);
    assert_eq!(apply(source, &output), source);
}

#[test]
fn new_block_imports_do_not_cross_statement_cfg_attributes() {
    let source = "fn run() { #[cfg(feature = \"timers\")] let delay = tokio::time::sleep(duration); }\n";
    let facts = [(
        range(source, "tokio::time::sleep"),
        Event::Use {
            kind: UseKind::Path,
            api: api("sleep", Namespace::ValueNS, Form::Direct),
        },
    )];

    let output = analyze(source, &facts, true);
    assert_eq!(output.issues.len(), 1);
    assert_eq!(apply(source, &output), source);
}
