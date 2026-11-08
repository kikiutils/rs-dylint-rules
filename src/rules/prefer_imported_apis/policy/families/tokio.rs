use super::{
    Family,
    Members,
};

pub(super) const FAMILIES: &[Family] = &[
    Family::direct("tokio::time", Members::Functions(&[])),
    Family::qualified("tokio::task", "tokio", "tokio", Members::Functions(&[])),
    Family::qualified("tokio::sync::mpsc", "tokio", "tokio", Members::Channel),
    Family::qualified("tokio::sync::oneshot", "tokio", "tokio", Members::Channel),
];
