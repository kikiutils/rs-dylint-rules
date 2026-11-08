use pathkit::Path;
use rustc_attr_ir::find_attr;
use rustc_hir as hir;
use rustc_lint::{
    LateContext,
    LintContext,
};
use rustc_span::def_id::CRATE_DEF_INDEX;

use super::{
    Name,
    NameRequest,
    Names,
    Reference,
    ancestry,
    module,
};
use crate::workspace::Workspace;

impl Names {
    pub(crate) fn safe(
        &self,
        cx: &LateContext<'_>,
        workspace: &Workspace,
        path: &Path,
        id: hir::HirId,
        request: &NameRequest<'_>,
    ) -> bool {
        let own_module = module(cx, id);
        let parents = ancestry(cx, id);
        let target = if request.module_target {
            let (module, declaration) = cx.tcx.hir_get_module(cx.tcx.parent_module(id));
            let body = workspace.locate_known(cx, module.spans.inner_span);
            let declaration = workspace.locate_known(cx, declaration);
            body.filter(|(file, range)| file == &path && request.target.contains_range(*range))
                .filter(|(file, _)| {
                    own_module == hir::CRATE_HIR_ID
                        || declaration.is_some_and(|(declaration_file, range)| {
                            // External module bodies and their declarations are
                            // in different files; inline bodies are inside items.
                            declaration_file != *file || range.contains_range(request.target)
                        })
                })
                .map(|_| own_module)
        } else {
            cx.tcx
                .hir_parent_iter(id)
                .find_map(|(parent, node)| {
                    let hir::Node::Block(block) = node else { return None };
                    workspace.locate_known(cx, block.span).and_then(|(file, range)| {
                        // HIR can include a block prefix such as `unsafe`,
                        // while the CST statement list begins at its brace.
                        (file == path && range.end() == request.target.end() && range.contains_range(request.target))
                            .then_some(parent)
                    })
                })
                .filter(|&target| module(cx, target) == own_module)
        };

        let Some(target) = target else { return false };

        // A macro input can be expanded in several semantic environments.
        // Only disagreeing resolutions of the same original segment are unsafe.
        let mut original = Vec::new();
        for reference in self.references(cx) {
            if reference.span.from_expansion() {
                continue;
            }

            let Some((file, range)) = workspace.locate_known(cx, reference.span) else {
                continue;
            };

            if file != path || !request.at.contains_range(range) {
                continue;
            }

            let name = &reference.name;
            if original.iter().any(|&(seen, namespace, definition)| {
                seen == range && namespace == name.namespace && definition != name.definition
            }) {
                return false;
            }

            original.push((range, name.namespace, name.definition));
        }

        let textual = |binding: &Name| {
            binding
                .definition
                .and_then(rustc_span::def_id::DefId::as_local)
                .is_some_and(|definition| {
                    matches!(cx.tcx.hir_node(cx.tcx.local_def_id_to_hir_id(definition)),
                    hir::Node::Item(item) if matches!(item.kind,
                        hir::ItemKind::Macro(_, definition, _) if definition.macro_rules))
                })
        };

        // macro_rules textual scopes cross module boundaries and take
        // precedence over a newly inserted macro import in the child module.
        let visible =
            |binding: &Name| (binding.module == own_module || textual(binding)) && parents.contains(&binding.scope);

        let matching = |binding: &Name| binding.name == request.name && binding.namespace == request.namespace;
        let same = |binding: &Name| request.definition.is_some() && binding.definition == request.definition;

        // Only scopes between the insertion target and the access can shadow
        // the new import. An outer binding is deliberately shadowed by it.
        let target_distance = parents.iter().position(|scope| *scope == target).unwrap_or(usize::MAX);
        let explicit_distance = self
            .bindings
            .iter()
            .filter(|binding| !binding.glob && matching(binding) && visible(binding))
            .filter_map(|binding| parents.iter().position(|scope| *scope == binding.scope))
            .min();

        for binding in self
            .bindings
            .iter()
            .filter(|binding| matching(binding) && visible(binding))
        {
            let distance = parents
                .iter()
                .position(|scope| *scope == binding.scope)
                .unwrap_or(usize::MAX);

            if distance > target_distance && !request.reuse && !request.external && !textual(binding) {
                continue;
            }

            if binding.glob && explicit_distance.is_some_and(|explicit| explicit <= distance) {
                continue;
            }

            if (request.reuse || request.external)
                && !textual(binding)
                && explicit_distance.is_some_and(|explicit| explicit < distance)
            {
                continue;
            }

            // A glob binding is not a duplicate declaration. References below
            // decide whether overriding it would change existing code.
            if binding.glob && !request.reuse && !request.external {
                continue;
            }

            if !same(binding) {
                return false;
            }
        }

        if request.external {
            return self.external_available(cx, id, target, request);
        }

        if request.reuse {
            return self
                .bindings
                .iter()
                .any(|binding| matching(binding) && visible(binding) && same(binding))
                || self.references(cx).iter().any(|reference| {
                    reference.lexical
                        && matching(&reference.name)
                        && same(&reference.name)
                        && workspace
                            .locate_known(cx, reference.span)
                            .is_some_and(|(file, range)| file == path && request.at.contains_range(range))
                });
        }

        // Module imports also affect children which glob-import that module.
        // Follow the actual compiler module edges, including external files.
        let mut affected = vec![own_module];
        if request.module_target {
            let mut cursor = 0;
            while cursor < affected.len() {
                for &(source, consumer) in &self.globs {
                    if source == affected[cursor] && !affected.contains(&consumer) {
                        affected.push(consumer);
                    }
                }

                cursor += 1;
            }
        }

        !self.references(cx).iter().any(|reference| {
            let name = &reference.name;
            if !matching(name) || same(name) {
                return false;
            }

            if workspace.locate_known(cx, reference.span).is_some_and(|(file, range)| {
                file == path && request.covered.iter().any(|covered| covered.contains_range(range))
            }) {
                return false;
            }

            // Qualified paths can reach a module's glob-provided name
            // without a lexical reference in that module (self::io,
            // crate::io, or a path from another module/file).
            if !reference.lexical {
                if !request.module_target {
                    return false;
                }

                let Some(owner) = reference
                    .via
                    .and_then(rustc_span::def_id::DefId::as_local)
                    .map(|definition| cx.tcx.local_def_id_to_hir_id(definition))
                    .filter(|owner| affected.contains(owner))
                else {
                    return false;
                };

                return !self.bindings.iter().any(|binding| {
                    !binding.glob
                        && binding.scope == owner
                        && matching(binding)
                        && binding.definition == name.definition
                });
            }

            if !affected.contains(&name.module) {
                return false;
            }

            if name.module == own_module && !ancestry(cx, name.scope).contains(&target) {
                return false;
            }

            !self.lexical_binding(cx, reference).is_some_and(|binding| {
                !binding.glob
                    && binding.scope != target
                    && (name.module != own_module || ancestry(cx, binding.scope).contains(&target))
            })
        })
    }

