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
    NameRequest,
    Names,
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
                    workspace
                        .locate_known(cx, block.span)
                        .and_then(|(_, range)| (range == request.target).then_some(parent))
                })
                .filter(|&target| module(cx, target) == own_module)
        };

        let Some(target) = target else { return false };

        // One input expression can be expanded more than once. A source edit
        // must preserve every resolution, including non-policy occurrences.
        let mut references = self.references.iter().filter_map(|(span, reference)| {
            if span.from_expansion() {
                return None;
            }

            workspace
                .locate_known(cx, *span)
                .and_then(|(file, range)| (file == path && request.at.contains_range(range)).then_some(reference))
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
                && !(reuse && binding.definition == Some(request.definition))
            {
                return false;
            }
        }

        if request.external && !self.external_available(cx, id, request) {
            return false;
        }

        // A newly inserted binding must not capture paths elsewhere in its scope.
        reuse
            || !self.references.iter().any(|(_, reference)| {
                reference.name == request.name
                    && reference.namespace == request.namespace
                    && reference.module == own_module
                    && reference.definition != Some(request.definition)
                    && ancestry(cx, reference.scope).contains(&target)
            })
    }

    fn external_available(&self, cx: &LateContext<'_>, id: hir::HirId, request: &NameRequest<'_>) -> bool {
        let own_module = module(cx, id);
        let parents = ancestry(cx, id);
        if self.bindings.iter().any(|binding| {
            binding.name == request.name
                && binding.namespace == request.namespace
                && binding.definition == Some(request.definition)
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
            .any(|(name, definition)| name == request.name && *definition == request.definition)
        {
            return true;
        }

        // An already resolved root occurrence proves which version this extern
        // name denotes, even when several versions are present in the graph.
        if self.references.iter().any(|(_, reference)| {
            reference.name == request.name
                && reference.namespace == request.namespace
                && reference.definition == Some(request.definition)
                && reference.module == own_module
                && reference.scope == id
        }) {
            return true;
        }

        // For a new crate-root spelling, require a direct extern entry and a
        // unique matching crate identity; transitive/renamed crates are not enough.
        request.definition.index == CRATE_DEF_INDEX
            && cx.tcx.crate_name(request.definition.krate).as_str() == request.name
            && cx.sess().opts.externs.get(request.name).is_some()
            && cx
                .tcx
                .crates(())
                .iter()
                .filter(|&&krate| cx.tcx.crate_name(krate).as_str() == request.name)
                .count()
                == 1
    }
}
