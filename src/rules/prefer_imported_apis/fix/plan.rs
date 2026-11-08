use ra_ap_syntax::{
    AstNode,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    ast,
};
use rustc_hir::def::Namespace;
use rustc_span::def_id::DefId;

use super::{
    super::{
        analysis::{
            Access,
            Import,
            bindings::{
                Binding,
                existing_import,
                root_available,
            },
        },
        names::{
            NameRequest,
            RemovalRequest,
            ReplacedName,
        },
        policy::Form,
    },
    imports as edits,
    rewrite_imports,
};
use crate::{
    diagnostic::{
        Edit,
        Fix,
        RuleOutput,
    },
    syntax,
};

struct Group<'a> {
    scope: SyntaxNode,
    binding: Binding,
    accesses: Vec<&'a Access>,
    imports: Vec<usize>,
}

struct Batch<'a> {
    groups: Vec<Group<'a>>,
}

struct Validation<'a> {
    access: &'a Access,
    binding: Binding,
    target: SyntaxNode,
    reuse: bool,
    replacement: Option<DefId>,
    outer_reuse: bool,
}

struct Candidate<'a> {
    edits: Vec<Edit>,
    retained: Vec<TextRange>,
    validations: Vec<Validation<'a>>,
    required_removals: Vec<TextRange>,
    imports: Vec<usize>,
    introduced: Vec<(Binding, SyntaxNode)>,
}

#[derive(Clone, Copy, Default)]
struct Dependency {
    route: bool,
    reuse: bool,
}

pub(in super::super) struct Checks<'a> {
    pub(in super::super) name: &'a dyn Fn(NameRequest<'_>) -> bool,
    pub(in super::super) remove: &'a dyn Fn(RemovalRequest<'_>) -> bool,
    pub(in super::super) reference: &'a dyn Fn(TextRange) -> bool,
    pub(in super::super) depends: &'a dyn Fn(RemovalRequest<'_>) -> bool,
    pub(in super::super) protected: &'a dyn Fn(&SyntaxNode) -> bool,
}

pub(in super::super) fn plan(
    root: &SyntaxNode,
    source: &str,
    imports: &[Import],
    accesses: &[Access],
    requests: &[&Access],
    checks: &Checks<'_>,
    output: &mut RuleOutput,
) {
    if output.issues.is_empty() {
        return;
    }

    let mut batches = groups(imports, requests, output);
    merge_related(&mut batches, imports);

    // Shared source routes are mandatory dependencies. Outer binding reuse is
    // optional: only a proven provider may replace an independent inner import.
    let mut candidates: Vec<_> = batches
        .iter()
        .map(|batch| candidate(root, source, imports, batch, checks))
        .collect();

    let mut finished: Vec<_> = candidates
        .iter()
        .map(|candidate| finish(root, source, imports, accesses, candidate, checks))
        .collect();

    let mut dependencies: Vec<Vec<_>> = (0..batches.len())
        .map(|left| {
            (0..batches.len())
                .map(|right| {
                    if left < right {
                        dependency(
                            &candidates[left],
                            &candidates[right],
                            finished[left].is_some(),
                            finished[right].is_some(),
                            imports,
                            checks,
                        )
                    } else {
                        Dependency::default()
                    }
                })
                .collect()
        })
        .collect();

    loop {
        let pair = (0..batches.len())
            .find_map(|left| {
                (left + 1..batches.len())
                    .find(|&right| dependencies[left][right].route)
                    .map(|right| (left, right))
            })
            .or_else(|| {
                (0..batches.len()).find_map(|left| {
                    (left + 1..batches.len())
                        .find(|&right| dependencies[left][right].reuse)
                        .map(|right| (left, right))
                })
            });

        let Some((left, right)) = pair else { break };

        let other = batches.remove(right);
        batches[left].groups.extend(other.groups);
        batches[left]
            .groups
            .sort_by_key(|group| group.scope.text_range().start());

        candidates.remove(right);
        finished.remove(right);
        dependencies.remove(right);
        for row in &mut dependencies {
            row.remove(right);
        }

        candidates[left] = candidate(root, source, imports, &batches[left], checks);
        finished[left] = finish(root, source, imports, accesses, &candidates[left], checks);
        for other in 0..batches.len() {
            if other == left {
                continue;
            }

            let (a, b) = (left.min(other), left.max(other));
            dependencies[a][b] = dependency(
                &candidates[a],
                &candidates[b],
                finished[a].is_some(),
                finished[b].is_some(),
                imports,
                checks,
            );
        }
    }

    let mut ready = Vec::new();
    for (batch, edits) in batches.iter().zip(finished) {
        let Some(edits) = edits else {
            continue;
        };

        let Some(issue) = first_issue(batch, imports, output) else {
            continue;
        };

        if !edits.is_empty() {
            ready.push((issue, edits));
        }
    }

    // rustfix treats separate insertions at the same offset as conflicting.
    // Package only proven fixes together, so a failed transaction stays isolated.
    loop {
        let shared_anchor = (0..ready.len()).find_map(|left| {
            (left + 1..ready.len())
                .find(|&right| {
                    ready[left]
                        .1
                        .iter()
                        .any(|a| a.range.is_empty() && ready[right].1.iter().any(|b| a.range == b.range))
                })
                .map(|right| (left, right))
        });

        let Some((left, right)) = shared_anchor else { break };
        let (issue, edits) = ready.remove(right);
        ready[left].0 = ready[left].0.min(issue);
        ready[left].1.extend(edits);
    }

    for (issue, edits) in ready {
        output.issues[issue].fix = Some(Fix {
            edits: normalize(edits),
        });
    }
}

