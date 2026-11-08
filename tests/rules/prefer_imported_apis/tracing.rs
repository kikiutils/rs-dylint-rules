use anyhow::Result;

use super::support;

#[test]
fn qualifies_tracing_fmt_without_capturing_standard_fmt() -> Result<()> {
    let source = r#"use std::fmt;
use tracing_subscriber::fmt as trace_fmt;
use tracing_subscriber::fmt::{layer as make_layer, Layer as LogLayer};
use tracing_subscriber::fmt::format::{DefaultFields, FormatEvent};
use tracing_subscriber::Registry;
pub struct Value;
impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { formatter.write_str("ok") }
}
pub fn subscribers() {
    let _ = trace_fmt().finish();
    let _: LogLayer<Registry> = make_layer();
    let _ = trace_fmt::format::FmtSpan::NEW;
}
pub fn event<F: FormatEvent<Registry, DefaultFields>>(_: F) {}
#[test]
fn behavior() {
    assert_eq!(format!("{}", Value), "ok");
    subscribers();
}
"#;

    let (directory, fixed) = support::fixed(
        source,
        "tracing-subscriber={version=\"=0.3.23\",default-features=false,features=[\"fmt\"]}",
    )?;

    assert_eq!(
        fixed,
        source
            .replace("trace_fmt()", "tracing_subscriber::fmt()")
            .replace("trace_fmt::", "tracing_subscriber::fmt::")
            .replace("LogLayer<", "tracing_subscriber::fmt::Layer<")
            .replace("= make_layer()", "= tracing_subscriber::fmt::layer()")
    );

    support::behavior(&directory)
}