    fn external_available(
        &self,
        cx: &LateContext<'_>,
        id: hir::HirId,
        target: hir::HirId,
        request: &NameRequest<'_>,
    ) -> bool {
        let own_module = module(cx, id);
        let parents = ancestry(cx, id);
        let implicit = matches!(request.name, "std" | "core" | "alloc")
            && request.definition.is_some_and(|definition| {
                definition.index == CRATE_DEF_INDEX && cx.tcx.crate_name(definition.krate).as_str() == request.name
            });

        let target_parents = ancestry(cx, target);
        let bound = self.bindings.iter().any(|binding| {
            binding.name == request.name
                && binding.namespace == request.namespace
                && request.definition.is_some()
                && binding.definition == request.definition
                && binding.module == own_module
                && parents.contains(&binding.scope)
                && target_parents.contains(&binding.scope)
        });

        if bound {
            // A local alias may expose a renamed direct dependency under the
            // policy's canonical root. Match its compiler artifact against all
            // direct extern names, never against transitive crate metadata.
            return implicit
                || request.definition.is_some_and(|definition| {
                    cx.sess()
                        .opts
                        .externs
                        .iter()
                        .any(|(name, _)| external_crate(cx, name, definition))
                });
        }

        if !implicit
            && !request.definition.map_or_else(
                || {
                    cx.sess()
                        .opts
                        .externs
                        .get(request.name)
                        .is_some_and(|entry| entry.add_prelude)
                },
                |definition| external_crate(cx, request.name, definition),
            )
        {
            return false;
        }

        // This attribute is inherited by child modules. Use compiler attributes
        // so cfg_attr has already been evaluated; absolute extern paths do not
        // prove that the same name resolves without the extern prelude.
        if ancestry(cx, own_module)
            .iter()
            .any(|&parent| find_attr!(cx.tcx.hir_attrs(parent), NoImplicitPrelude))
        {
            return false;
        }

        if self
            .externs
            .iter()
            .any(|(name, definition)| name == request.name && Some(*definition) == request.definition)
        {
            return true;
        }

        // Rust injects std/core into the extern prelude without --extern.
        // Unlike alloc, these roots need no explicit extern-crate statement.
        if implicit {
            let attrs = cx.tcx.hir_attrs(hir::CRATE_HIR_ID);
            let no_core = find_attr!(attrs, NoCore);
            let no_std = find_attr!(attrs, NoStd);
            if !no_core && (request.name == "core" || (request.name == "std" && !no_std)) {
                return true;
            }
        }

        request.definition.map_or_else(
            || {
                cx.sess()
                    .opts
                    .externs
                    .get(request.name)
                    .is_some_and(|entry| entry.add_prelude)
            },
            |definition| external_crate(cx, request.name, definition),
        )
    }