fn groups<'a>(imports: &[Import], requests: &[&'a Access], output: &RuleOutput) -> Vec<Batch<'a>> {
    let mut groups: Vec<Group<'_>> = Vec::new();
    for &access in requests {
        let Some(binding) = Binding::for_api(&access.api) else {
            continue;
        };

        let scope = if access.api.is_direct() {
            original_import(access, imports).map_or(&access.scope, |import| &import.scope)
        } else {
            &access.scope
        };

        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.scope == *scope && group.binding == binding)
        {
            group.accesses.push(access);
        } else {
            groups.push(Group {
                scope: scope.clone(),
                binding,
                accesses: vec![access],
                imports: Vec::new(),
            });
        }
    }

    for (index, import) in imports.iter().enumerate().filter(|(_, import)| !import.allowed) {
        let binding = Binding::for_import(import);
        let mut owned = false;
        for group in &mut groups {
            let related = group
                .accesses
                .iter()
                .any(|access| original_import(access, imports).is_some_and(|original| original.node == import.node));

            if related || (group.scope == import.scope && binding.as_ref() == Some(&group.binding)) {
                group.imports.push(index);
                owned = true;
            }
        }

        if !owned
            && has_import_issue(import, output)
            && let Some(binding) = binding
        {
            groups.push(Group {
                scope: import.scope.clone(),
                binding,
                accesses: Vec::new(),
                imports: vec![index],
            });
        }
    }

    groups.sort_by_key(|group| group.scope.text_range().start());
    groups.into_iter().map(|group| Batch { groups: vec![group] }).collect()
}

fn has_import_issue(import: &Import, output: &RuleOutput) -> bool {
    output
        .issues
        .iter()
        .flat_map(|issue| &issue.diagnostics)
        .any(|diagnostic| diagnostic.range == import.range || diagnostic.range == import.node.text_range())
}

fn merge_related(batches: &mut Vec<Batch<'_>>, imports: &[Import]) {
    loop {
        let pair = (0..batches.len()).find_map(|left| {
            (left + 1..batches.len())
                .find(|&right| {
                    batches[left].groups.iter().any(|a| {
                        batches[right].groups.iter().any(|b| {
                            a.imports.iter().any(|a| {
                                b.imports
                                    .iter()
                                    .any(|b| declaration(&imports[*a]) == declaration(&imports[*b]))
                            })
                        })
                    })
                })
                .map(|right| (left, right))
        });

        let Some((left, right)) = pair else { break };
        let other = batches.remove(right);
        batches[left].groups.extend(other.groups);
        batches[left]
            .groups
            .sort_by_key(|group| group.scope.text_range().start());
    }
}

fn declaration(import: &Import) -> TextRange {
    import
        .node
        .ancestors()
        .find_map(ast::Use::cast)
        .map_or(import.node.text_range(), |declaration| {
            declaration.syntax().text_range()
        })
}

