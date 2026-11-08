use ra_ap_syntax::{
    AstNode,
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
    syntax::{
        derive_list,
        derive_meta,
    },
};

pub(super) fn groups_message() -> String {
    let mut groups = std::collections::BTreeMap::<_, Vec<_>>::new();
    for derive in super::DERIVES {
        let Some((group, _)) = derive.key else { continue };
        let names = groups.entry(group).or_default();
        if !names.contains(&derive.name) {
            names.push(derive.name);
        }
    }

    groups
        .into_values()
        .map(|names| names.join(", "))
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
            message: format!("expected derives grouped as {}", groups_message()),
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
            .is_some_and(|name| derives.iter().any(|derive| derive.helpers.contains(&name.as_str())))
    });

    registered_helper || syntax::inert_meta(meta)
}