    fn lexical_binding(&self, cx: &LateContext<'_>, reference: &Reference) -> Option<&Name> {
        let name = &reference.name;
        let parents = ancestry(cx, name.scope);
        self.bindings
            .iter()
            .filter(|binding| {
                binding.name == name.name
                    && binding.namespace == name.namespace
                    && binding.module == name.module
                    && binding.definition == name.definition
            })
            .filter_map(|binding| {
                parents
                    .iter()
                    .position(|scope| *scope == binding.scope)
                    .map(|distance| (distance, binding))
            })
            .min_by_key(|(distance, _)| *distance)
            .map(|(_, binding)| binding)
    }
}

pub(in super::super) fn external_crate(
    cx: &LateContext<'_>,
    name: &str,
    definition: rustc_span::def_id::DefId,
) -> bool {
    // Match the actual --extern artifact, not all same-named transitive or
    // sysroot crates (libc, for example, can occur in both graphs).
    if definition.index != CRATE_DEF_INDEX {
        return false;
    }

    let Some(entry) = cx.sess().opts.externs.get(name).filter(|entry| entry.add_prelude) else {
        return false;
    };

    if let Some(mut files) = entry.files() {
        let source = cx.tcx.used_crate_source(definition.krate);
        return files.any(|file| {
            source
                .paths()
                .any(|path| path == file.original() || path == file.canonicalized())
        });
    }

    cx.tcx.crate_name(definition.krate).as_str() == name
        && matches!(cx.tcx.extern_crate(definition.krate), Some(entry) if entry.is_direct())
        && cx
            .tcx
            .crates(())
            .iter()
            .filter(|&&krate| {
                cx.tcx.crate_name(krate).as_str() == name
                    && matches!(cx.tcx.extern_crate(krate), Some(entry) if entry.is_direct())
            })
            .count()
            == 1
}
