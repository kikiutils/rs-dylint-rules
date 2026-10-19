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

pub(super) const GROUPS: &[&[&str]] = &[
    &["Debug"],
    &["Clone", "Copy"],
    &["Default"],
    &["PartialEq", "Eq", "PartialOrd", "Ord", "Hash"],
    &["Serialize", "Serialize_repr", "Deserialize", "Deserialize_repr"],
    &["IsEmpty", "EnumIs"],
    &["EnumCount", "VariantNames", "VariantArray"],
];

// Group order first, then the fixed position within that group.
pub(super) type SortKey = (usize, usize);

pub(super) fn sort_key(name: &str) -> Option<SortKey> {
    GROUPS.iter().enumerate().find_map(|(group, names)| {
        names
            .iter()
            .position(|known| *known == name)
            .map(|position| (group, position))
    })
}

pub(super) fn analyze(root: &SyntaxNode, resolve: impl Fn(TextRange) -> Option<SortKey>) -> RuleOutput {
    let mut output = RuleOutput::default();
    let source = root.text().to_string();
    let base = root.text_range().start();
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

        if keys.windows(2).all(|pair| pair[0] <= pair[1]) {
            continue;
        }

        let range = attribute.syntax().text_range();
        let diagnostic = Diagnostic {
            range,
            message: format!(
                "expected derives grouped as {}",
                GROUPS
                    .iter()
                    .map(|group| group.join(", "))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
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

        let protected = attribute.syntax().ancestors().skip(1).any(|ancestor| {
            ancestor.children().filter_map(ast::Attr::cast).any(|other| {
                !safe_attribute(
                    &other,
                    (owner.as_ref() == Some(&ancestor)).then_some(owner_keys.as_slice()),
                )
            })
        });

        let comments = tree
            .syntax()
            .descendants_with_tokens()
            .any(|element| matches!(element.kind(), SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT));

        if protected || comments {
            output.report(diagnostic);
            continue;
        }

        let mut ordered: Vec<_> = paths.iter().zip(keys).collect();
        ordered.sort_by_key(|(_, key)| *key);
        let mut replacement = ordered
            .iter()
            .map(|(range, _)| source[std::ops::Range::<usize>::from(**range - base)].to_owned())
            .collect::<Vec<_>>()
            .join(", ");

        if source[std::ops::Range::<usize>::from(inner - base)]
            .trim_end()
            .ends_with(',')
        {
            replacement.push(',');
        }

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

    if syntax::inert_attribute(attribute) {
        return true;
    }

    let Some(meta) = attribute.meta() else { return false };
    let Some(name) = meta.simple_name() else { return false };
    match name.as_str() {
        "serde" | "is_empty" | "strum" => {
            let Some(keys) = owner_keys else { return false };
            let derives: &[&str] = match name.as_str() {
                "serde" => &["Serialize", "Deserialize", "Deserialize_repr"],
                "is_empty" => &["IsEmpty"],
                "strum" => &["EnumIs", "EnumCount", "VariantNames", "VariantArray"],
                _ => unreachable!(),
            };

            derives
                .iter()
                .filter_map(|name| sort_key(name))
                .any(|key| keys.contains(&key))
        },
        _ => false,
    }
}