fn candidate<'a>(
    root: &SyntaxNode,
    source: &str,
    imports: &[Import],
    batch: &Batch<'a>,
    checks: &Checks<'_>,
) -> Candidate<'a> {
    let mut result = Candidate {
        edits: Vec::new(),
        retained: Vec::new(),
        validations: Vec::new(),
        required_removals: Vec::new(),
        imports: Vec::new(),
        introduced: Vec::new(),
    };

    let candidates: Vec<_> = batch
        .groups
        .iter()
        .flat_map(|group| &group.accesses)
        .filter(|access| !(checks.protected)(&access.node) && !syntax::has_comments(root, access.range))
        .map(|access| access.range)
        .collect();

    let mut planned: Vec<(Binding, SyntaxNode)> = Vec::new();
    for group in &batch.groups {
        result.imports.extend(&group.imports);
        let replacement = group
            .accesses
            .iter()
            .find(|access| access.api.replaces_type())
            .map(|access| access.api.definition)
            .or_else(|| {
                group
                    .imports
                    .iter()
                    .filter_map(|&index| imports[index].api.as_ref())
                    .find(|api| api.replaces_type())
                    .map(|api| api.definition)
            });

        let local_binding = group
            .accesses
            .iter()
            .any(|access| original_import(access, imports).is_some_and(|import| import.scope == group.scope));

        let mut addition = None;
        for &access in &group.accesses {
            if (checks.protected)(&access.node) || syntax::has_comments(root, access.range) {
                continue;
            }

            let scopes = syntax::visible_scopes(&access.node);
            let imported = existing_import(access, imports, &group.binding);
            let previous = planned
                .iter()
                .find(|(binding, scope)| !local_binding && *binding == group.binding && scopes.contains(scope));

            let reuse = group.binding.external || imported.is_some() || previous.is_some() || addition.is_some();
            let target = previous.map_or_else(
                || imported.map_or(&group.scope, |import| &import.scope),
                |(_, scope)| scope,
            );

            let conflict = planned.iter().any(|(binding, scope)| {
                binding.same_name(&group.binding) && *binding != group.binding && scopes.contains(scope)
            });

            if conflict
                || !name_available(
                    access,
                    &group.binding,
                    target,
                    reuse,
                    replacement,
                    &candidates,
                    checks.name,
                )
            {
                if root_available(access, &group.scope, checks.name)
                    && let Some(path) = access.api.full_path()
                    && path != access.spelling
                {
                    result.edits.push(Edit {
                        range: access.range,
                        replacement: path,
                    });
                }

                continue;
            }

            if !reuse {
                if access.conditional || edits::insertion(&group.scope, source).is_none() {
                    continue;
                }

                let Some((path, retained)) = import_route(access, imports, source, &group.scope, checks.name) else {
                    continue;
                };

                let leaf = path.rsplit("::").next().unwrap_or("").trim().trim_start_matches("r#");
                let statement = if leaf == group.binding.name {
                    format!("use {path};")
                } else {
                    format!("use {path} as {};", group.binding.name)
                };

                addition = Some(statement);
                result.retained.extend(retained);
            }

            if let Some(definition) = replacement {
                let displaced = scopes
                    .iter()
                    .position(|scope| scope == target)
                    .map_or(&[][..], |end| &scopes[..=end]);

                result.required_removals.extend(
                    imports
                        .iter()
                        .filter(|import| {
                            import.definition == definition
                                && import.name == group.binding.name
                                && displaced.contains(&import.scope)
                        })
                        .map(|import| import.node.text_range()),
                );
            }

            result.validations.push(Validation {
                access,
                binding: group.binding.clone(),
                target: target.clone(),
                reuse,
                replacement,
                outer_reuse: !local_binding,
            });

            result.edits.push(Edit {
                range: access.range,
                replacement: access.api.canonical(),
            });
        }

        if let Some(statement) = addition
            && let Some(insertion) = edits::insertion(&group.scope, source)
        {
            result.edits.push(insertion.edit([statement.as_str()]));
            planned.push((group.binding.clone(), group.scope.clone()));
        }
    }

    result.introduced = planned;
    result.imports.sort_unstable();
    result.imports.dedup();
    result
}

