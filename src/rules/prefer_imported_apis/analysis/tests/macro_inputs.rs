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
    let source = "fn run() { custom!(target: \"name\", tokio::time::sleep()); }\n";
    let facts = [(
        range(source, "tokio::time::sleep"),
        Event::Use {
            kind: UseKind::Path,
            api: api("sleep", Namespace::ValueNS, Form::Direct),
        },
    )];

    assert_eq!(analyze(source, &facts, true).issues, []);
}
