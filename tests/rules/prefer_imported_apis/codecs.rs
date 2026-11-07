use super::support;

#[test]
fn qualifies_imported_function_aliases_without_losing_turbofish() -> anyhow::Result<()> {
    let source = r#"use serde_json::from_str as parse_json;
pub fn parse() -> serde_json::Result<u8> { parse_json::<u8>("7") }
"#;

    let (_, fixed) = support::rewritten(source, "serde_json=\"=1.0.154\"")?;
    assert_eq!(fixed, source.replace("parse_json::<u8>", "serde_json::from_str::<u8>"));
    Ok(())
}

#[test]
fn qualifies_json_functions_and_generic_types_without_renaming_trait_imports() -> anyhow::Result<()> {
    let source = r"use serde_json as json;
use serde::Serialize;
#[derive(Serialize)]
struct Record { value: u8 }
pub fn json() -> json::Result<json::Value> {
    let text = json::to_string(&Record { value: 7 })?;
    json::from_str(&text)
}
pub fn map(_: Option<json::Error>) -> json::Map<String, json::Value> { json::Map::new() }
";

    let (_, fixed) = support::fixed(
        source,
        "serde_json=\"=1.0.154\"\nserde={version=\"=1.0.229\",features=[\"derive\"]}",
    )?;

    assert_eq!(fixed, source.replace("json::", "serde_json::"));
    Ok(())
}

#[test]
fn qualifies_toml_functions_values_and_tables() -> anyhow::Result<()> {
    let source = r#"use toml as format;
pub fn parse() -> format::Value {
    let table: format::Table = format::from_str("answer = 42").unwrap();
    let text = format::to_string_pretty(&table).unwrap();
    format::from_str(&text).unwrap()
}
"#;

    let (_, fixed) = support::fixed(source, "toml=\"=1.2.1\"")?;
    assert_eq!(fixed, source.replace("format::", "toml::"));

    Ok(())
}

#[test]
fn qualifies_messagepack_postcard_and_cbor_conversions_and_preserves_roundtrips() -> anyhow::Result<()> {
    let source = r"use rmp_serde as msgpack;
use postcard as binary;
use ciborium as cbor;
pub fn roundtrip() -> (u8, u8, u8) {
    let message = msgpack::to_vec_named(&7u8).unwrap();
    let compact = binary::to_allocvec(&8u8).unwrap();
    let mut buffer = Vec::new();
    cbor::ser::into_writer(&9u8, &mut buffer).unwrap();
    (
        msgpack::from_slice(&message).unwrap(),
        binary::from_bytes(&compact).unwrap(),
        cbor::de::from_reader(&buffer[..]).unwrap(),
    )
}
#[test]
fn behavior() { assert_eq!(roundtrip(), (7, 8, 9)); }
";

    let (directory, fixed) = support::fixed(
        source,
        "rmp-serde=\"=1.3.1\"\npostcard={version=\"=1.1.3\",default-features=false,features=[\"alloc\"]}\nciborium=\"=0.2.2\"",
    )?;

    assert_eq!(
        fixed,
        source
            .replace("msgpack::", "rmp_serde::")
            .replace("binary::", "postcard::")
            .replace("cbor::ser::", "ciborium::")
            .replace("cbor::de::", "ciborium::")
    );

    support::behavior(&directory)
}

#[test]
fn qualifies_hex_functions_but_allows_the_conversion_trait() -> anyhow::Result<()> {
    let source = r#"use hex as encoding;
use hex::ToHex;
pub fn hexadecimal() -> String {
    let bytes = encoding::decode("ff").unwrap();
    let mut target = [0];
    encoding::decode_to_slice("ab", &mut target).unwrap();
    let text: String = bytes.encode_hex();
    let suffix = encoding::encode(target);
    format!("{text}{suffix}")
}
"#;

    let (_, fixed) = support::fixed(source, "hex=\"=0.4.3\"")?;
    assert_eq!(fixed, source.replace("encoding::", "hex::"));
    Ok(())
}
