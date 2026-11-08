use ra_ap_syntax::{
    AstNode,
    Edition,
    SourceFile,
    ast,
};
use rustc_hir::{
    self as hir,
    def::{
        DefKind,
        Namespace,
        Res,
    },
};
use rustc_lint::{
    LateContext,
    LintContext,
};
use rustc_span::{
    BytePos,
    Ident,
    Span,
    Symbol,
    def_id::DefId,
    hygiene::MacroKind,
    kw,
};

use super::{
    Name,
    NamePath,
    Names,
    Reference,
    Segment,
    ancestry,
    module,
};

impl Names {
    pub(crate) fn reference(&mut self, cx: &LateContext<'_>, path: &hir::Path<'_>, id: hir::HirId) {
        // Nested import prefixes aren't independent lexical roots.
        if std::iter::once(cx.tcx.hir_node(id))
            .chain(cx.tcx.hir_parent_iter(id).map(|(_, node)| node))
            .any(|node| matches!(node, hir::Node::Item(item) if matches!(item.kind, hir::ItemKind::Use(_))))
        {
            return;
        }

        let mut segments = segments(path.segments);
        if let Some(last) = segments.last_mut() {
            last.resolutions = vec![path.res];
        }

        self.paths.push(NamePath { id, segments });
    }

    pub(crate) fn import_path(&mut self, tree: &hir::UseTree<'_>, id: hir::HirId, parent: &[Segment]) -> Vec<Segment> {
        let mut route = parent.to_vec();
        let mut prefix = segments(tree.prefix.segments);
        let resolutions: Vec<_> = tree
            .prefix
            .res
            .iter()
            .flatten()
            .copied()
            .filter(|resolution| resolution.ns().is_some())
            .collect();

        if !resolutions.is_empty()
            && let Some(last) = prefix.last_mut()
        {
            last.resolutions = resolutions;
        }

        route.extend(prefix);
        self.paths.push(NamePath {
            id,
            segments: route.clone(),
        });

        route
    }

    pub(crate) fn macro_path(
        &mut self,
        cx: &LateContext<'_>,
        span: Span,
        id: hir::HirId,
        definition: DefId,
        kind: MacroKind,
    ) {
        let Ok(source) = cx.sess().source_map().span_to_snippet(span) else {
            return;
        };

        let Ok(edition) = cx.sess().edition().to_string().parse() else {
            return;
        };

        let Some(path) = macro_route(&source, edition, kind) else {
            return;
        };

        let mut segments: Vec<_> = path
            .syntax()
            .descendants()
            .filter_map(ast::PathSegment::cast)
            .map(|segment| {
                let range = segment.syntax().text_range();
                let segment_span = span
                    .with_lo(span.lo() + BytePos(range.start().into()))
                    .with_hi(span.lo() + BytePos(range.end().into()));

                Segment {
                    ident: Ident::new(
                        Symbol::intern(segment.syntax().text().to_string().trim_start_matches("r#")),
                        segment_span,
                    ),
                    resolutions: Vec::new(),
                }
            })
            .collect();

        if path.first_qualifier_or_self().coloncolon_token().is_some() {
            segments.insert(
                0,
                Segment {
                    ident: Ident::new(kw::PathRoot, span.shrink_to_lo()),
                    resolutions: Vec::new(),
                },
            );
        }

        if let Some(last) = segments.last_mut() {
            last.resolutions = vec![Res::Def(cx.tcx.def_kind(definition), definition)];
        }

        self.paths.push(NamePath { id, segments });
    }

    pub(super) fn references(&self, cx: &LateContext<'_>) -> &[Reference] {
        // Imports and declarations may occur after their uses. Resolve missing
        // prefixes only after all compiler bindings have been collected.
        self.references
            .get_or_init(|| self.paths.iter().flat_map(|path| self.resolve_path(cx, path)).collect())
    }

