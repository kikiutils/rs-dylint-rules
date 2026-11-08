use super::{
    Family,
    Members,
};

pub(super) const FAMILIES: &[Family] = &[
    Family::qualified("rustix::io", "rustix", "rustix", Members::Owned { constants: false }),
    Family::qualified("tokio::io", "tokio", "tokio", Members::Owned { constants: false }),
    Family::qualified("libc", "libc", "libc", Members::Owned { constants: true }),
];
