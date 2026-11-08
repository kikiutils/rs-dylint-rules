use std::collections::BTreeMap;

use ra_ap_syntax::{
    AstNode,
    Direction,
    NodeOrToken,
    SyntaxKind,
    SyntaxNode,
    SyntaxToken,
    TextRange,
    ast,
    ast::HasVisibility,
};

use super::super::{
    analysis::{
        Access,
        Import,
    },
    model::UseKind,
    names::{
        NameRequest,
        RemovalRequest,
    },
    policy::Form,
};
use crate::{
    diagnostic::Edit,
    syntax,
};

type Declarations = BTreeMap<(u32, u32), (ast::Use, Vec<TextRange>, Vec<Edit>)>;

pub(super) struct Rewrites {
    pub(super) edits: Vec<Edit>,
    pub(super) removed: Vec<TextRange>,
}

pub(super) fn route(import: &Import) -> Option<(String, Vec<TextRange>)> {
    let tree = ast::UseTree::cast(import.node.clone())?;
    let mut paths: Vec<_> = tree
        .syntax()
        .ancestors()
        .filter_map(ast::UseTree::cast)
        .filter_map(|tree| tree.path())
        .collect();

    paths.reverse();
    let ranges = paths.iter().map(|path| path.syntax().text_range()).collect();
    let path = paths
        .iter()
        .map(|path| path.syntax().text().to_string())
        .collect::<Vec<_>>()
        .join("::");

    Some((path, ranges))
}

pub(super) fn plan<'a>(
    root: &SyntaxNode,
    source: &str,
    imports: impl IntoIterator<Item = &'a Import>,
    accesses: &[Access],
    ranges: &[TextRange],
    retained: &[TextRange],
    checks: &super::plan::Checks<'_>,
) -> Rewrites {
    let mut declarations = Declarations::new();
    let mut edits = Vec::new();
    let mut removed = Vec::new();
    for import in imports.into_iter().filter(|import| !import.allowed) {
        let Some(tree) = ast::UseTree::cast(import.node.clone()) else {
            continue;
        };

        if tree.star_token().is_some() || tree.use_tree_list().is_some() {
            continue;
        }

        let Some(declaration) = tree.syntax().ancestors().find_map(ast::Use::cast) else {
            continue;
        };

        // Public names are part of the module interface; never silently remove or rename them.
        if declaration.visibility().is_some()
            || syntax::protected(declaration.syntax())
            || declaration
                .syntax()
                .children()
                .filter_map(ast::Attr::cast)
                .filter_map(|attribute| attribute.meta())
                .any(syntax::has_expectation)
        {
            continue;
        }

        let Some(path) = tree.path() else { continue };
        let rename_only = (import.api.as_ref().is_some_and(|api| import.name == api.name)
            || import
                .qualifier
                .as_ref()
                .is_some_and(|qualifier| import.name == qualifier.name))
            && tree.rename().is_some()
            && (import.namespace_import || import.api.as_ref().is_some_and(|api| matches!(api.form, Form::Direct)));

        if rename_only {
            plan_alias_removal(root, &tree, &declaration, &mut declarations);
            continue;
        }

        let request = RemovalRequest {
            name: &import.name,
            definition: import.definition,
            namespace: import.namespace,
            range: import.node.text_range(),
            scope: import.scope.text_range(),
            origin: import.origin,
            replacements: ranges,
            retained,
        };

        // This also protects inactive code and macro inputs which have no HIR facts.
        let unhandled = import
            .scope
            .descendants_with_tokens()
            .filter_map(NodeOrToken::into_token)
            .any(|token| {
                if declaration.syntax().text_range().contains_range(token.text_range())
                    || request.covered(token.text_range())
                    || (checks.reference)(token.text_range())
                {
                    return false;
                }

                (token.kind() == SyntaxKind::IDENT
                    && token.text().trim_start_matches("r#") == import.name
                    && unresolved_context(&token)
                    && !qualified_suffix(&token))
                    || (token.kind() == SyntaxKind::STRING
                        && token.parent_ancestors().any(|node| ast::Attr::can_cast(node.kind()))
                        && token
                            .text()
                            .split(|c: char| !c.is_alphanumeric() && c != '_')
                            .any(|word| word == import.name))
            });

        if unhandled || !(checks.remove)(request) {
            continue;
        }

        let trait_import = import.api.as_ref().is_some_and(|api| {
            matches!(api.form, Form::Direct)
                && accesses.iter().any(|access| {
                    access.kind == UseKind::Trait
                        && access.api.definition == api.definition
                        && syntax::visible_scopes(&access.node).contains(&import.scope)
                })
        });

        if trait_import {
            let Some(api) = &import.api else { continue };
            let Some(leaf) = path.segment().and_then(|segment| segment.name_ref()) else {
                continue;
            };

            if leaf.text().trim_start_matches("r#") != api.name {
                continue;
            }

            if !(checks.name)(NameRequest {
                fact: super::super::model::FactKey {
                    range: import.range,
                    definition: import.definition,
                },
                at: import.range,
                name: &api.name,
                definition: Some(api.definition),
                namespace: api.namespace,
                target: import.scope.text_range(),
                module_target: import.scope.kind() != SyntaxKind::STMT_LIST,
                reuse: false,
                external: false,
                replacement: None,
            }) {
                continue;
            }

            plan_alias_removal(root, &tree, &declaration, &mut declarations);
            continue;
        }

        // A direct alias without rewritten references still needs a lawful binding.
        if import.api.as_ref().is_some_and(|api| matches!(api.form, Form::Direct))
            && !ranges.iter().any(|range| {
                accesses
                    .iter()
                    .any(|access| *range == access.range && access.api.definition == import.definition)
            })
        {
            continue;
        }

        let range = declaration.syntax().text_range();
        let key = (u32::from(range.start()), u32::from(range.end()));
        declarations
            .entry(key)
            .or_insert_with(|| (declaration, Vec::new(), Vec::new()))
            .1
            .push(tree.syntax().text_range());
    }

    for (declaration, removals, renames) in declarations.values() {
        let Some(tree) = declaration.use_tree() else { continue };
        let Some(replacement) = rewrite(&tree, source, removals, renames) else {
            continue;
        };

        match replacement {
            Rewritten::Text(replacement) => edits.push(Edit {
                range: tree.syntax().text_range(),
                replacement,
            }),
            Rewritten::Removed => {
                // Don't orphan documentation or discard comments belonging to a removed declaration.
                if syntax::has_comments(declaration.syntax(), declaration.syntax().text_range())
                    || trailing_comment(declaration.semicolon_token())
                {
                    continue;
                }

                edits.push(Edit {
                    range: declaration.syntax().text_range(),
                    replacement: String::new(),
                });
            },
        }

        removed.extend(removals);
    }

    Rewrites { edits, removed }
}