    fn resolve_path(&self, cx: &LateContext<'_>, path: &NamePath) -> Vec<Reference> {
        let own_module = module(cx, path.id);
        let parents = ancestry(cx, path.id);
        let absolute = path
            .segments
            .first()
            .is_some_and(|segment| segment.ident.name == kw::PathRoot);

        let mut via = None;
        let mut references = Vec::new();
        for (index, segment) in path
            .segments
            .iter()
            .filter(|segment| segment.ident.name != kw::PathRoot)
            .enumerate()
        {
            let mut resolutions: Vec<_> = segment
                .resolutions
                .iter()
                .copied()
                .filter(|resolution| resolution.ns().is_some())
                .collect();

            if resolutions.is_empty() {
                let root = index == 0 && !absolute;
                let resolution =
                    self.resolve_prefix(cx, path.id, segment.ident, via, root.then_some(parents.as_slice()));

                resolutions.push(resolution);
            }

            for &resolution in &resolutions {
                if let Some(namespace) = resolution.ns() {
                    references.push(Reference {
                        span: segment.ident.span,
                        name: Name {
                            glob: false,
                            name: segment.ident.name.to_string(),
                            namespace,
                            definition: resolution.opt_def_id(),
                            scope: path.id,
                            module: own_module,
                        },
                        lexical: index == 0 && !absolute,
                        via,
                    });
                }
            }

            via = resolutions.iter().find_map(rustc_hir::def::Res::opt_def_id);
        }

        references
    }

    fn resolve_prefix(
        &self,
        cx: &LateContext<'_>,
        id: hir::HirId,
        ident: Ident,
        via: Option<DefId>,
        parents: Option<&[hir::HirId]>,
    ) -> Res {
        let own_module = module(cx, id);
        let definition = match ident.name {
            kw::Crate => Some(hir::CRATE_HIR_ID.owner.def_id.to_def_id()),
            kw::SelfLower => Some(own_module.owner.def_id.to_def_id()),
            kw::Super => via
                .unwrap_or(own_module.owner.def_id.to_def_id())
                .as_local()
                .map(|definition| cx.tcx.parent_module_from_def_id(definition).to_def_id()),
            _ => None,
        };

        if let Some(definition) = definition {
            return Res::Def(DefKind::Mod, definition);
        }

        let binding = self
            .bindings
            .iter()
            .filter(|binding| binding.name == ident.name.as_str() && binding.namespace == Namespace::TypeNS)
            .filter_map(|binding| {
                let distance = if let Some(parents) = parents {
                    (binding.module == own_module)
                        .then(|| parents.iter().position(|parent| *parent == binding.scope))
                        .flatten()?
                } else {
                    (Some(binding.module.owner.def_id.to_def_id()) == via && binding.scope == binding.module)
                        .then_some(0)?
                };

                Some((distance, binding))
            })
            .min_by_key(|(distance, _)| *distance)
            .map(|(_, binding)| binding);

        if let Some(binding) = binding {
            return binding
                .definition
                .map_or(Res::Err, |definition| Res::Def(cx.tcx.def_kind(definition), definition));
        }

        let Some(via) = via
            .or_else(|| parents.map(|_| own_module.owner.def_id.to_def_id()))
            .filter(|definition| cx.tcx.def_kind(*definition) == DefKind::Mod)
        else {
            return Res::Err;
        };

        let children = if let Some(local) = via.as_local() {
            cx.tcx.module_children_local(local)
        } else {
            cx.tcx.module_children(via)
        };

        children
            .iter()
            .find(|child| child.ident.name == ident.name)
            .map_or(Res::Err, |child| child.res.map_id(|_| unreachable!()))
    }
}

fn macro_route(source: &str, edition: Edition, kind: MacroKind) -> Option<ast::Path> {
    match kind {
        MacroKind::Bang => SourceFile::parse(source, edition)
            .tree()
            .syntax()
            .children()
            .filter_map(ast::MacroCall::cast)
            .find_map(|call| call.path()),
        MacroKind::Attr if source.trim_start().starts_with('#') => {
            // Append an owner so the original attribute parses as an item attribute.
            // Appending leaves every source-relative path offset unchanged.
            SourceFile::parse(&format!("{source}\nstruct __MacroOwner;"), edition)
                .tree()
                .syntax()
                .descendants()
                .filter_map(ast::Attr::cast)
                .filter_map(|attribute| attribute.meta())
                .find_map(|meta| meta.syntax().children().find_map(ast::Path::cast))
        },
        MacroKind::Attr | MacroKind::Derive => ast::Expr::parse(source, edition)
            .syntax_node()
            .descendants()
            .find_map(ast::Path::cast),
    }
}

fn segments(segments: &[hir::PathSegment<'_>]) -> Vec<Segment> {
    segments
        .iter()
        .map(|segment| Segment {
            ident: segment.ident,
            resolutions: vec![segment.res],
        })
        .collect()
}
