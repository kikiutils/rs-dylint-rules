use anyhow::Result;

use super::support;

#[test]
fn qualifies_standard_functions_and_core_reexports() -> Result<()> {
    let source = r"pub fn inspect() -> (u32, usize, bool) {
    let mut value = 3;
    let old = std::mem::replace(&mut value, 5);
    let pointer = core::ptr::null::<u8>();
    let _ = std::env::current_dir();
    let _ = std::thread::available_parallelism();
    let _ = std::io::copy(&mut std::io::empty(), &mut std::io::sink());
    (std::process::id(), old, pointer.is_null())
}
";

    let (_, fixed) = support::fixed(source, "")?;
    assert_eq!(
        fixed,
        format!(
            "use std::mem;\nuse std::ptr;\nuse std::env;\nuse std::thread;\nuse std::io;\nuse std::process;\n{}",
            source
                .replace("std::mem::", "mem::")
                .replace("core::ptr::", "ptr::")
                .replace("std::env::", "env::")
                .replace("std::thread::", "thread::")
                .replace("std::io::", "io::")
                .replace("std::process::", "process::")
        )
    );

    Ok(())
}

#[test]
fn qualifies_error_types_without_restricting_other_types_or_prelude_functions() -> Result<()> {
    let source = r#"use std::process as proc;
pub fn result() -> std::io::Result<Option<()>> { Ok(Some(())) }
pub fn format_result() -> std::fmt::Result { Ok(()) }
pub fn format_error() -> std::fmt::Error { std::fmt::Error }
pub fn kind() -> std::io::ErrorKind { std::io::ErrorKind::Other }
pub fn command() -> proc::Command { proc::Command::new("example") }
pub fn prelude() {
    drop(String::new());
    let _ = size_of::<u8>();
    let _ = std::mem::align_of::<u8>();
}
"#;

    let (_, fixed) = support::fixed(source, "")?;
    assert_eq!(
        fixed,
        format!(
            "use std::io;\nuse std::fmt;\n{}",
            source.replace("std::io::", "io::").replace("std::fmt::", "fmt::")
        )
    );

    Ok(())
}

#[test]
fn accepts_canonical_standard_paths_and_unmanaged_module_aliases() -> Result<()> {
    support::accepted(
        r#"use std::{io, process, mem};
use std::process as command_process;
pub fn result() -> io::Result<()> { Ok(()) }
pub fn id() -> u32 { process::id() }
pub fn replace(value: &mut u8) -> u8 { mem::replace(value, 1) }
pub fn command() -> command_process::Command { command_process::Command::new("example") }
"#,
        "",
    )
}

#[test]
fn qualifies_bare_functions_and_type_aliases_without_changing_imports() -> Result<()> {
    let source = r#"use std::{process::id, io::Error as IoError};
pub fn current() -> (fn() -> u32, u32) { (id, id()) }
pub fn error() -> IoError { IoError::other("example") }
"#;

    let (directory, fixed) = support::rewritten(source, "")?;
    assert_eq!(
        fixed,
        format!(
            "use std::process;\nuse std::io;\n{}",
            source
                .replace("{ (id, id()) }", "{ (process::id, process::id()) }")
                .replace("-> IoError", "-> io::Error")
                .replace("IoError::other", "io::Error::other")
        )
    );

    let output = crate::support::run(&directory, false, support::LINT)?;
    assert!(output.status.success(), "{output:?}");
    let diagnostics = crate::support::diagnostics(&output, support::LINT);
    assert!(diagnostics.is_empty(), "{output:?}");

    Ok(())
}
