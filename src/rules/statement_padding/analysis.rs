use ra_ap_syntax::{
    AstNode,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    TextSize,
    ast,
};

use crate::diagnostic::{
    Diagnostic,
    Edit,
    Fix,
    Issue,
    RuleOutput,
};

// Only layout is changed: a blank line is inserted after the element that needs
// separation, so comments and attributes stay with the element that follows.
pub(super) fn analyze(root: &SyntaxNode, source: &str) -> RuleOutput {
    let mut output = RuleOutput::default();

    for scope in root.descendants().filter(|node| {
        matches!(
            node.kind(),
            SyntaxKind::SOURCE_FILE
                | SyntaxKind::ITEM_LIST
                | SyntaxKind::ASSOC_ITEM_LIST
                | SyntaxKind::EXTERN_ITEM_LIST
                | SyntaxKind::STMT_LIST
        )
    }) {
        let children: Vec<_> = scope.children().collect();
        for pair in children.windows(2) {
            check_pair(source, &pair[0], &pair[1], &mut output);
        }
    }

    output
}

fn check_pair(source: &str, current: &SyntaxNode, next: &SyntaxNode, output: &mut RuleOutput) {
    if !is_element(current) || !is_element(next) || is_declaration(current) || is_declaration(next) {
        return;
    }

    let current_range = current.text_range();
    let multiline = is_multiline(source, current);
    let single_line_constant = |node: &SyntaxNode| is_constant(node) && !is_multiline(source, node);
    if single_line_constant(current) && single_line_constant(next) {
        return;
    }

    if !multiline && !is_item(current) {
        return;
    }

    let message = if is_item(current) {
        "expected a blank line after this item"
    } else {
        "expected a blank line after this multi-line statement"
    };

    let between = &source[TextRange::new(current_range.end(), next.text_range().start())];
    let Some(newline) = between.find('\n') else {
        output.report(Diagnostic {
            range: current_range,
            message: format!("{message}; the next element starts on the same line"),
        });

        return;
    };

    let after = &between[newline + 1..];
    if after.find('\n').is_some_and(|end| after[..end].trim().is_empty()) {
        return;
    }

    let replacement = if between[..newline].ends_with('\r') {
        "\r\n"
    } else {
        "\n"
    };

    let offset = current_range.end() + TextSize::of(&between[..=newline]);
    output.issues.push(Issue {
        diagnostics: vec![Diagnostic {
            range: current_range,
            message: message.to_owned(),
        }],
        fix: Some(Fix {
            edits: vec![Edit {
                range: TextRange::empty(offset),
                replacement: replacement.to_owned(),
            }],
        }),
    });
}

// Attributes and comments belong to the element that follows them, so they do
// not make a single-line element multi-line.
fn is_multiline(source: &str, node: &SyntaxNode) -> bool {
    let start = node
        .children_with_tokens()
        .find(|element| {
            !matches!(
                element.kind(),
                SyntaxKind::ATTR | SyntaxKind::WHITESPACE | SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT
            )
        })
        .map_or(node.text_range().start(), |element| element.text_range().start());

    source[TextRange::new(start, node.text_range().end())].contains('\n')
}

fn is_element(node: &SyntaxNode) -> bool {
    !matches!(node.kind(), SyntaxKind::ATTR | SyntaxKind::ERROR)
}

// Use declarations, extern crates and module declarations are outside this rule.
fn is_declaration(node: &SyntaxNode) -> bool {
    match node.kind() {
        SyntaxKind::USE | SyntaxKind::EXTERN_CRATE => true,
        SyntaxKind::MODULE => ast::Module::cast(node.clone()).is_some_and(|module| module.item_list().is_none()),
        _ => false,
    }
}

fn is_item(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::CONST
            | SyntaxKind::ENUM
            | SyntaxKind::EXTERN_BLOCK
            | SyntaxKind::FN
            | SyntaxKind::IMPL
            | SyntaxKind::MACRO_CALL
            | SyntaxKind::MACRO_DEF
            | SyntaxKind::MACRO_RULES
            | SyntaxKind::MODULE
            | SyntaxKind::STATIC
            | SyntaxKind::STRUCT
            | SyntaxKind::TRAIT
            | SyntaxKind::TYPE_ALIAS
            | SyntaxKind::UNION
    )
}

fn is_constant(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::CONST | SyntaxKind::STATIC | SyntaxKind::TYPE_ALIAS
    )
}
