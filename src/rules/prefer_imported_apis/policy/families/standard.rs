use super::{
    Family,
    Members,
};

// Keep the prelude's ordinary spelling independent of namespace policy.
const PRELUDE: &[&str] = &["drop", "size_of", "size_of_val", "align_of", "align_of_val"];

pub(super) const FAMILIES: &[Family] = &[
    Family::qualified("std::process", "std::process", "process", Members::Functions(&[])),
    Family::qualified("std::env", "std::env", "env", Members::Functions(&[])),
    Family::qualified("std::mem", "std::mem", "mem", Members::Functions(PRELUDE)),
    Family::qualified("std::ptr", "std::ptr", "ptr", Members::Functions(&[])),
    Family::qualified("std::thread", "std::thread", "thread", Members::Functions(&[])),
    Family::qualified("std::io", "std::io", "io", Members::Functions(&[])),
    Family::qualified(
        "std::io",
        "std::io",
        "io",
        Members::Named(&["Error", "ErrorKind", "Result"]),
    ),
    Family::qualified("std::fmt", "std::fmt", "fmt", Members::Named(&["Error", "Result"])),
    Family::qualified("std::sync::mpsc", "std::sync::mpsc", "std_mpsc", Members::Channel),
    // std reexports these same definitions. The fallback also works in no_std crates.
    Family::qualified("core::mem", "core::mem", "mem", Members::Functions(PRELUDE)).fallback(),
    Family::qualified("core::ptr", "core::ptr", "ptr", Members::Functions(&[])).fallback(),
    Family::qualified("core::fmt", "core::fmt", "fmt", Members::Named(&["Error", "Result"])).fallback(),
];
