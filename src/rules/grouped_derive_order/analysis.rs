use ra_ap_syntax::{
    AstNode,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    ast,
};

use crate::diagnostic::{
    Diagnostic,
    Edit,
    Fix,
    Issue,
    RuleOutput,
};

pub(super) const GROUPS: &[&[&str]] = &[
    &["Debug"],
    &["Clone", "Copy"],
    &["Default"],
    &["PartialEq", "Eq", "PartialOrd", "Ord", "Hash"],
    &["Serialize", "Deserialize"],
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
        let Some(meta) = derive_meta(&attribute) else { continue };
        let Some(tree) = meta.token_tree() else { continue };
        let Some(left) = tree.l_paren_token() else { continue };
        let Some(right) = tree.r_paren_token() else { continue };
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
        if !supported || paths.len() < 2 {
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
        let protected = attribute.syntax().ancestors().skip(1).any(|ancestor| {
            ancestor
                .children()
                .filter_map(ast::Attr::cast)
                .any(|other| derive_meta(&other).is_none())
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
