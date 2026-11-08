use rustc_hir::def::Namespace;

use super::*;

#[test]
fn macro_expression_paths_keep_generic_suffixes_and_original_offsets() {
    let source = "fn run() { vec![::std::ptr::null::<u8>(); 2]; }\n";
    let facts = [(
        range(source, "::std::ptr::null"),
        Event::Use {
            kind: UseKind::Path,
            api: api(
                "null",
                Namespace::ValueNS,
                Form::Qualified {
                    qualifier: qualifier("std::ptr", "ptr", false),
                    member: "null".to_owned(),
                },
            ),
        },
    )];

    let output = analyze(source, &facts, true);
    assert_eq!(
        apply(source, &output),
        "use std::ptr;\nfn run() { vec![ptr::null::<u8>(); 2]; }\n"
    );
}

#[test]
fn nested_macro_inputs_are_read_from_rust_expressions() {
    for (source, expected) in [
        (
            "fn run() { format!(\"{}\", dbg!(tokio::time::sleep())); }\n",
            "use tokio::time::sleep;\nfn run() { format!(\"{}\", dbg!(sleep())); }\n",
        ),
        (
            "fn run() { vec![tokio::time::sleep(duration)]; }\n",
            "use tokio::time::sleep;\nfn run() { vec![sleep(duration)]; }\n",
        ),
        (
            "fn run() { custom!(value, tokio::time::sleep()); }\n",
            "use tokio::time::sleep;\nfn run() { custom!(value, sleep()); }\n",
        ),
    ] {
        let facts = [(
            range(source, "tokio::time::sleep"),
            Event::Use {
                kind: UseKind::Path,
                api: api("sleep", Namespace::ValueNS, Form::Direct),
            },
        )];

        let output = analyze(source, &facts, true);
        assert_eq!(apply(source, &output), expected);
    }
}

#[test]
fn macro_input_targets_use_the_call_site_cst() {
    let source =
        "fn run() { evaluate!({ #[cfg(feature = \"timers\")] let delay = tokio::time::sleep(duration); }); }\n";

    let facts = [(
        range(source, "tokio::time::sleep"),
        Event::Use {
            kind: UseKind::Path,
            api: api("sleep", Namespace::ValueNS, Form::Direct),
        },
    )];

    let output = analyze(source, &facts, true);
    assert_eq!(output.issues.len(), 1);
    assert_eq!(
        apply(source, &output),
        format!(
            "use tokio::time::sleep;\n{}",
            source.replace("tokio::time::sleep(duration)", "sleep(duration)")
        )
    );
}

#[test]
fn non_expression_macro_grammars_are_not_guessed() {
    for source in [
        "fn run() { custom!(target: \"name\", tokio::time::sleep()); }\n",
        "fn run() { custom!(value, code if code == tokio::time::sleep()); }\n",
        "fn run() { custom!(value, code if code == tokio::time::sleep(),); }\n",
    ] {
        let facts = [(
            range(source, "tokio::time::sleep"),
            Event::Use {
                kind: UseKind::Path,
                api: api("sleep", Namespace::ValueNS, Form::Direct),
            },
        )];

        assert_eq!(analyze(source, &facts, true).issues, [], "{source}");
    }
}

#[test]
fn match_macro_inputs_preserve_offsets_through_patterns_guards_and_nesting() {
    for input in [
        "matches!(value, code if code == Errno::INTR)",
        "std::matches!(value, code if code == Errno::INTR)",
        "::core::matches!(value, code if code == Errno::INTR)",
        "matches!(value, code if code == Errno::INTR,)",
        "matches!(value, code if /* 註解 */ code == Errno::INTR)",
        "matches![value, code if code == Errno::INTR, /* trailing */]",
        "matches! { value, code if code == Errno::INTR }",
        "matches!(value, Some(error) if matches!(error, code if code == Errno::INTR))",
        "matches!(value::<u8, u16>(), code if code == Errno::INTR)",
        "matches!(Value { code: 1 }, code if code == Errno::INTR)",
        "matches!(value, Errno::INTR | Errno::AGAIN)",
        "matches!(Errno::INTR, code if code != Errno::AGAIN)",
    ] {
        let source = format!("fn run() {{ {input}; }}\n");
        let facts: Vec<_> = ranges(&source, "Errno")
            .map(|range| {
                (
                    range,
                    Event::Use {
                        kind: UseKind::Path,
                        api: api(
                            "Errno",
                            Namespace::TypeNS,
                            Form::Qualified {
                                qualifier: qualifier("rustix", "rustix", true),
                                member: "io::Errno".to_owned(),
                            },
                        ),
                    },
                )
            })
            .collect();

        assert_eq!(
            apply(&source, &analyze(&source, &facts, true)),
            source.replace("Errno", "rustix::io::Errno"),
            "{input}",
        );
    }
}
