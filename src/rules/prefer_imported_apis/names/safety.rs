use pathkit::Path;
use rustc_hir as hir;
use rustc_lint::{
    LateContext,
    LintContext,
};
use rustc_span::{
    def_id::CRATE_DEF_INDEX,
    sym,
};

use super::{
    Name,
    NameRequest,
    Names,
    Reference,
    RemovalRequest,
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
        source_imports: &[hir::HirId],
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

        // One input expression can be expanded more than once. A source edit
        // must preserve every resolution, including non-policy occurrences.
        let mut references = self.references(cx).iter().filter_map(|reference| {
            if reference.span.from_expansion() || !reference.lexical {
                return None;
            }

            workspace
                .locate_known(cx, reference.span)
                .and_then(|(file, range)| (file == path && request.at.contains_range(range)).then_some(&reference.name))
        });

        if let Some(first) = references.next()
            && references
                .any(|reference| reference.definition != first.definition || reference.namespace != first.namespace)
        {
            return false;
        }

        if !request.external && !request.reuse && request.module_target && self.glob_sources.contains(&target) {
            return false;
        }

        let reuse = request.reuse || request.external;
        for binding in &self.bindings {
            if binding.name != request.name || binding.namespace != request.namespace || binding.module != own_module {
                continue;
            }

            if (parents.contains(&binding.scope) || binding.scope == target)
                && !(reuse && request.definition.is_some() && binding.definition == request.definition)
                && !request.replacement.as_ref().is_some_and(|replacement| {
                    binding.definition == Some(replacement.definition) && source_imports.contains(&binding.id)
                })
            {
                return false;
            }
        }

        if request.external && !self.external_available(cx, id, request) {
            return false;
        }

        // A newly inserted binding must not capture paths elsewhere in its scope.
        reuse
            || !self
                .references(cx)
                .iter()
                .filter(|reference| reference.lexical)
                .any(|reference| {
                    let name = &reference.name;
                    let replaced = request.replacement.as_ref().is_some_and(|replacement| {
                        name.definition == Some(replacement.definition)
                            && workspace.locate_known(cx, reference.span).is_some_and(|(file, range)| {
                                file == path && replacement.ranges.iter().any(|replaced| replaced.contains_range(range))
                            })
                    });

                    name.name == request.name
                        && name.namespace == request.namespace
                        && name.module == own_module
                        && (request.definition.is_none() || name.definition != request.definition)
                        && !replaced
                        && ancestry(cx, name.scope).contains(&target)
                        && !self.lexical_binding(cx, reference).is_some_and(|binding| {
                            binding.scope != target && ancestry(cx, binding.scope).contains(&target)
                        })
                })
    }

    fn external_available(&self, cx: &LateContext<'_>, id: hir::HirId, request: &NameRequest<'_>) -> bool {
        let own_module = module(cx, id);
        let parents = ancestry(cx, id);
        if self.bindings.iter().any(|binding| {
            binding.name == request.name
                && binding.namespace == request.namespace
                && request.definition.is_some()
                && binding.definition == request.definition
                && binding.module == own_module
                && parents.contains(&binding.scope)
        }) {
            return true;
        }

        // This attribute is inherited by child modules. Use compiler attributes
        // so cfg_attr has already been evaluated; absolute extern paths do not
        // prove that the same name resolves without the extern prelude.
        if ancestry(cx, own_module).iter().any(|&parent| {
            cx.tcx
                .hir_attrs(parent)
                .iter()
                .any(|attribute| attribute.has_name(sym::no_implicit_prelude))
        }) {
            return false;
        }

        if self
            .externs
            .iter()
            .any(|(name, definition)| name == request.name && Some(*definition) == request.definition)
        {
            return true;
        }

        // An already resolved root occurrence proves which version this extern
        // name denotes, even when several versions are present in the graph.
        if self
            .references(cx)
            .iter()
            .filter(|reference| reference.lexical)
            .any(|reference| {
                let reference = &reference.name;
                reference.name == request.name
                    && reference.namespace == request.namespace
                    && request.definition.is_some()
                    && reference.definition == request.definition
                    && reference.module == own_module
                    && (reference.scope == id || super::lexical_scope(cx, reference.scope) == own_module)
            })
        {
            return true;
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

    pub(crate) fn removable(
        &self,
        cx: &LateContext<'_>,
        workspace: &Workspace,
        path: &Path,
        request: &RemovalRequest<'_>,
    ) -> bool {
        let own_module = module(cx, request.origin);
        if self.glob_sources.contains(&own_module) {
            return false;
        }

        self.references(cx).iter().all(|reference| {
            if !self.references_import(cx, reference, request) {
                return true;
            }

            workspace.locate_known(cx, reference.span).is_some_and(|(file, range)| {
                file == path && (request.range.contains_range(range) || request.covered(range))
            })
        })
    }

    pub(crate) fn depends(
        &self,
        cx: &LateContext<'_>,
        workspace: &Workspace,
        path: &Path,
        request: &RemovalRequest<'_>,
    ) -> bool {
        self.references(cx).iter().any(|reference| {
            self.references_import(cx, reference, request)
                && workspace.locate_known(cx, reference.span).is_some_and(|(file, range)| {
                    file == path
                        && request.scope.contains_range(range)
                        && request.retained.iter().any(|retained| retained.contains_range(range))
                })
        })
    }

    pub(crate) fn reference_ranges(
        &self,
        cx: &LateContext<'_>,
        workspace: &Workspace,
        path: &Path,
    ) -> impl Iterator<Item = ra_ap_syntax::TextRange> {
        self.references(cx)
            .iter()
            .filter(|reference| !reference.span.from_expansion())
            .filter_map(move |reference| workspace.locate_known(cx, reference.span))
            .filter_map(move |(file, range)| (file == path).then_some(range))
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

    fn references_import(&self, cx: &LateContext<'_>, reference: &Reference, request: &RemovalRequest<'_>) -> bool {
        let name = &reference.name;
        if name.name != request.name
            || name.definition != Some(request.definition)
            || name.namespace != request.namespace
        {
            return false;
        }

        let own_module = module(cx, request.origin);
        if reference.lexical {
            name.module == own_module
                && self
                    .lexical_binding(cx, reference)
                    .is_some_and(|binding| binding.id == request.origin)
        } else {
            super::lexical_scope(cx, request.origin) == own_module
                && reference.via == Some(own_module.owner.def_id.to_def_id())
        }
    }
}

pub(in super::super) fn external_crate(
    cx: &LateContext<'_>,
    name: &str,
    definition: rustc_span::def_id::DefId,
) -> bool {
    // Match the actual --extern artifact, not all same-named transitive or
    // sysroot crates (libc, for example, can occur in both graphs).
    if definition.index != CRATE_DEF_INDEX || cx.tcx.crate_name(definition.krate).as_str() != name {
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

    matches!(cx.tcx.extern_crate(definition.krate), Some(entry) if entry.is_direct())
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