fn import_route(
    access: &Access,
    imports: &[Import],
    source: &str,
    target: &SyntaxNode,
    safe: &dyn Fn(NameRequest<'_>) -> bool,
) -> Option<(String, Vec<TextRange>)> {
    match &access.api.form {
        Form::Direct => {
            let route = if let Some(import) = original_import(access, imports) {
                rewrite_imports::route(import)?
            } else {
                let path = source[access.range].trim();
                if access.spelling.rsplit("::").next() != Some(access.api.name.as_str()) || path.contains(['<', '>']) {
                    return None;
                }

                (path.to_owned(), vec![access.range])
            };

            (route
                .0
                .rsplit("::")
                .next()
                .map(|name| name.trim().trim_start_matches("r#"))
                == Some(access.api.name.as_str()))
            .then_some(route)
        },
        Form::Qualified { qualifier, .. } => {
            root_available(access, target, safe).then(|| (qualifier.source.to_owned(), Vec::new()))
        },
        Form::Replace { target: Some(import) } => {
            root_available(access, target, safe).then(|| (import.source.to_owned(), Vec::new()))
        },
        Form::Replace { target: None } => None,
    }
}

fn finish(
    root: &SyntaxNode,
    source: &str,
    imports: &[Import],
    accesses: &[Access],
    candidate: &Candidate<'_>,
    checks: &Checks<'_>,
) -> Option<Vec<Edit>> {
    let ranges: Vec<_> = candidate
        .edits
        .iter()
        .filter(|edit| !edit.range.is_empty())
        .map(|edit| edit.range)
        .collect();

    let cleanup = rewrite_imports::plan(
        root,
        source,
        candidate.imports.iter().map(|&index| &imports[index]),
        accesses,
        &ranges,
        &candidate.retained,
        checks,
    );

    if candidate
        .required_removals
        .iter()
        .any(|range| !cleanup.removed.contains(range))
        || candidate.validations.iter().any(|validation| {
            !name_available(
                validation.access,
                &validation.binding,
                &validation.target,
                validation.reuse,
                validation.replacement,
                &ranges,
                checks.name,
            )
        })
    {
        return None;
    }

    let mut edits = candidate.edits.clone();
    edits.extend(cleanup.edits);
    Some(edits)
}

fn dependency(
    left: &Candidate<'_>,
    right: &Candidate<'_>,
    left_ready: bool,
    right_ready: bool,
    imports: &[Import],
    checks: &Checks<'_>,
) -> Dependency {
    Dependency {
        route: route_dependency(left, right, imports, checks) || route_dependency(right, left, imports, checks),
        reuse: (right_ready && binding_dependency(left, right)) || (left_ready && binding_dependency(right, left)),
    }
}

fn binding_dependency(consumer: &Candidate<'_>, provider: &Candidate<'_>) -> bool {
    consumer.validations.iter().any(|validation| {
        validation.outer_reuse
            && !validation.reuse
            && provider.introduced.iter().any(|(binding, scope)| {
                *binding == validation.binding && syntax::visible_scopes(&validation.access.node).contains(scope)
            })
    })
}

fn route_dependency(
    consumer: &Candidate<'_>,
    provider: &Candidate<'_>,
    imports: &[Import],
    checks: &Checks<'_>,
) -> bool {
    if consumer.retained.is_empty() || consumer.edits.is_empty() || provider.edits.is_empty() {
        return false;
    }

    provider.imports.iter().any(|&index| {
        let import = &imports[index];
        (checks.depends)(RemovalRequest {
            name: &import.name,
            definition: import.definition,
            namespace: import.namespace,
            range: import.node.text_range(),
            scope: import.scope.text_range(),
            origin: import.origin,
            replacements: &[],
            retained: &consumer.retained,
        })
    })
}

fn first_issue(batch: &Batch<'_>, imports: &[Import], output: &RuleOutput) -> Option<usize> {
    output.issues.iter().position(|issue| {
        issue.diagnostics.iter().any(|diagnostic| {
            batch.groups.iter().any(|group| {
                group.accesses.iter().any(|access| diagnostic.range == access.range)
                    || group.imports.iter().any(|&index| {
                        diagnostic.range == imports[index].range || diagnostic.range == imports[index].node.text_range()
                    })
            })
        })
    })
}

fn normalize(mut edits: Vec<Edit>) -> Vec<Edit> {
    edits.sort_by_key(|edit| (edit.range.start(), edit.range.end()));
    let mut merged: Vec<Edit> = Vec::new();
    for edit in edits {
        if let Some(previous) = merged.last_mut()
            && previous.range.start() == edit.range.start()
            && previous.range.is_empty()
        {
            previous.range = edit.range;
            previous.replacement.push_str(&edit.replacement);
        } else if !merged.last().is_some_and(|previous| previous == &edit) {
            merged.push(edit);
        }
    }

    merged
}

fn name_available(
    access: &Access,
    binding: &Binding,
    target: &SyntaxNode,
    reuse: bool,
    replacement: Option<DefId>,
    ranges: &[TextRange],
    safe: &dyn Fn(NameRequest<'_>) -> bool,
) -> bool {
    safe(NameRequest {
        fact: access.fact,
        at: access.range,
        name: &binding.name,
        definition: binding.definition,
        namespace: binding.namespace,
        target: target.text_range(),
        module_target: target.kind() != SyntaxKind::STMT_LIST,
        reuse,
        external: binding.external,
        replacement: replacement.map(|definition| ReplacedName { definition, ranges }),
    })
}

fn original_import<'a>(access: &Access, imports: &'a [Import]) -> Option<&'a Import> {
    let scopes = syntax::visible_scopes(&access.node);
    let (definition, namespace, name) = match &access.api.form {
        Form::Qualified { qualifier, .. } if access.spelling.contains("::") => (
            qualifier.definition,
            Namespace::TypeNS,
            access.spelling.split("::").next().unwrap_or(""),
        ),
        _ => (access.api.definition, access.api.namespace, access.spelling.as_str()),
    };

    imports
        .iter()
        .filter(|import| {
            import.definition == definition
                && import.namespace == namespace
                && import.name == name
                && scopes.contains(&import.scope)
                && !import.conditional
        })
        .min_by_key(|import| scopes.iter().position(|scope| *scope == import.scope))
}