fn plan_alias_removal(root: &SyntaxNode, tree: &ast::UseTree, declaration: &ast::Use, declarations: &mut Declarations) {
    let Some(path) = tree.path() else { return };
    let Some(rename) = tree.rename() else { return };
    let range = TextRange::new(path.syntax().text_range().end(), rename.syntax().text_range().end());
    if syntax::has_comments(root, range) {
        return;
    }

    let declaration_range = declaration.syntax().text_range();
    declarations
        .entry((u32::from(declaration_range.start()), u32::from(declaration_range.end())))
        .or_insert_with(|| (declaration.clone(), Vec::new(), Vec::new()))
        .2
        .push(Edit {
            range,
            replacement: String::new(),
        });
}

fn unresolved_context(token: &SyntaxToken) -> bool {
    let ancestors: Vec<_> = token.parent_ancestors().collect();
    let opaque = ancestors.iter().any(|node| {
        ast::Attr::can_cast(node.kind())
            || ast::TokenTree::can_cast(node.kind())
            || ast::MacroCall::can_cast(node.kind())
    });

    opaque
        || (ancestors.iter().any(syntax::has_conditional_attribute)
            && ancestors.iter().any(|node| ast::NameRef::can_cast(node.kind())))
}

fn previous(token: &SyntaxToken) -> Option<SyntaxToken> {
    let mut previous = token.prev_token()?;
    while matches!(
        previous.kind(),
        SyntaxKind::WHITESPACE | SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT
    ) {
        previous = previous.prev_token()?;
    }

    Some(previous)
}

