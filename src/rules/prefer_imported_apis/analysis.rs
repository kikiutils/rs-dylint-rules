use std::collections::{
    BTreeMap,
    BTreeSet,
};

use ra_ap_syntax::{
    AstNode,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    ast,
};

use super::Api;
use crate::diagnostic::{
    Diagnostic,
    Edit,
    Fix,
    RuleOutput,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Kind {
    Import { name: String },
    Glob,
    CrateAlias,
    Access,
    Macro,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Fact {
    pub(super) range: TextRange,
    pub(super) api: Api,
    pub(super) kind: Kind,
}

struct Import {
    api: Api,
    name: String,
    tree: ast::UseTree,
    scope: SyntaxNode,
}

struct Access {
    api: Api,
    range: TextRange,
    node: SyntaxNode,
    scope: SyntaxNode,
    qualified: bool,
    merge: Option<ast::UseTree>,
}

fn scope(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.ancestors().find(|node| {
        matches!(
            node.kind(),
            SyntaxKind::STMT_LIST | SyntaxKind::ITEM_LIST | SyntaxKind::SOURCE_FILE
        )
    })
}

fn visible_scopes(node: &SyntaxNode) -> Vec<SyntaxNode> {
    let mut scopes = Vec::new();
    for node in node.ancestors() {
        if matches!(
            node.kind(),
            SyntaxKind::STMT_LIST | SyntaxKind::ITEM_LIST | SyntaxKind::SOURCE_FILE
        ) {
            let module = node.kind() != SyntaxKind::STMT_LIST;
            scopes.push(node);
            if module {
                break;
            }
        }
    }
    scopes
}

fn protected(node: &SyntaxNode) -> bool {
    node.ancestors().any(|node| {
        node.children()
            .filter_map(ast::Attr::cast)
            .any(|attr| !safe_attribute(&attr))
    })
}

// These built-ins neither generate bindings nor inspect the surrounding tokens.
// Keep cfg_attr conservative: it can enable an arbitrary attribute macro.
fn safe_attribute(attribute: &ast::Attr) -> bool {
    attribute
        .meta()
        .and_then(|meta| meta.simple_name())
        .is_some_and(|name| {
            matches!(
                name.as_str(),
                "cfg"
                    | "allow"
                    | "warn"
                    | "deny"
                    | "forbid"
                    | "expect"
                    | "doc"
                    | "deprecated"
                    | "must_use"
                    | "non_exhaustive"
                    | "repr"
            )
        })
}

fn text(root: &SyntaxNode, range: TextRange) -> String {
    root.text().to_string()[std::ops::Range::<usize>::from(range - root.text_range().start())].to_owned()
}

pub(super) fn analyze(root: &SyntaxNode, facts: &[Fact], local_macros: &BTreeSet<String>) -> RuleOutput {
    let mut output = RuleOutput::default();
    let mut imports = Vec::new();
    let mut accesses = Vec::new();
    for fact in facts {
        match &fact.kind {
            Kind::Import { name } => {
                let Some(tree) = root
                    .descendants()
                    .filter_map(ast::UseTree::cast)
                    .filter(|tree| tree.syntax().text_range().contains_range(fact.range))
                    .min_by_key(|tree| tree.syntax().text_range().len())
                else {
                    continue;
                };
                let Some(scope) = scope(tree.syntax()) else { continue };
                let renamed = tree.rename().is_some();
                let allowed = if fact.api == Api::Crate {
                    !renamed
                } else {
                    fact.api.allows(name)
                        && (!renamed
                            || match fact.api {
                                Api::Result => name == "AnyhowResult",
                                Api::Error => name == "AnyhowError",
                                _ => false,
                            })
                };
                if allowed {
                    imports.push(Import {
                        api: fact.api,
                        name: name.clone(),
                        tree,
                        scope,
                    });
                } else if !output
                    .issues
                    .iter()
                    .any(|issue| issue.diagnostics[0].range == fact.range)
                {
                    output.report(Diagnostic {
                        range: fact.range,
                        message: format!(
                            "import this anyhow API as `{}` without an unsupported alias",
                            fact.api.name()
                        ),
                    });
                }
            },
            Kind::CrateAlias => {
                if root
                    .descendants()
                    .filter_map(ast::ExternCrate::cast)
                    .any(|item| item.syntax().text_range().contains_range(fact.range))
                {
                    output.report(Diagnostic {
                        range: fact.range,
                        message: "do not alias the anyhow crate".to_owned(),
                    });
                }
            },
            Kind::Glob => {
                if root
                    .descendants()
                    .filter_map(ast::Use::cast)
                    .any(|item| item.syntax().text_range().contains_range(fact.range))
                {
                    output.report(Diagnostic {
                        range: fact.range,
                        message: format!(
                            "this glob imports an anyhow API without its required name `{}`; use explicit imports",
                            fact.api.name()
                        ),
                    });
                }
            },
            Kind::Access | Kind::Macro => {
                let (node, range) = if fact.kind == Kind::Macro {
                    let Some(call) = root.descendants().filter_map(ast::MacroCall::cast).find(|call| {
                        call.syntax().text_range().start() == fact.range.start()
                            && (fact.range.contains_range(call.syntax().text_range())
                                || call.syntax().text_range().contains_range(fact.range))
                    }) else {
                        continue;
                    };
                    let Some(path) = call.path() else { continue };
                    (path.syntax().clone(), path.syntax().text_range())
                } else {
                    let Some(path) = root
                        .descendants()
                        .filter_map(ast::Path::cast)
                        .filter(|path| {
                            path.syntax().text_range().start() == fact.range.start()
                                && path.syntax().text_range().contains_range(fact.range)
                        })
                        .min_by_key(|path| path.syntax().text_range().len())
                    else {
                        continue;
                    };
                    if path.syntax().ancestors().any(|node| ast::Use::can_cast(node.kind())) {
                        continue;
                    }
                    (path.syntax().clone(), fact.range)
                };
                let Some(scope) = scope(&node) else { continue };
                let qualified = text(root, range).contains("::");
                if !accesses
                    .iter()
                    .any(|access: &Access| access.range == range && access.api == fact.api)
                {
                    accesses.push(Access {
                        api: fact.api,
                        range,
                        node,
                        scope,
                        qualified,
                        merge: None,
                    });
                }
            },
        }
    }
    // Prefer an existing, unconditional module import from the same route.
    // Only compiler-confirmed API imports can serve as anchors.
    for access in &mut accesses {
        if !matches!(access.api, Api::Anyhow | Api::Bail | Api::Ensure) || !access.qualified {
            continue;
        }
        let Some(module) = visible_scopes(&access.node).last().cloned() else {
            continue;
        };
        // Do not lift an import out of a conditional function/block/impl.
        // Attributes on the module itself guard both its imports and uses.
        let crosses_cfg = access.node.ancestors().take_while(|node| *node != module).any(|node| {
            node.children().filter_map(ast::Attr::cast).any(|attr| {
                attr.meta()
                    .and_then(|meta| meta.simple_name())
                    .is_some_and(|name| matches!(name.as_str(), "cfg" | "cfg_attr"))
            })
        });
        if crosses_cfg {
            continue;
        }
        let path = text(root, access.range);
        let Some((prefix, _)) = path.rsplit_once("::") else {
            continue;
        };
        let candidate = imports
            .iter()
            .filter(|import| import.scope == module)
            .find_map(|import| {
                import
                    .tree
                    .syntax()
                    .ancestors()
                    .filter_map(ast::UseTree::cast)
                    .find(|tree| {
                        tree.use_tree_list().is_some()
                            && tree.path().is_some_and(|path| path.syntax().text() == prefix)
                            && tree.syntax().text_range().end() <= access.range.start()
                            && !has_comments(root, tree.syntax().text_range())
                            && tree.syntax().ancestors().filter_map(ast::Use::cast).all(|item| {
                                !item.syntax().children().any(|child| ast::Attr::can_cast(child.kind()))
                                    && !item
                                        .syntax()
                                        .children()
                                        .any(|child| ast::Visibility::can_cast(child.kind()))
                            })
                    })
            });
        if let Some(tree) = candidate {
            access.scope = module;
            access.merge = Some(tree);
        }
    }
    let mut groups: BTreeMap<(u32, u32), Vec<(usize, &Access)>> = BTreeMap::new();
    for access in &accesses {
        if !access.qualified {
            let name = text(root, access.range).trim_start_matches("r#").to_owned();
            let scopes = visible_scopes(&access.node);
            let matching: Vec<_> = imports
                .iter()
                .filter(|import| import.api == access.api && import.name == name && scopes.contains(&import.scope))
                .collect();
            if !matching.is_empty()
                && matching
                    .iter()
                    .all(|import| import.tree.syntax().text_range().end() > access.range.start())
            {
                output.report(Diagnostic {
                    range: access.range,
                    message: format!("place the import for `{name}` above this use"),
                });
            }
            continue;
        }
        let index = output.issues.len();
        output.report(Diagnostic {
            range: access.range,
            message: format!("import this anyhow API first and use `{}`", access.api.name()),
        });
        let range = access.scope.text_range();
        groups
            .entry((range.start().into(), range.end().into()))
            .or_default()
            .push((index, access));
    }
    for group in groups.values() {
        let mut edits = Vec::new();
        let mut additions = BTreeMap::new();
        let mut first_issue = None;
        for (index, access) in group {
            if protected(&access.node) || has_comments(root, access.range) {
                continue;
            }
            let scopes = visible_scopes(&access.node);
            let existing = scopes.iter().find_map(|scope| {
                imports.iter().find(|import| {
                    import.api == access.api
                        && import.scope == *scope
                        && import.tree.syntax().text_range().end() <= access.range.start()
                })
            });
            let name = existing.map_or(access.api.name(), |import| import.name.as_str());
            if !safe_name(access, name, existing, &accesses, local_macros) {
                continue;
            }
            if existing.is_none()
                && access.api != Api::Error
                && text(root, access.range).rsplit("::").next().map(str::trim) != Some(name)
            {
                // Do not introduce a macro alias or "Result as Result" while
                // fixing a route that already uses a re-exported alias.
                continue;
            }
            if existing.is_none() {
                additions
                    .entry((access.api, text(root, access.range)))
                    .or_insert_with(|| name.to_owned());
            }
            edits.push(Edit {
                range: access.range,
                replacement: name.to_owned(),
            });
            first_issue.get_or_insert(*index);
        }
        let Some(index) = first_issue else { continue };
        if !additions.is_empty() {
            let scope = &group[0].1.scope;
            if let Some(tree) = group.iter().find_map(|(_, access)| access.merge.as_ref()) {
                let Some(list) = tree.use_tree_list() else { continue };
                let Some(last) = list.use_trees().last() else { continue };
                let names: BTreeSet<_> = additions.into_values().collect();
                edits.push(Edit {
                    range: TextRange::empty(last.syntax().text_range().end()),
                    replacement: format!(", {}", names.into_iter().collect::<Vec<_>>().join(", ")),
                });
                output.issues[index].fix = Some(Fix { edits });
                continue;
            }
            let offset = if scope.kind() == SyntaxKind::SOURCE_FILE {
                scope
                    .children()
                    .find(|node| !ast::Attr::can_cast(node.kind()))
                    .map_or(scope.text_range().end(), |node| node.text_range().start())
            } else {
                let Some(token) = scope.first_token() else { continue };
                scope
                    .children()
                    .filter_map(ast::Attr::cast)
                    .last()
                    .map_or(token.text_range().end(), |attr| attr.syntax().text_range().end())
            };
            if scope.kind() == SyntaxKind::SOURCE_FILE
                && has_comments(root, TextRange::new(root.text_range().start(), offset))
            {
                continue;
            }
            let newline = if root.text().to_string().contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            let mut statements = String::new();
            // Multiple qualified routes to the same API need only one binding.
            let mut names = Vec::new();
            for ((_, path), name) in additions {
                if names.contains(&name) {
                    continue;
                }
                names.push(name.clone());
                statements.push_str("use ");
                statements.push_str(&path);
                if path.rsplit("::").next().map(str::trim) != Some(name.as_str()) {
                    statements.push_str(" as ");
                    statements.push_str(&name);
                }
                statements.push(';');
                statements.push_str(newline);
            }
            if scope.kind() != SyntaxKind::SOURCE_FILE {
                statements.insert_str(0, newline);
            }
            edits.push(Edit {
                range: TextRange::empty(offset),
                replacement: statements,
            });
        }
        output.issues[index].fix = Some(Fix { edits });
    }
    output
}

fn has_comments(root: &SyntaxNode, range: TextRange) -> bool {
    root.descendants_with_tokens().any(|element| {
        range.contains_range(element.text_range())
            && matches!(element.kind(), SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT)
    })
}

fn safe_name(
    access: &Access,
    name: &str,
    existing: Option<&Import>,
    accesses: &[Access],
    local_macros: &BTreeSet<String>,
) -> bool {
    // Textually scoped macro_rules! can win over an imported macro, including
    // definitions in parent files or generated by another macro.
    if matches!(access.api, Api::Anyhow | Api::Bail | Api::Ensure) && local_macros.contains(name) {
        return false;
    }
    // Check every visible lexical scope, not just the insertion scope: a local
    // binding or generic can shadow an otherwise valid module import.
    for scope in visible_scopes(&access.node) {
        // A new module binding can flow into child files through super::*.
        // Without examining those consumers, keep module insertions conservative.
        if existing.is_none()
            && scope == access.scope
            && scope.kind() != SyntaxKind::STMT_LIST
            && access.merge.is_none()
            && scope.children().any(|node| ast::Module::can_cast(node.kind()))
        {
            return false;
        }
        let macro_api = matches!(access.api, Api::Anyhow | Api::Bail | Api::Ensure);
        let check_hidden = existing.map_or(scope == access.scope, |import| macro_api || import.scope != scope);
        // Derive/attribute macros can emit bindings missing from the CST.
        if check_hidden
            && scope
                .descendants()
                .filter_map(ast::Attr::cast)
                .any(|attr| !safe_attribute(&attr))
        {
            return false;
        }
        for element in scope.descendants_with_tokens() {
            if element.kind() != SyntaxKind::IDENT || element.to_string().trim_start_matches("r#") != name {
                continue;
            }
            let Some(parent) = element.parent() else { continue };
            if visible_scopes(&parent).last() != visible_scopes(&access.node).last() {
                continue;
            }
            let range = element.text_range();
            if accesses.iter().any(|other| other.range.contains_range(range)) {
                continue;
            }
            if existing.is_some_and(|import| import.tree.syntax().text_range().contains_range(range)) {
                continue;
            }
            if let Some(path) = parent.ancestors().find_map(ast::Path::cast) {
                if let Some(tree) = path.syntax().ancestors().find_map(ast::UseTree::cast) {
                    if tree.rename().is_some()
                        || tree.use_tree_list().is_some()
                        || tree
                            .path()
                            .is_some_and(|full| full.syntax().text_range() != path.syntax().text_range())
                    {
                        continue;
                    }
                } else if path.qualifier().is_some()
                    || (matches!(access.api, Api::Anyhow | Api::Bail | Api::Ensure)
                        && !path.syntax().ancestors().filter_map(ast::MacroCall::cast).any(|call| {
                            call.path()
                                .is_some_and(|full| full.syntax().text_range() == path.syntax().text_range())
                        }))
                {
                    continue;
                }
            }
            return false;
        }
        if existing.is_none()
            && scope
                .descendants()
                .filter_map(ast::UseTree::cast)
                .any(|tree| tree.star_token().is_some())
        {
            return false;
        }
        // Unknown macros can introduce bindings absent from the original CST.
        if check_hidden
            && scope.descendants().filter_map(ast::MacroCall::cast).any(|call| {
                !accesses.iter().any(|other| {
                    call.path()
                        .is_some_and(|path| path.syntax().text_range() == other.range)
                })
            })
        {
            return false;
        }
    }
    true
}
