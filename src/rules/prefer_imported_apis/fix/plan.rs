use std::collections::BTreeMap;

use ra_ap_syntax::{
    SyntaxKind,
    SyntaxNode,
};
use rustc_hir::def::Namespace;
use rustc_span::def_id::DefId;

use super::{
    super::{
        analysis::{
            Access,
            Import,
            visible_scopes,
        },
        names::NameRequest,
        policy::{
            Api,
            Form,
        },
    },
    imports as edits,
};
use crate::{
    diagnostic::{
        Edit,
        Fix,
        RuleOutput,
    },
    syntax,
};

#[derive(Clone, PartialEq, Eq)]
struct Binding {
    definition: DefId,
    name: String,
    namespace: Namespace,
    external: bool,
}

impl Binding {
    fn for_api(api: &Api) -> Self {
        match &api.form {
            Form::Direct => Self {
                definition: api.definition,
                name: api.name.clone(),
                namespace: api.namespace,
                external: false,
            },
            Form::Qualified { qualifier, .. } => Self {
                definition: qualifier.definition,
                name: qualifier.name.to_owned(),
                namespace: Namespace::TypeNS,
                external: qualifier.external,
            },
        }
    }

    fn same_name(&self, other: &Self) -> bool {
        self.name == other.name && self.namespace == other.namespace
    }
}

struct Planned {
    binding: Binding,
    scope: SyntaxNode,
    issue: usize,
}

struct Addition {
    binding: Binding,
    statement: String,
}

pub(in super::super) fn plan(
    root: &SyntaxNode,
    source: &str,
    imports: &[Import],
    requests: &[(usize, &Access)],
    safe_name: &impl Fn(NameRequest<'_>) -> bool,
    output: &mut RuleOutput,
) {
    let mut groups: BTreeMap<_, (SyntaxNode, Vec<(usize, &Access)>)> = BTreeMap::new();
    for &(issue, access) in requests {
        if syntax::protected(&access.node) || syntax::has_comments(root, access.range) {
            continue;
        }

        let target = &access.scope;
        let range = target.text_range();
        groups
            .entry((u32::from(range.start()), u32::from(range.end())))
            .or_insert_with(|| (target.clone(), Vec::new()))
            .1
            .push((issue, access));
    }

    let mut planned = Vec::<Planned>::new();
    for (target, group) in groups.values() {
        let insertion = edits::insertion(target, source);
        let mut additions = Vec::<Addition>::new();
        let mut replacements = Vec::new();
        let mut first_issue = None;
        for &(issue, access) in group {
            let binding = Binding::for_api(&access.api);
            let scopes = visible_scopes(&access.node);
            let existing = imports.iter().find(|import| {
                import.allowed
                    && !import.conditional
                    && import.definition == binding.definition
                    && import.namespace == binding.namespace
                    && import.name == binding.name
                    && scopes.contains(&import.scope)
                    && import.node.text_range().end() <= access.range.start()
            });

            let previous = planned
                .iter()
                .find(|import| import.binding == binding && scopes.contains(&import.scope));

            let reuse = binding.external || existing.is_some() || previous.is_some();
            let destination = previous.map_or_else(
                || existing.map_or(target, |import| &import.scope),
                |import| &import.scope,
            );

            if !safe_name(NameRequest {
                at: access.range,
                name: &binding.name,
                definition: binding.definition,
                namespace: binding.namespace,
                target: destination.text_range(),
                module_target: destination.kind() != SyntaxKind::STMT_LIST,
                reuse,
                external: binding.external,
            }) {
                continue;
            }

            let replacement = Edit {
                range: access.range,
                replacement: access.api.canonical(),
            };

            if let Some(previous) = previous {
                if let Some(fix) = &mut output.issues[previous.issue].fix {
                    fix.edits.push(replacement);
                }

                continue;
            }

            if !reuse {
                if access.conditional
                    || insertion.is_none()
                    || has_conflict(&binding, target, &scopes, &additions, &planned)
                {
                    continue;
                }

                let Some(addition) = addition(access, binding.clone(), target, source, safe_name) else {
                    continue;
                };

                if !additions.iter().any(|other| other.binding == binding) {
                    additions.push(addition);
                }
            }

            replacements.push(replacement);
            first_issue.get_or_insert(issue);
        }

        let Some(issue) = first_issue else { continue };
        if !additions.is_empty() {
            let Some(insertion) = insertion else {
                continue;
            };

            let edit = insertion.edit(additions.iter().map(|addition| addition.statement.as_str()));

            planned.extend(additions.iter().map(|addition| Planned {
                binding: addition.binding.clone(),
                scope: target.clone(),
                issue,
            }));

            replacements.push(edit);
        }

        output.issues[issue].fix = Some(Fix { edits: replacements });
    }
}

fn has_conflict(
    binding: &Binding,
    scope: &SyntaxNode,
    scopes: &[SyntaxNode],
    additions: &[Addition],
    planned: &[Planned],
) -> bool {
    additions
        .iter()
        .any(|other| other.binding.same_name(binding) && other.binding != *binding)
        || planned.iter().any(|other| {
            other.binding.same_name(binding)
                && other.binding != *binding
                && (other.scope == *scope || scopes.contains(&other.scope))
        })
}

fn addition(
    access: &Access,
    binding: Binding,
    target: &SyntaxNode,
    source: &str,
    safe_name: &impl Fn(NameRequest<'_>) -> bool,
) -> Option<Addition> {
    let path = match &access.api.form {
        Form::Direct => {
            // Do not invent a forbidden leaf alias when the written route names
            // a differently named reexport. An existing lawful binding is reusable.
            if access.spelling.rsplit("::").next()? != access.api.name {
                return None;
            }

            let path = source[access.range].trim();
            if path.contains(['<', '>']) {
                return None;
            }

            path.to_owned()
        },
        Form::Qualified { qualifier, .. } => {
            let root = qualifier.source.split("::").next()?;
            if !safe_name(NameRequest {
                at: access.range,
                name: root,
                definition: qualifier.root,
                namespace: Namespace::TypeNS,
                target: target.text_range(),
                module_target: target.kind() != SyntaxKind::STMT_LIST,
                reuse: true,
                external: true,
            }) {
                return None;
            }

            qualifier.source.to_owned()
        },
    };

    let leaf = path.rsplit("::").next()?;
    let statement = if matches!(access.api.form, Form::Direct) || leaf == binding.name {
        format!("use {path};")
    } else {
        format!("use {path} as {};", binding.name)
    };

    Some(Addition { binding, statement })
}
