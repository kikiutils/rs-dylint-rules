use ra_ap_syntax::{
    AstNode,
    SyntaxKind,
    SyntaxNode,
    SyntaxToken,
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

// Only layout is changed: trailing comments stay with the current element,
// while leading comments and attributes stay with the next one.
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

    // Do not highlight ordinary leading comments, which may actually trail the
    // previous element. Attributes and doc comments remain part of the item.
    let start = current
        .children_with_tokens()
        .find(|element| !matches!(element.kind(), SyntaxKind::WHITESPACE | SyntaxKind::COMMENT))
        .map_or(current.text_range().start(), |element| element.text_range().start());

    let current_range = TextRange::new(start, current.text_range().end());
    let multiline = is_multiline(source, current);
    if is_constant(current) && !multiline && is_constant(next) {
        return;
    }

    // Item also includes builtin assembly expressions; they remain statements here.
    let item = ast::Item::can_cast(current.kind()) && current.kind() != SyntaxKind::ASM_EXPR;
    if !multiline && !item {
        return;
    }

    let message = if item {
        "expected a blank line after this item"
    } else {
        "expected a blank line after this multi-line statement"
    };

    let Some((whitespace, newline)) = padding_whitespace(current) else {
        output.report(Diagnostic {
            range: current_range,
            message: format!("{message}; the next element starts on the same line"),
        });

        return;
    };

    let text = whitespace.text();
    if text[newline + 1..].contains('\n') {
        return;
    }

    let offset = whitespace.text_range().start() + TextSize::of(&text[..=newline]);
    // A line-comment token can own the CR of a CRLF ending.
    let replacement = if source[..usize::from(offset)].ends_with("\r\n") {
        "\r\n"
    } else {
        "\n"
    };

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

// Walk tokens rather than node ranges: the parser may attach a trailing comment
// to the next item. Only whitespace outside comments can separate elements.
fn padding_whitespace(node: &SyntaxNode) -> Option<(SyntaxToken, usize)> {
    let mut token = node.last_token()?.next_token()?;
    loop {
        match token.kind() {
            SyntaxKind::WHITESPACE => {
                if let Some(newline) = token.text().find('\n') {
                    return Some((token, newline));
                }
            },
            SyntaxKind::COMMENT => {},
            _ => return None,
        }

        token = token.next_token()?;
    }
}

// Attributes and comments belong to the element that follows them, so they do
// not make a single-line element multi-line.
fn is_multiline(source: &str, node: &SyntaxNode) -> bool {
    // Expression-statement attributes are stored on the expression itself.
    let expression = ast::ExprStmt::cast(node.clone()).and_then(|statement| statement.expr());
    let content = expression.as_ref().map_or(node, AstNode::syntax);
    let start = content
        .children_with_tokens()
        .find(|element| {
            !matches!(
                element.kind(),
                SyntaxKind::ATTR | SyntaxKind::WHITESPACE | SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT
            )
        })
        .map_or(content.text_range().start(), |element| element.text_range().start());

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

fn is_constant(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::CONST | SyntaxKind::STATIC | SyntaxKind::TYPE_ALIAS
    )
}
