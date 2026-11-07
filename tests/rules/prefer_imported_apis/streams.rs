use anyhow::Result;

use super::support;

const DEPENDENCY: &str = "anstream=\"=1.0.0\"";

#[test]
fn qualifies_anstream_standard_stream_functions() -> Result<()> {
    let source = r"use anstream as output;
pub fn streams() {
    let _ = output::stdout();
    let _ = output::stderr();
}
";

    let (_, fixed) = support::fixed(source, DEPENDENCY)?;
    assert_eq!(fixed, source.replace("output::", "anstream::"));
    Ok(())
}

#[test]
fn distinguishes_standard_streams_without_constraining_anstream_types() -> Result<()> {
    support::accepted(
        r"use std::io;
use anstream::Stderr;
pub fn adaptive() -> Stderr { anstream::stderr() }
pub fn standard() -> io::Stderr { io::stderr() }
",
        DEPENDENCY,
    )
}
