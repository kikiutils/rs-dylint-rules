use ra_ap_syntax::{
    AstNode,
    Edition,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    TextSize,
    ast,
};

use crate::syntax;

pub(super) fn read(root: &SyntaxNode, range: TextRange, edition: Edition) -> Option<(SyntaxNode, String)> {
    if let Some(path) = find(root, range) {
        return Some((path.syntax().clone(), spelling(&path, range)));
    }

    // Keep the original scope anchor. Recover spelling and inner attribute
    // barriers from expression-shaped input; compiler facts supply identity.
    let element = root.covering_element(range);
    let node = element.as_node().cloned().or_else(|| element.parent())?;
    let spelling = macro_spelling(root, range, edition)?;
    Some((node, spelling))
}

fn find(root: &SyntaxNode, range: TextRange) -> Option<ast::Path> {
    root.covering_element(range)
        .ancestors()
        .filter_map(ast::Path::cast)
        .find(|path| {
            path.syntax().text_range().start() == range.start()
                && path.syntax().text_range().contains_range(range)
                && !path.syntax().ancestors().any(|node| ast::Use::can_cast(node.kind()))
        })
}

fn macro_spelling(root: &SyntaxNode, range: TextRange, edition: Edition) -> Option<String> {
    let call = root
        .covering_element(range)
        .ancestors()
        .find_map(ast::MacroCall::cast)?;

    if syntax::protected(call.syntax()) {
        return None;
    }

    let input = call.token_tree()?;
    let range = range - input.syntax().text_range().start();
    let text = input.syntax().text().to_string();
    let parsed = ast::Expr::parse(&text, edition);
    let (root, range) = if parsed.errors().is_empty() {
        (parsed.syntax_node(), range)
    } else {
        let name = call.path()?.segment()?.name_ref()?;
        if name.text().trim_start_matches("r#") != "matches" {
            return None;
        }

        parse_match_input(&input, &text, range, edition)?
    };

    // A nested module does not inherit imports from the original call's scope.
    if root
        .covering_element(range)
        .ancestors()
        .any(|node| ast::Module::can_cast(node.kind()))
    {
        return None;
    }

    if let Some(path) = find(&root, range) {
        return (!syntax::protected(path.syntax())).then(|| spelling(&path, range));
    }

    macro_spelling(&root, range, edition)
}

fn parse_match_input(
    input: &ast::TokenTree,
    text: &str,
    range: TextRange,
    edition: Edition,
) -> Option<(SyntaxNode, TextRange)> {
    let input_range = input.syntax().text_range();
    let last = input
        .syntax()
        .children_with_tokens()
        .filter(|element| {
            element.text_range().end() < input_range.end()
                && !matches!(element.kind(), SyntaxKind::WHITESPACE | SyntaxKind::COMMENT)
        })
        .last()?;

    let end = if last.kind() == SyntaxKind::COMMA {
        usize::from(last.text_range().start() - input_range.start())
    } else {
        text.len() - 1
    };

    // Let Rust's parser find the expression/pattern boundary, including commas
    // inside generic arguments. Nested token trees are already grouped by CST.
    for comma in input
        .syntax()
        .children_with_tokens()
        .filter(|element| element.kind() == SyntaxKind::COMMA)
    {
        let separator = comma.text_range().start() - input_range.start();
        let split = usize::from(separator);
        if split >= end {
            continue;
        }

        let wrapped = format!("match ({}){{{}\n=> () }}", &text[1..split], &text[split + 1..end]);
        let parsed = ast::Expr::parse(&wrapped, edition);
        if parsed.errors().is_empty() {
            // The opening delimiter becomes `match (`; the separator becomes
            // `){`, adding one more byte only to paths after that separator.
            let mut offset = TextSize::of("match (") - TextSize::from(1);
            if range.start() > separator {
                offset += TextSize::from(1);
            }

            return Some((parsed.syntax_node(), range + offset));
        }
    }

    None
}

pub(super) fn spelling(path: &ast::Path, range: TextRange) -> String {
    let mut result = String::new();
    if path
        .first_segment()
        .is_some_and(|segment| segment.coloncolon_token().is_some())
    {
        result.push_str("::");
    }

    let names: Vec<_> = path
        .segments()
        .filter_map(|segment| {
            let name = segment.name_ref()?;
            range
                .contains_range(name.syntax().text_range())
                .then(|| name.text().trim_start_matches("r#").to_owned())
        })
        .collect();

    result.push_str(&names.join("::"));
    result
}
