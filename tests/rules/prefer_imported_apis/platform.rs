use anyhow::Result;

use super::support;

#[test]
fn qualifies_rustix_io_without_claiming_shared_standard_types() -> Result<()> {
    let source = r"use rustix::io::{Errno, Result as RawResult, write as write_fd, IoSlice};
pub fn errno() -> Errno { Errno::INTR }
pub fn write(fd: impl std::os::fd::AsFd, bytes: &[u8]) -> RawResult<usize> { write_fd(fd, bytes) }
pub fn shared(_: IoSlice<'_>, _: std::io::IoSliceMut<'_>) {}
";

    let (_, fixed) = support::fixed(source, "rustix={version=\"=1.1.6\",features=[\"std\"]}")?;
    assert_eq!(
        fixed,
        source
            .replace(
                "use rustix::io::{Errno, Result as RawResult, write as write_fd, IoSlice};",
                "use rustix::io::{ IoSlice};"
            )
            .replace(
                "-> Errno { Errno::INTR",
                "-> rustix::io::Errno { rustix::io::Errno::INTR"
            )
            .replace("RawResult", "rustix::io::Result")
            .replace("write_fd(", "rustix::io::write(")
    );

    Ok(())
}

#[test]
fn qualifies_tokio_io_but_preserves_extension_traits_and_standard_errors() -> Result<()> {
    let source = r#"use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, duplex, Error, ErrorKind, Result};
pub async fn transfer() -> Result<Vec<u8>> {
    let (reader, mut writer) = duplex(32);
    writer.write_all(b"example").await?;
    let mut reader = BufReader::new(reader);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    let _ = Error::new(ErrorKind::Other, "example");
    Ok(bytes)
}
"#;

    let (_, fixed) = support::fixed(source, "tokio={version=\"=1.54.1\",features=[\"io-util\"]}")?;
    assert_eq!(
        fixed,
        source
            .replace(
                "use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, duplex, Error, ErrorKind, Result};",
                "use std::io;\nuse tokio::io::{AsyncReadExt, AsyncWriteExt};"
            )
            .replace("-> Result", "-> io::Result")
            .replace("= duplex(", "= tokio::io::duplex(")
            .replace("= BufReader::", "= tokio::io::BufReader::")
            .replace("Error::new(ErrorKind::Other", "io::Error::new(io::ErrorKind::Other")
    );

    Ok(())
}

#[test]
fn qualifies_libc_owned_functions_and_constants_but_not_core_ffi_reexports() -> Result<()> {
    let source = r"use libc::{fork, syscall, AT_EMPTY_PATH, c_int, c_void};
pub unsafe fn child() -> c_int { unsafe { fork() } }
pub fn flags() -> c_int { AT_EMPTY_PATH }
pub fn shared(_: *mut c_void, _: std::ffi::c_int) { let _ = syscall as *const (); }
";

    let (_, fixed) = support::fixed(source, "libc=\"=0.2.191\"")?;
    assert_eq!(
        fixed,
        source
            .replace(
                "use libc::{fork, syscall, AT_EMPTY_PATH, c_int, c_void};",
                "use libc::{ c_int, c_void};"
            )
            .replace("{ fork() }", "{ libc::fork() }")
            .replace("{ AT_EMPTY_PATH }", "{ libc::AT_EMPTY_PATH }")
            .replace("= syscall as", "= libc::syscall as")
    );

    Ok(())
}

#[test]
fn qualified_grouped_imports_do_not_capture_new_namespace_bindings() -> Result<()> {
    let source = r#"use std::io::Result as IoResult;
use rustix::{io::{Errno, write as write_fd, IoSlice}};
pub fn result() -> IoResult<()> { Ok(()) }
pub fn errno() -> Errno { Errno::INTR }
pub fn write(fd: impl std::os::fd::AsFd) { let _ = write_fd(fd, b"x"); }
pub fn shared(_: IoSlice<'_>) {}
"#;

    let (_, fixed) = support::fixed(source, "rustix=\"=1.1.6\"")?;
    assert_eq!(
        fixed,
        source
            .replace("use std::io::Result as IoResult;", "use std::io;\n")
            .replace(
                "use rustix::{io::{Errno, write as write_fd, IoSlice}};",
                "use rustix::{io::{ IoSlice}};"
            )
            .replace("IoResult", "io::Result")
            .replace(
                "-> Errno { Errno::INTR",
                "-> rustix::io::Errno { rustix::io::Errno::INTR"
            )
            .replace("write_fd(", "rustix::io::write(")
    );

    Ok(())
}
