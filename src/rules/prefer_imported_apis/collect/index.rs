use kikiutils::types::fx_collections::FxHashMap;
use rustc_hir::HirId;

use super::super::model::{
    Event,
    FactKey,
};
use crate::facts::Fact;

pub(super) struct FactIndex {
    origins: FxHashMap<FactKey, Vec<HirId>>,
}

impl FactIndex {
    pub(super) fn new(facts: &[&Fact<Event>]) -> Self {
        let mut index = Self {
            origins: FxHashMap::default(),
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
        }

        index
    }

    pub(super) fn origins(&self, key: FactKey) -> &[HirId] {
        self.origins.get(&key).map_or(&[], Vec::as_slice)
    }
}
