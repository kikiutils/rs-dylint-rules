use super::{
    Family,
    Members,
};

pub(super) const FAMILIES: &[Family] = &[Family::qualified(
    "parking_lot",
    "parking_lot",
    "parking_lot",
    Members::Named(&["Mutex", "RwLock"]),
)];
