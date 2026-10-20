use ra_ap_syntax::{
    AstNode,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    ast,
};

use crate::{
    diagnostic::{
        Diagnostic,
        Edit,
        Fix,
        Issue,
        RuleOutput,
    },
    syntax,
};

pub(super) type SortKey = (usize, usize);

#[cfg(test)]
pub(super) fn sort_key(name: &str) -> Option<SortKey> {
    super::DERIVES
        .iter()
        .find(|derive| derive.name == name)
        .map(|derive| derive.key)
}

fn groups_message() -> String {
    let mut groups: Vec<Vec<&str>> = Vec::new();
    for derive in super::DERIVES {
        if groups.len() <= derive.key.0 {
            groups.push(Vec::new());
        }

        groups[derive.key.0].push(derive.name);
    }

    groups
        .iter()
        .map(|group| group.join(", "))
        .collect::<Vec<_>>()
        .join("; ")
}

pub(super) fn analyze(root: &SyntaxNode, source: &str, resolve: impl Fn(TextRange) -> Option<SortKey>) -> RuleOutput {
    let mut output = RuleOutput::default();
    for attribute in root.descendants().filter_map(ast::Attr::cast) {
        let Some((tree, inner, paths)) = derive_list(&attribute) else {
            continue;
        };

        if paths.len() < 2 {
            continue;
        }

        let Some(keys) = paths.iter().map(|range| resolve(*range)).collect::<Option<Vec<_>>>() else {
            continue;
        };

        if keys.is_sorted() {
            continue;
        }

        let range = attribute.syntax().text_range();
        let diagnostic = Diagnostic {
            range,
            message: format!("expected derives grouped as {}", groups_message()),
        };

        let owner = attribute.syntax().parent();
        let owner_keys: Vec<_> = owner
            .iter()
            .flat_map(SyntaxNode::children)
            .filter_map(ast::Attr::cast)
            .filter_map(|attr| derive_list(&attr))
            .flat_map(|(_, _, paths)| paths)
            .filter_map(&resolve)
            .collect();

        let protected = syntax::protected_with(attribute.syntax(), |other| {
            safe_attribute(
                other,
                (other.syntax().parent() == owner).then_some(owner_keys.as_slice()),
            )
        });

        if protected || syntax::has_comments(tree.syntax(), tree.syntax().text_range()) {
            output.report(diagnostic);
            continue;
        }

        let mut ordered: Vec<_> = paths.iter().zip(keys).collect();
        ordered.sort_by_key(|(_, key)| *key);
        let replacement = ordered
            .iter()
            .map(|(range, _)| source[**range].to_owned())
            .collect::<Vec<_>>()
            .join(", ");

        output.issues.push(Issue {
            diagnostics: vec![diagnostic],
            fix: Some(Fix {
                edits: vec![Edit {
                    range: inner,
                    replacement,
                }],
            }),
        });
    }

    output
}

fn derive_meta(attribute: &ast::Attr) -> Option<ast::TokenTreeMeta> {
    let ast::Meta::TokenTreeMeta(meta) = attribute.meta()? else {
        return None;
    };

    let path = meta
        .path()?
        .syntax()
        .text()
        .to_string()
        .replace(char::is_whitespace, "");

    matches!(
        path.as_str(),
        "derive" | "core::derive" | "std::derive" | "::core::derive" | "::std::derive"
    )
    .then_some(meta)
}

fn derive_list(attribute: &ast::Attr) -> Option<(ast::TokenTree, TextRange, Vec<TextRange>)> {
    let meta = derive_meta(attribute)?;
    let tree = meta.token_tree()?;
    let left = tree.l_paren_token()?;
    let right = tree.r_paren_token()?;
    let inner = TextRange::new(left.text_range().end(), right.text_range().start());
    let mut paths = Vec::new();
    let mut start = None;
    let mut end = None;
    let mut supported = true;
    for element in tree
        .syntax()
        .children_with_tokens()
        .filter(|element| inner.contains_range(element.text_range()))
    {
        if element.as_node().is_some() {
            supported = false;
            break;
        }

        match element.kind() {
            SyntaxKind::WHITESPACE | SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT => {},
            SyntaxKind::COMMA => {
                if let (Some(start), Some(end)) = (start.take(), end.take()) {
                    paths.push(TextRange::new(start, end));
                } else {
                    supported = false;
                    break;
                }
            },
            _ => {
                start.get_or_insert(element.text_range().start());
                end = Some(element.text_range().end());
            },
        }
    }

    if let (Some(start), Some(end)) = (start, end) {
        paths.push(TextRange::new(start, end));
    }

    supported.then_some((tree, inner, paths))
}

fn safe_attribute(attribute: &ast::Attr, owner_keys: Option<&[SortKey]>) -> bool {
    if derive_meta(attribute).is_some() {
        return true;
    }

    let Some(keys) = owner_keys else { return false };
    let Some(name) = attribute.meta().and_then(|meta| meta.simple_name()) else {
        return false;
    };

    super::DERIVES
        .iter()
        .any(|derive| derive.helper == Some(name.as_str()) && keys.contains(&derive.key))
}
