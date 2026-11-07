use kikiutils::types::fx_collections::FxHashMap;
use rustc_hir::def::{
    DefKind,
    Res,
};
use rustc_lint::LateContext;
use rustc_span::def_id::{
    CRATE_DEF_INDEX,
    DefId,
};

use super::{
    Api,
    Form,
    Qualifier,
    families::{
        self,
        Family,
        Members,
    },
};

#[derive(Default)]
pub(in super::super) struct Catalog {
    apis: FxHashMap<DefId, Api>,
    qualifiers: FxHashMap<DefId, Qualifier>,
}

impl Catalog {
    pub(in super::super) fn build(cx: &LateContext<'_>) -> Self {
        let mut catalog = Self::default();
        for family in families::all() {
            let Some(crate_name) = family.module.split("::").next() else {
                continue;
            };

            for &krate in cx.tcx.crates(()) {
                if cx.tcx.crate_name(krate).as_str() != crate_name {
                    continue;
                }

                let root = DefId {
                    krate,
                    index: CRATE_DEF_INDEX,
                };

                catalog.family(cx, root, family);
            }
        }

        catalog
    }

    pub(in super::super) fn api(&self, definition: DefId) -> Option<&Api> {
        self.apis.get(&definition)
    }

    pub(in super::super) fn qualifier(&self, definition: DefId) -> Option<&Qualifier> {
        self.qualifiers.get(&definition)
    }

    fn family(&mut self, cx: &LateContext<'_>, root: DefId, family: &Family) {
        let Some(module) = resolve(cx, root, family.module) else {
            return;
        };

        let qualifier = match family.qualifier {
            Some((source, name)) => {
                let Some(definition) = resolve(cx, root, source) else {
                    return;
                };

                let qualifier = Qualifier {
                    source,
                    name,
                    definition,
                    root,
                    external: !source.contains("::"),
                };

                self.qualifiers.entry(definition).or_insert_with(|| qualifier.clone());
                Some(qualifier)
            },
            None => None,
        };

        self.members(cx, module, family, qualifier.as_ref(), "");
    }

    fn members(
        &mut self,
        cx: &LateContext<'_>,
        module: DefId,
        family: &Family,
        qualifier: Option<&Qualifier>,
        nested: &str,
    ) {
        for child in cx.tcx.module_children(module) {
            if !child.vis.is_public() {
                continue;
            }

            let Res::Def(kind, definition) = child.res else {
                continue;
            };

            let name = child.ident.name.as_str();
            if matches!(family.members, Members::Channel) && kind == DefKind::Mod && name == "error" {
                self.members(cx, definition, family, qualifier, "error::");
                continue;
            }

            let selected = match family.members {
                Members::Functions(excluded) => kind == DefKind::Fn && !excluded.contains(&name),
                Members::Named(names) => names.contains(&name),
                Members::Channel => matches!(
                    kind,
                    DefKind::Fn | DefKind::Struct | DefKind::Enum | DefKind::Union | DefKind::TyAlias
                ),
            };

            if !selected {
                continue;
            }

            let Some(namespace) = child.res.ns() else { continue };
            let form = qualifier.map_or(Form::Direct, |qualifier| {
                let relative = family
                    .module
                    .strip_prefix(qualifier.source)
                    .unwrap_or("")
                    .trim_start_matches("::");

                let member = if relative.is_empty() {
                    format!("{nested}{name}")
                } else {
                    format!("{relative}::{nested}{name}")
                };

                Form::Qualified {
                    qualifier: qualifier.clone(),
                    member,
                }
            });

            // Earlier public routes win, so a reexport keeps its short canonical
            // spelling rather than being replaced by an implementation route.
            self.apis.entry(definition).or_insert_with(|| Api {
                definition,
                name: name.to_owned(),
                namespace,
                form,
            });
        }
    }
}

fn resolve(cx: &LateContext<'_>, root: DefId, path: &str) -> Option<DefId> {
    let mut definition = root;
    for segment in path.split("::").skip(1) {
        definition = cx.tcx.module_children(definition).iter().find_map(|child| {
            if !child.vis.is_public() || child.ident.name.as_str() != segment {
                return None;
            }

            match child.res {
                Res::Def(DefKind::Mod, definition) => Some(definition),
                _ => None,
            }
        })?;
    }

    Some(definition)
}