fn qualified_suffix(token: &SyntaxToken) -> bool {
    let Some(mut separator) = previous(token).filter(|token| token.kind() == SyntaxKind::COLON2) else {
        return false;
    };

    loop {
        let Some(segment) = previous(&separator) else {
            return true;
        };

        if matches!(
            segment.kind(),
            SyntaxKind::SELF_KW | SyntaxKind::SUPER_KW | SyntaxKind::CRATE_KW
        ) {
            return false;
        }

        if segment.kind() != SyntaxKind::IDENT {
            return true;
        }

        let Some(next) = previous(&segment).filter(|token| token.kind() == SyntaxKind::COLON2) else {
            return true;
        };

        separator = next;
    }
}

enum Rewritten {
    Removed,
    Text(String),
}

// Rewrite only the affected branches. Sibling tokens, comments and formatting stay verbatim.
fn rewrite(tree: &ast::UseTree, source: &str, removals: &[TextRange], renames: &[Edit]) -> Option<Rewritten> {
    let range = tree.syntax().text_range();
    if removals.contains(&range) {
        return (!syntax::has_comments(tree.syntax(), range)).then_some(Rewritten::Removed);
    }

    let Some(list) = tree.use_tree_list() else {
        let mut text = source[range].to_owned();
        for edit in renames.iter().filter(|edit| range.contains_range(edit.range)) {
            text.replace_range(
                std::ops::Range::<usize>::from(edit.range - range.start()),
                &edit.replacement,
            );
        }

        return Some(Rewritten::Text(text));
    };

    let children: Vec<_> = list.use_trees().collect();
    let rewritten: Vec<_> = children
        .iter()
        .map(|child| rewrite(child, source, removals, renames))
        .collect::<Option<_>>()?;

    if rewritten.iter().all(|tree| matches!(tree, Rewritten::Removed)) {
        return (!syntax::has_comments(tree.syntax(), range)).then_some(Rewritten::Removed);
    }

    let mut edits = Vec::new();
    let mut index = 0;
    while index < children.len() {
        if let Rewritten::Text(replacement) = &rewritten[index] {
            if replacement != &source[children[index].syntax().text_range()] {
                edits.push(Edit {
                    range: children[index].syntax().text_range(),
                    replacement: replacement.clone(),
                });
            }

            index += 1;
            continue;
        }

        let start = index;
        while index + 1 < children.len() && matches!(rewritten[index + 1], Rewritten::Removed) {
            index += 1;
        }

        let first = children[start].syntax();
        let last = children[index].syntax();
        let trailing = last
            .siblings_with_tokens(Direction::Next)
            .skip(1)
            .find(|element| element.kind() != SyntaxKind::WHITESPACE);

        if trailing
            .as_ref()
            .is_some_and(|element| matches!(element.kind(), SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT))
        {
            return None;
        }

        let deletion = if let Some(comma) = trailing.filter(|element| element.kind() == SyntaxKind::COMMA) {
            if trailing_comment(comma.as_token().cloned()) {
                return None;
            }

            TextRange::new(first.text_range().start(), comma.text_range().end())
        } else {
            let comma = first
                .siblings_with_tokens(Direction::Prev)
                .skip(1)
                .find(|element| element.kind() != SyntaxKind::WHITESPACE)?;

            if comma.kind() != SyntaxKind::COMMA {
                return None;
            }

            TextRange::new(comma.text_range().start(), last.text_range().end())
        };

        if syntax::has_comments(tree.syntax(), deletion) {
            return None;
        }

        edits.push(Edit {
            range: deletion,
            replacement: String::new(),
        });

        index += 1;
    }

    edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
    let mut text = source[range].to_owned();
    for edit in edits {
        let relative = edit.range - range.start();
        text.replace_range(std::ops::Range::<usize>::from(relative), &edit.replacement);
    }

    Some(Rewritten::Text(text))
}

fn trailing_comment(token: Option<SyntaxToken>) -> bool {
    let Some(mut next) = token.and_then(|token| token.next_token()) else {
        return false;
    };

    loop {
        match next.kind() {
            SyntaxKind::WHITESPACE if !next.text().contains('\n') => {},
            SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT => return true,
            _ => return false,
        }

        let Some(token) = next.next_token() else { return false };
        next = token;
    }
}
