use std::collections::{
    BTreeMap,
    BTreeSet,
};

use kikiutils::types::fx_collections::FxHashSet;
use ra_ap_syntax::{
    AstNode,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    TextSize,
    ast,
};

use super::{
    Api,
    resolution::NameRequest,
};
use crate::{
    diagnostic::{
        Diagnostic,
        Edit,
        Fix,
        RuleOutput,
    },
    syntax,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum Kind {
    Import { name: String },
    Glob,
    CrateAlias,
    Access,
    Macro,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ApiUse {
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
    target: ImportTarget,
    qualified: bool,
}

type FixGroups<'a> = BTreeMap<(bool, u32, u32), Vec<(usize, &'a Access)>>;

// The edit destination is part of the batching key, not incidental access state.
#[derive(Clone)]
enum ImportTarget {
    Scope(SyntaxNode),
    Group { tree: ast::UseTree, scope: SyntaxNode },
}

impl ImportTarget {
    fn scope(&self) -> SyntaxNode {
        match self {
            Self::Scope(node) => node.clone(),
            Self::Group { scope, .. } => scope.clone(),
        }
    }

    fn key(&self) -> (bool, u32, u32) {
        let (group, node) = match self {
            Self::Scope(node) => (false, node),
            Self::Group { tree, .. } => (true, tree.syntax()),
        };

        let range = node.text_range();
        (group, range.start().into(), range.end().into())
    }
}

fn conditional_import(import: &Import) -> bool {
    import.tree.syntax().ancestors().filter_map(ast::Use::cast).any(|item| {
        item.syntax()
            .children()
            .filter_map(ast::Attr::cast)
            .any(|attr| syntax::conditional_attribute(&attr))
    })
}

fn scope(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.ancestors().find(syntax::is_scope)
}

fn visible_scopes(node: &SyntaxNode) -> Vec<SyntaxNode> {
    let mut scopes = Vec::new();
    for node in node.ancestors().filter(syntax::is_scope) {
        let module = node.kind() != SyntaxKind::STMT_LIST;
        scopes.push(node);
        if module {
            break;
        }
    }

    scopes
}

pub(super) fn analyze<'a>(
    root: &SyntaxNode,
    source: &str,
    facts: impl IntoIterator<Item = (TextRange, &'a ApiUse)>,
    resolve_name: impl Fn(NameRequest<'_>) -> bool,
) -> RuleOutput {
    let mut output = RuleOutput::default();
    let mut imports = Vec::new();
    let mut accesses = Vec::new();
    let mut seen = FxHashSet::default();
    for (fact_range, fact) in facts.into_iter().filter(|entry| seen.insert(*entry)) {
        match &fact.kind {
            Kind::Import { name } => {
                let Some(tree) = root
                    .covering_element(fact_range)
                    .ancestors()
                    .find_map(ast::UseTree::cast)
                else {
                    continue;
                };

                let Some(scope) = scope(tree.syntax()) else { continue };
                let renamed = tree.rename().is_some();
                let allowed = match fact.api {
                    Api::Crate => !renamed,
                    Api::Result => fact.api.allows(name) && (!renamed || name == "AnyhowResult"),
                    Api::Error => name == "AnyhowError",
                    _ => fact.api.allows(name) && !renamed,
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
                    .any(|issue| issue.diagnostics[0].range == fact_range)
                {
                    output.report(Diagnostic {
                        range: fact_range,
                        message: format!(
                            "import this anyhow API as `{}` without an unsupported alias",
                            fact.api.name()
                        ),
                    });
                }
            },
            Kind::CrateAlias => {
                if root
                    .covering_element(fact_range)
                    .ancestors()
                    .filter_map(ast::ExternCrate::cast)
                    .any(|item| item.syntax().text_range().contains_range(fact_range))
                {
                    output.report(Diagnostic {
                        range: fact_range,
                        message: "do not alias the anyhow crate".to_owned(),
                    });
                }
            },
            Kind::Glob => {
                if root
                    .covering_element(fact_range)
                    .ancestors()
                    .filter_map(ast::Use::cast)
                    .any(|item| item.syntax().text_range().contains_range(fact_range))
                {
                    output.report(Diagnostic {
                        range: fact_range,
                        message: format!(
                            "this glob imports an anyhow API without its required name `{}`; use explicit imports",
                            fact.api.name()
                        ),
                    });
                }
            },
            Kind::Access | Kind::Macro => {
                let (node, range) = if fact.kind == Kind::Macro {
                    // The call-site span may include the trailing semicolon.
                    // Start inside its path so the MacroCall remains an ancestor.
                    let start = TextRange::new(fact_range.start(), fact_range.start() + TextSize::from(1));

                    let Some(call) = syntax::enclosing::<ast::MacroCall>(root, start)
                        .filter(|call| call.syntax().text_range().start() == fact_range.start())
                    else {
                        continue;
                    };

                    let Some(path) = call.path() else { continue };
                    (path.syntax().clone(), path.syntax().text_range())
                } else {
                    let Some(path) = root
                        .covering_element(fact_range)
                        .ancestors()
                        .filter_map(ast::Path::cast)
                        .find(|path| path.syntax().text_range().start() == fact_range.start())
                    else {
                        continue;
                    };

                    if path.syntax().ancestors().any(|node| ast::Use::can_cast(node.kind())) {
                        continue;
                    }

                    (path.syntax().clone(), fact_range)
                };

                let Some(scope) = scope(&node) else { continue };
                let qualified = source[range].contains("::");
                if !accesses
                    .iter()
                    .any(|access: &Access| access.range == range && access.api == fact.api)
                {
                    accesses.push(Access {
                        api: fact.api,
                        range,
                        node,
                        target: ImportTarget::Scope(scope),
                        qualified,
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
        if syntax::crosses_conditional(&access.node, &module) {
            continue;
        }

        let path = &source[access.range];
        let Some((prefix, leaf)) = path.rsplit_once("::") else {
            continue;
        };

        if leaf.trim() != access.api.name() {
            continue;
        }

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
                            && !syntax::has_comments(root, tree.syntax().text_range())
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
            access.target = ImportTarget::Group { tree, scope: module };
        }
    }

    // Reserve one binding per module/API, including module-level expressions
    // that occur before an otherwise mergeable import. The destination must
    // remain above every use, so an earlier scope insertion wins over a group.
    let mut bindings: BTreeMap<_, ImportTarget> = BTreeMap::new();
    for access in &accesses {
        let scope = access.target.scope();
        if !matches!(access.api, Api::Anyhow | Api::Bail | Api::Ensure)
            || scope.kind() == SyntaxKind::STMT_LIST
            || leaf(&source[access.range]) != access.api.name()
        {
            continue;
        }

        let range = scope.text_range();
        bindings
            .entry((range.start(), range.end(), access.api))
            .and_modify(|target| {
                if access.target.key().1 < target.key().1 {
                    *target = access.target.clone();
                }
            })
            .or_insert_with(|| access.target.clone());
    }

    for access in &mut accesses {
        let scope = access.target.scope();
        let range = scope.text_range();
        if let Some(target) = bindings.get(&(range.start(), range.end(), access.api)) {
            access.target = target.clone();
        }
    }

    let mut groups = FixGroups::new();
    for access in &accesses {
        if !access.qualified {
            let name = source[access.range].trim_start_matches("r#").to_owned();
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

        groups.entry(access.target.key()).or_default().push((index, access));
    }

    for group in groups.values() {
        let mut edits = Vec::new();
        let mut additions = BTreeMap::new();
        let mut first_issue = None;
        for (index, access) in group {
            if syntax::protected(&access.node) || syntax::has_comments(root, access.range) {
                continue;
            }

            let scopes = visible_scopes(&access.node);
            let existing = scopes.iter().find_map(|scope| {
                imports.iter().find(|import| {
                    import.api == access.api
                        && import.scope == *scope
                        && !conditional_import(import)
                        && import.tree.syntax().text_range().end() <= access.range.start()
                })
            });

            let name = existing.map_or(access.api.name(), |import| import.name.as_str());
            if !safe_target(access, existing)
                || !resolve_name(NameRequest {
                    at: access.range,
                    api: access.api,
                    name,
                    target: access.target.scope().text_range(),
                    module_target: access.target.scope().kind() != SyntaxKind::STMT_LIST,
                    reuse: existing.is_some(),
                })
            {
                continue;
            }

            if existing.is_none() && access.api != Api::Error && leaf(&source[access.range]) != name {
                // Do not introduce a macro alias or "Result as Result" while
                // fixing a route that already uses a re-exported alias.
                continue;
            }

            if existing.is_none() {
                additions
                    .entry((access.api, &source[access.range]))
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
            let scope = group[0].1.target.scope();
            if let ImportTarget::Group { tree, .. } = &group[0].1.target {
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

            let preamble_end = scope
                .children()
                .find(|node| !ast::AnyAttr::can_cast(node.kind()))
                .map_or(scope.text_range().end(), |node| node.text_range().start());

            if syntax::has_comments(root, TextRange::new(scope.text_range().start(), preamble_end)) {
                continue;
            }

            let offset = if scope.kind() == SyntaxKind::SOURCE_FILE {
                preamble_end
            } else {
                let Some(token) = scope.first_token() else { continue };
                scope
                    .children()
                    .filter_map(ast::Attr::cast)
                    .last()
                    .map_or(token.text_range().end(), |attr| attr.syntax().text_range().end())
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
                statements.push_str(path);
                if leaf(path) != name {
                    statements.push_str(" as ");
                    statements.push_str(&name);
                }

                statements.push(';');
                statements.push('\n');
            }

            if scope.kind() != SyntaxKind::SOURCE_FILE {
                statements.insert(0, '\n');
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

// The last path segment, as written.
fn leaf(path: &str) -> &str {
    path.rsplit_once("::").map_or(path, |(_, leaf)| leaf).trim()
}

fn safe_target(access: &Access, existing: Option<&Import>) -> bool {
    // A conditional item initializer without a block offers no local place to
    // insert an import. Do not lift a new binding outside that cfg boundary.
    let insertion_scope = access.target.scope();
    !(existing.is_none()
        && insertion_scope.kind() != SyntaxKind::STMT_LIST
        && syntax::crosses_conditional(&access.node, &insertion_scope))
}
