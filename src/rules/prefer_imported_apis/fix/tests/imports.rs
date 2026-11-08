use ra_ap_syntax::{
    AstNode,
    SyntaxNode,
    ast,
};

use super::insertion;
use crate::{
    diagnostic::Edit,
    test_support::parse,
};

fn apply(source: &str, edit: &Edit) -> String {
    let mut result = source.to_owned();
    result.replace_range(std::ops::Range::<usize>::from(edit.range), &edit.replacement);
    result
}

fn insert(source: &str, scope: &SyntaxNode) -> Option<String> {
    insertion(scope, source).map(|insertion| apply(source, &insertion.edit(["use std::io;"])))
}

#[test]
fn file_imports_follow_inner_attributes_and_docs() {
    for newline in ["\n", "\r\n"] {
        let source = format!("#![allow(unused)]{newline}//! scope docs{newline}fn f() {{}}{newline}");
        assert_eq!(
            insert(&source, &parse(&source)),
            Some(source.replace("fn f()", "use std::io;\nfn f()"))
        );
    }
}

#[test]
fn outer_docs_stay_with_their_item() {
    for docs in ["/// function docs\n", "/// function docs\n\n", "/** function docs */ "] {
        let source = format!("#![allow(unused)]\n{docs}fn f() {{}}\n");
        assert_eq!(
            insert(&source, &parse(&source)),
            Some(format!("#![allow(unused)]\nuse std::io;\n{docs}fn f() {{}}\n"))
        );
    }

    let source = "#![allow(unused)] /** function docs */ fn f() {}\n";
    assert_eq!(
        insert(source, &parse(source)),
        Some("#![allow(unused)] use std::io;\n/** function docs */ fn f() {}\n".to_owned())
    );
}

#[test]
fn preamble_comments_and_inner_attribute_comments_allow_insertion() {
    for newline in ["\n", "\r\n"] {
        for preamble in [
            format!("// file banner{newline}{newline}"),
            format!("#![allow(/* owned comment */ unused)]{newline}"),
            format!("// shared helpers{newline}#![allow(dead_code)]{newline}{newline}"),
            format!("#![allow(unused)] // attribute comment{newline}"),
        ] {
            let source = format!("{preamble}fn f() {{}}{newline}");
            assert_eq!(
                insert(&source, &parse(&source)),
                Some(format!("{preamble}use std::io;\nfn f() {{}}{newline}"))
            );
        }
    }
}

#[test]
fn leading_comments_stay_with_the_following_item_or_statement() {
    for newline in ["\n", "\r\n"] {
        for comment in ["// item comment", "/* item comment */", "/// item docs"] {
            let source = format!("{comment}{newline}fn f() {{}}{newline}");
            assert_eq!(
                insert(&source, &parse(&source)),
                Some(format!("use std::io;\n{source}"))
            );
        }

        let source = format!("fn f() {{{newline}    // statement comment{newline}    work();{newline}}}{newline}");
        let root = parse(&source);
        let scope = root
            .descendants()
            .find_map(ast::StmtList::cast)
            .unwrap_or_else(|| panic!("missing scope"));

        assert_eq!(
            insert(&source, scope.syntax()),
            Some(source.replace("    // statement", "    use std::io;\n// statement"))
        );
    }
}

#[test]
fn inserted_text_is_uses_and_lf_only() {
    let source = "fn f() { work(); }";
    let root = parse(source);
    let scope = root
        .descendants()
        .find_map(ast::StmtList::cast)
        .unwrap_or_else(|| panic!("missing scope"));

    let edit = insertion(scope.syntax(), source)
        .unwrap_or_else(|| panic!("missing insertion"))
        .edit(["use std::io;", "use std::fmt;"]);

    assert_eq!(edit.replacement, "use std::io;\nuse std::fmt;\n");
}

#[test]
fn insertion_does_not_move_the_bom_or_shebang() {
    let source = "\u{feff}fn f() {}\r\n";
    let padded = source.replacen('\u{feff}', "   ", 1);
    assert_eq!(
        insert(source, &parse(&padded)),
        Some("\u{feff}use std::io;\nfn f() {}\r\n".to_owned())
    );

    let source = "#!/usr/bin/env rust-script\nfn f() {}\n";
    assert_eq!(
        insert(source, &parse(source)),
        Some("#!/usr/bin/env rust-script\nuse std::io;\nfn f() {}\n".to_owned())
    );
}
