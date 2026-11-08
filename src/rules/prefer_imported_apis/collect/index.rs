use kikiutils::types::fx_collections::FxHashMap;
use rustc_hir::HirId;
use rustc_span::def_id::DefId;

use super::super::model::{
    Event,
    FactKey,
};
use crate::facts::Fact;

pub(super) struct FactIndex {
    origins: FxHashMap<FactKey, Vec<HirId>>,
    imports: FxHashMap<DefId, FxHashMap<String, Vec<HirId>>>,
    import_keys: FxHashMap<HirId, FactKey>,
}

impl FactIndex {
    pub(super) fn new(facts: &[&Fact<Event>]) -> Self {
        let mut index = Self {
            origins: FxHashMap::default(),
            imports: FxHashMap::default(),
            import_keys: FxHashMap::default(),
        };

        for fact in facts {
            let key = FactKey {
                range: fact.range,
                definition: fact.value.definition(),
            };

            let origins = index.origins.entry(key).or_default();
            if !origins.contains(&fact.id) {
                origins.push(fact.id);
            }

            let name = match &fact.value {
                Event::Import { name, .. } | Event::NamespaceImport { name, .. } => name,
                Event::Use { .. } => continue,
            };

            index.import_keys.insert(fact.id, key);
            let imports = index
                .imports
                .entry(key.definition)
                .or_default()
                .entry(name.clone())
                .or_default();

            if !imports.contains(&fact.id) {
                imports.push(fact.id);
            }
        }

        index
    }

    pub(super) fn origins(&self, key: FactKey) -> &[HirId] {
        self.origins.get(&key).map_or(&[], Vec::as_slice)
    }

    pub(super) fn source_imports(&self, definition: DefId, name: &str) -> &[HirId] {
        self.imports
            .get(&definition)
            .and_then(|names| names.get(name))
            .map_or(&[], Vec::as_slice)
    }

    pub(super) fn import_origins(&self, origin: HirId) -> &[HirId] {
        self.import_keys.get(&origin).map_or(&[], |key| self.origins(*key))
    }
}
