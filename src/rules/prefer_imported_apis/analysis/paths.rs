use ra_ap_syntax::{
    AstNode,
    Edition,
    SyntaxNode,
    TextRange,
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
    let parsed = ast::Expr::parse(&input.syntax().text().to_string(), edition);
    if !parsed.errors().is_empty() {
        return None;
    }

    let root = parsed.syntax_node();
    let range = range - input.syntax().text_range().start();
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
