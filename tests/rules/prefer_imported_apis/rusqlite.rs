use super::support;

const DEPENDENCY: &str = r#"rusqlite={version="=0.41.1",default-features=false,features=["bundled"]}"#;

#[test]
fn qualifies_rusqlite_errors_and_preserves_enum_patterns_and_behavior() -> anyhow::Result<()> {
    let source = r"use rusqlite as sqlite;
pub fn missing() -> sqlite::Error { sqlite::Error::QueryReturnedNoRows }
pub fn index(error: &sqlite::Error) -> usize {
    match error { sqlite::Error::InvalidColumnIndex(index) => *index, _ => 0 }
}
#[test]
fn behavior() {
    assert!(matches!(missing(), rusqlite::Error::QueryReturnedNoRows));
    assert_eq!(index(&rusqlite::Error::InvalidColumnIndex(7)), 7);
}
";

    let (directory, fixed) = support::fixed(source, DEPENDENCY)?;
    assert_eq!(
        fixed,
        source
            .replace(
                "-> sqlite::Error { sqlite::Error::",
                "-> rusqlite::Error { rusqlite::Error::"
            )
            .replace("error: &sqlite::Error", "error: &rusqlite::Error")
            .replace("match error { sqlite::Error::", "match error { rusqlite::Error::")
    );

    support::behavior(&directory)
}

#[test]
fn accepts_canonical_errors_without_restricting_result_or_connection_aliases() -> anyhow::Result<()> {
    support::accepted(
        r"use rusqlite as sqlite;
use rusqlite::Result as SqliteResult;
pub fn check(_: Option<rusqlite::Error>, _: Option<sqlite::Connection>) -> SqliteResult<()> { Ok(()) }
",
        DEPENDENCY,
    )
}

#[test]
fn qualifies_bare_and_aliased_errors_without_removing_imports() -> anyhow::Result<()> {
    for (import, name) in [
        ("use rusqlite::Error;", "Error"),
        ("use rusqlite::Error as SqliteError;", "SqliteError"),
    ] {
        let (_, fixed) = support::rewritten(
            &format!("{import}\npub fn missing() -> {name} {{ {name}::QueryReturnedNoRows }}\n"),
            DEPENDENCY,
        )?;

        assert_eq!(
            fixed,
            format!("{import}\npub fn missing() -> rusqlite::Error {{ rusqlite::Error::QueryReturnedNoRows }}\n")
        );
    }

    Ok(())
}
