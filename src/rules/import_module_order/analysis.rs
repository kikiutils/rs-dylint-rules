use ra_ap_syntax::{
    AstNode,
    SyntaxKind,
    SyntaxNode,
    ast::{
        self,
        HasAttrs,
    },
};

use super::fix::reorder;
use crate::diagnostic::{
    Diagnostic,
    Fix,
    Issue,
    RuleOutput,
};

// Enums
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Group {
    External,
    Module,
    Local,
}

// Structs
pub(super) struct Entry {
    pub(super) node: SyntaxNode,
    pub(super) group: Group,
}

// Functions

/// Resolution and activation come from the compiler; ordering uses original CST nodes.
pub(super) fn analyze(
    root: &SyntaxNode,
    resolve: &impl Fn(&ast::Path) -> Option<Group>,
    is_active: &impl Fn(&SyntaxNode) -> bool,
) -> RuleOutput {
    let mut analysis = RuleOutput::default();

    for scope in root.descendants().filter(|node| {
        matches!(
            node.kind(),
            SyntaxKind::SOURCE_FILE | SyntaxKind::ITEM_LIST | SyntaxKind::STMT_LIST
        )
    }) {
        if is_active(&scope) {
            check_scope(&scope, resolve, is_active, &mut analysis);
        }
    }

    analysis
}

fn check_scope(
    scope: &SyntaxNode,
    resolve: &impl Fn(&ast::Path) -> Option<Group>,
    is_active: &impl Fn(&SyntaxNode) -> bool,
    analysis: &mut RuleOutput,
) {
    let mut entries = Vec::new();
    let mut uncertain = false;

    for node in scope.children() {
        if !is_active(&node) {
            continue;
        }
        if let Some(import) = ast::Use::cast(node.clone()) {
            let Some(tree) = import.use_tree() else {
                continue;
            };

            let mut groups = Vec::new();
            let complete = collect_groups(&tree, resolve, &mut groups);
            groups.sort_unstable();
            groups.dedup();

            if !complete || groups.is_empty() {
                uncertain = true;
                analysis.report(Diagnostic {
                    range: node.text_range(),
                    message: "cannot safely classify this import; its module origin is unresolved or differs between analysis contexts".to_owned(),
                });
            } else if groups.len() != 1 {
                uncertain = true;
                analysis.report(Diagnostic {
                    range: node.text_range(),
                    message: "external and local imports must not be mixed in the same use declaration".to_owned(),
                });
            } else {
                entries.push(Entry { node, group: groups[0] });
            }
        } else if ast::Module::cast(node.clone())
            .is_some_and(|module| module.semicolon_token().is_some() && !is_test_module(&module))
        {
            entries.push(Entry {
                node,
                group: Group::Module,
            });
        }
    }

    let mut highest = Group::External;
    let mut violations = Vec::new();

    for entry in &entries {
        if entry.group < highest {
            violations.push(Diagnostic {
                range: entry.node.text_range(),
                message: "expected external imports, then external mod declarations, then local imports".to_owned(),
            });
        }

        highest = highest.max(entry.group);
    }

    if violations.is_empty() {
        return;
    }

    if !uncertain && let Some(edit) = reorder(scope, &entries) {
        analysis.issues.push(Issue {
            diagnostics: violations,
            fix: Some(Fix { edits: vec![edit] }),
        });
    } else {
        for diagnostic in violations {
            analysis.report(diagnostic);
        }
    }
}

fn is_test_module(module: &ast::Module) -> bool {
    module.attrs().any(|attribute| {
        let Some(ast::Meta::CfgMeta(meta)) = attribute.meta() else {
            return false;
        };
        let Some(ast::CfgPredicate::CfgAtom(atom)) = meta.cfg_predicate() else {
            return false;
        };
        atom.eq_token().is_none() && atom.ident_token().is_some_and(|token| token.text() == "test")
    })
}

fn collect_groups(
    tree: &ast::UseTree,
    resolve: &impl Fn(&ast::Path) -> Option<Group>,
    groups: &mut Vec<Group>,
) -> bool {
    if let Some(mut path) = tree.path() {
        while let Some(qualifier) = path.qualifier() {
            path = qualifier;
        }

        let Some(segment) = path.segment() else {
            return false;
        };

        let group = match segment.kind() {
            Some(ast::PathSegmentKind::CrateKw | ast::PathSegmentKind::SelfKw | ast::PathSegmentKind::SuperKw) => {
                Some(Group::Local)
            },
            _ => resolve(&path),
        };

        if let Some(group) = group {
            groups.push(group);
            return true;
        }

        return false;
    }

    let Some(list) = tree.use_tree_list() else {
        return false;
    };

    let mut complete = true;

    for tree in list.use_trees() {
        complete &= collect_groups(&tree, resolve, groups);
    }

    complete
}
