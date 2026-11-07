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
            Some(source.replace("fn f()", &format!("use std::io;{newline}fn f()")))
        );
    }
}

#[test]
fn outer_docs_stay_with_their_item() {
    let source = "/// function docs\nfn f() {}\n";
    assert_eq!(insert(source, &parse(source)), Some(format!("use std::io;\n{source}")));
}

#[test]
fn comments_in_the_insertion_preamble_block_edits() {
    for source in [
        "// file banner\n\nfn f() {}\n",
        "#![allow(/* owned comment */ unused)]\nfn f() {}\n",
    ] {
        assert!(insert(source, &parse(source)).is_none(), "{source}");
    }
}

#[test]
fn block_imports_preserve_indentation_and_line_endings() {
    for newline in ["\n", "\r\n"] {
        let source = format!("fn f() {{{newline}    work();{newline}}}{newline}");
        let root = parse(&source);
        let scope = root
            .descendants()
            .find_map(ast::StmtList::cast)
            .unwrap_or_else(|| panic!("missing scope"));

        assert_eq!(
            insert(&source, scope.syntax()),
            Some(source.replace("    work();", &format!("    use std::io;{newline}    work();")))
        );
    }
}

#[test]
fn same_line_blocks_get_a_line_break_before_the_import() {
    let source = "fn f() { work(); }";
    let root = parse(source);
    let scope = root
        .descendants()
        .find_map(ast::StmtList::cast)
        .unwrap_or_else(|| panic!("missing scope"));

    assert_eq!(
        insert(source, scope.syntax()),
        Some("fn f() { \nuse std::io;\nwork(); }".to_owned())
    );
}

#[test]
fn insertion_does_not_move_the_bom_or_shebang() {
    let source = "\u{feff}fn f() {}\r\n";
    let padded = source.replacen('\u{feff}', "   ", 1);
    assert_eq!(
        insert(source, &parse(&padded)),
        Some("\u{feff}use std::io;\r\nfn f() {}\r\n".to_owned())
    );

    let source = "#!/usr/bin/env rust-script\nfn f() {}\n";
    assert_eq!(
        insert(source, &parse(source)),
        Some("#!/usr/bin/env rust-script\nuse std::io;\nfn f() {}\n".to_owned())
    );
}
