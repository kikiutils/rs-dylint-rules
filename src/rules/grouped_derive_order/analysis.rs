use ra_ap_syntax::{
    AstNode,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    ast,
};

use super::Derive;
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

pub(super) fn groups_message(derives: &[Derive]) -> String {
    derives
        .chunk_by(|left, right| left.key.0 == right.key.0)
        .map(|group| {
            let mut names = Vec::new();
            for derive in group {
                if !names.contains(&derive.name) {
                    names.push(derive.name);
                }
            }

            names.join(", ")
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub(super) fn analyze(
    root: &SyntaxNode,
    source: &str,
    resolve: impl Fn(TextRange) -> Option<&'static Derive>,
) -> RuleOutput {
    let mut output = RuleOutput::default();
    for attribute in root.descendants().filter_map(ast::Attr::cast) {
        let Some((tree, inner, paths)) = derive_list(&attribute) else {
            continue;
        };

        if paths.len() < 2 {
            continue;
        }

        let Some(derives) = paths.iter().map(|range| resolve(*range)).collect::<Option<Vec<_>>>() else {
            continue;
        };

        if derives.is_sorted_by_key(|derive| derive.key) {
            continue;
        }

        let range = attribute.syntax().text_range();
        let diagnostic = Diagnostic {
            range,
            message: format!("expected derives grouped as {}", groups_message(super::DERIVES)),
        };

        let owner = attribute.syntax().parent();
        let owner_derives: Vec<_> = owner
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
                (other.syntax().parent() == owner).then_some(owner_derives.as_slice()),
                &resolve,
            )
        });

        if protected || syntax::has_comments(tree.syntax(), tree.syntax().text_range()) {
            output.report(diagnostic);
            continue;
        }

        let mut ordered: Vec<_> = paths.iter().zip(derives).collect();
        ordered.sort_by_key(|(_, derive)| derive.key);
        let replacement = ordered
            .iter()
            .map(|(range, _)| &source[**range])
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
    for element in tree
        .syntax()
        .children_with_tokens()
        .filter(|element| inner.contains_range(element.text_range()))
    {
        if element.as_node().is_some() {
            return None;
        }

        match element.kind() {
            SyntaxKind::WHITESPACE | SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT => {},
            SyntaxKind::COMMA => {
                if let (Some(start), Some(end)) = (start.take(), end.take()) {
                    paths.push(TextRange::new(start, end));
                } else {
                    return None;
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

    Some((tree, inner, paths))
}

fn safe_attribute(
    attribute: &ast::Attr,
    owner_derives: Option<&[&Derive]>,
    resolve: &impl Fn(TextRange) -> Option<&'static Derive>,
) -> bool {
    if derive_meta(attribute).is_some() {
        return derive_list(attribute)
            .is_some_and(|(_, _, paths)| paths.into_iter().all(|range| resolve(range).is_some()));
    }

    attribute
        .meta()
        .is_some_and(|meta| safe_helper_meta(meta, owner_derives))
}

fn safe_helper_meta(meta: ast::Meta, owner_derives: Option<&[&Derive]>) -> bool {
    if let ast::Meta::CfgAttrMeta(cfg) = meta {
        return cfg.metas().all(|meta| safe_helper_meta(meta, owner_derives));
    }

    let registered_helper = owner_derives.is_some_and(|derives| {
        meta.simple_name()
            .is_some_and(|name| derives.iter().any(|derive| derive.helper == Some(name.as_str())))
    });

    registered_helper || syntax::inert_meta(meta)
}
