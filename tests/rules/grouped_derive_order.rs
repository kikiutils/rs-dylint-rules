use anyhow::Result;

use crate::support;

#[test]
fn fixes_builtin_lists_independently_and_resolves_aliases() -> Result<()> {
    let source = "use core::fmt::Debug as FormatDebug;\n#[derive(Copy, Clone)]\n#[derive(Hash, Eq, PartialEq, FormatDebug)]\npub struct Value(u32);\nfn nested() {\n    #[derive(Eq, PartialEq, Debug)]\n    struct Nested;\n}\n";
    let directory = support::fixture(source)?;
    let output = support::run(&directory, false, "kiki_grouped_derive_order")?;
    assert!(output.status.success(), "{output:?}");
    assert!(support::has_lint(&output, "kiki_grouped_derive_order"), "{output:?}");
    let output = support::run(&directory, true, "kiki_grouped_derive_order")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        support::source(&directory)?,
        "use core::fmt::Debug as FormatDebug;\n#[derive(Clone, Copy)]\n#[derive(FormatDebug, PartialEq, Eq, Hash)]\npub struct Value(u32);\nfn nested() {\n    #[derive(Debug, PartialEq, Eq)]\n    struct Nested;\n}\n"
    );

    let output = support::run(&directory, false, "kiki_grouped_derive_order")?;
    assert!(output.status.success(), "{output:?}");
    assert!(!support::has_lint(&output, "kiki_grouped_derive_order"), "{output:?}");
    Ok(())
}

#[test]
fn all_builtin_derives_preserve_behavior_and_raw_source_offsets() -> Result<()> {
    let source = "const LABEL: &str = \"型別\";\r\n#[derive(Hash, Ord, Eq, Copy, PartialOrd, Default, Clone, Debug, PartialEq)]\r\npub struct Value(u32);\r\n#[test] fn behavior() { let value = Value::default(); assert_eq!(value, value.clone()); assert!(value < Value(1)); assert_eq!(format!(\"{value:?}\"), \"Value(0)\"); use std::hash::Hash; let mut hasher = std::collections::hash_map::DefaultHasher::new(); value.hash(&mut hasher); }\r\n";
    let directory = support::fixture(source)?;
    for fix in [false, true] {
        let output = support::run(&directory, fix, "kiki_grouped_derive_order")?;
        assert!(output.status.success(), "{output:?}");
        support::behavior(&directory)?;
    }

    assert_eq!(
        support::source(&directory)?,
        source.replace(
            "Hash, Ord, Eq, Copy, PartialOrd, Default, Clone, Debug, PartialEq",
            "Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash"
        )
    );

    Ok(())
}

#[test]
fn serde_aliases_and_qualified_paths_sort_and_preserve_roundtrip_behavior() -> Result<()> {
    let source = r##"#![allow(dead_code)]
use renamed::{Deserialize as Decode, Serialize as Encode};
#[derive(Decode, Eq, Encode, Debug, PartialEq)]
#[cfg_attr(all(), serde(crate = "renamed", rename_all = "camelCase"), allow(dead_code))]
pub struct Value { pub some_number: u32 }
#[derive(renamed::Deserialize, renamed::Serialize)]
#[serde(crate = "renamed")]
pub enum State { Ready, Value(u32) }
#[test] fn roundtrip() {
    let value = Value { some_number: 7 };
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, r#"{"someNumber":7}"#);
    assert_eq!(serde_json::from_str::<Value>(&json).unwrap(), value);
    let state = serde_json::to_string(&State::Value(9)).unwrap();
    assert_eq!(serde_json::to_string(&serde_json::from_str::<State>(&state).unwrap()).unwrap(), state);
}
"##;

    let directory = support::fixture(source)?;
    support::dependency(
        &directory,
        "renamed={package=\"serde\",version=\"=1.0.229\", features=[\"derive\"]}\nserde_json=\"=1.0.154\"",
    )?;

    for fix in [false, true] {
        let output = support::run(&directory, fix, "kiki_grouped_derive_order")?;
        assert!(output.status.success(), "{output:?}");
        if !fix {
            assert!(support::has_lint(&output, "kiki_grouped_derive_order"), "{output:?}");
        }

        support::behavior(&directory)?;
    }

    assert_eq!(
        support::source(&directory)?,
        source
            .replace(
                "Decode, Eq, Encode, Debug, PartialEq",
                "Debug, PartialEq, Eq, Encode, Decode"
            )
            .replace(
                "renamed::Deserialize, renamed::Serialize",
                "renamed::Serialize, renamed::Deserialize"
            )
    );

    let output = support::run(&directory, false, "kiki_grouped_derive_order")?;
    assert!(output.status.success(), "{output:?}");
    assert!(!support::has_lint(&output, "kiki_grouped_derive_order"), "{output:?}");
    Ok(())
}

#[test]
fn thiserror_derives_preserve_display_conversions_and_sources() -> Result<()> {
    let source = r#"#[derive(thiserror::Error, Debug)]
pub enum Failure {
    #[error("io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("context: {source}")]
    Context { #[source] source: std::io::Error },
}
#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct Wrapped(#[from] Failure);
#[test] fn behavior() {
    use std::error::Error;
    let failure = Failure::from(std::io::Error::other("disk"));
    assert_eq!(failure.to_string(), "io failed: disk");
    assert_eq!(failure.source().unwrap().to_string(), "disk");
    let wrapped = Wrapped::from(failure);
    assert_eq!(wrapped.to_string(), "io failed: disk");
    assert_eq!(wrapped.source().unwrap().to_string(), "disk");
    let context = Failure::Context { source: std::io::Error::other("network") };
    assert_eq!(context.to_string(), "context: network");
    assert_eq!(context.source().unwrap().to_string(), "network");
}
"#;

    let directory = support::fixture(source)?;
    support::dependency(&directory, "thiserror=\"=2.0.21\"")?;

    for fix in [false, true] {
        let output = support::run(&directory, fix, "kiki_grouped_derive_order")?;
        assert!(output.status.success(), "{output:?}");
        if !fix {
            assert_eq!(support::diagnostics(&output, "kiki_grouped_derive_order").len(), 2);
        }

        support::behavior(&directory)?;
    }

    let expected = source.replace("thiserror::Error, Debug", "Debug, thiserror::Error");
    assert_eq!(support::source(&directory)?, expected);
    let output = support::run(&directory, true, "kiki_grouped_derive_order")?;
    assert!(output.status.success(), "{output:?}");
    assert!(!support::has_lint(&output, "kiki_grouped_derive_order"), "{output:?}");
    assert_eq!(support::source(&directory)?, expected);
    Ok(())
}

#[test]
fn supported_third_party_derives_preserve_behavior() -> Result<()> {
    let source = r#"use is_empty::IsEmpty as Empty;
use serde_repr::{Deserialize_repr as Decode, Serialize_repr as Encode};
use strum::{EnumCount as Count, VariantNames as Names};
#[derive(Empty, Default, Debug)]
pub struct Record {
    value: Option<u8>,
    #[is_empty(if = "Vec::is_empty")]
    extra: Vec<u8>,
}
#[derive(Decode, Encode, Debug, PartialEq)]
#[repr(u8)]
pub enum Number { One = 1, Two = 2 }
#[derive(strum::VariantArray, Names, Count, strum::EnumIs, Debug, PartialEq)]
pub enum State { Ready, Done }
#[test] fn behavior() {
    use is_empty::IsEmpty;
    assert!(Record::default().is_empty());
    assert!(!Record { value: Some(1), extra: vec![] }.is_empty());
    assert_eq!(serde_json::to_string(&Number::Two).unwrap(), "2");
    assert_eq!(serde_json::from_str::<Number>("1").unwrap(), Number::One);
    assert_eq!(<State as strum::EnumCount>::COUNT, 2);
    assert_eq!(<State as strum::VariantNames>::VARIANTS, &["Ready", "Done"]);
    assert_eq!(<State as strum::VariantArray>::VARIANTS, &[State::Ready, State::Done]);
    assert!(State::Ready.is_ready());
    assert!(!State::Done.is_ready());
}
"#;

    let directory = support::fixture(source)?;
    support::dependency(
        &directory,
        "is_empty=\"=0.3.0\"\nserde=\"=1.0.229\"\nserde_repr=\"=0.1.21\"\nserde_json=\"=1.0.154\"\nstrum={version=\"=0.28.0\",default-features=false,features=[\"derive\"]}",
    )?;

    for fix in [false, true] {
        let output = support::run(&directory, fix, "kiki_grouped_derive_order")?;
        assert!(output.status.success(), "{output:?}");
        if !fix {
            assert!(support::has_lint(&output, "kiki_grouped_derive_order"), "{output:?}");
        }

        support::behavior(&directory)?;
    }

    assert_eq!(
        support::source(&directory)?,
        source
            .replace("Empty, Default, Debug", "Debug, Default, Empty")
            .replace("Decode, Encode, Debug, PartialEq", "Debug, PartialEq, Encode, Decode")
            .replace(
                "strum::VariantArray, Names, Count, strum::EnumIs, Debug, PartialEq",
                "Debug, PartialEq, strum::EnumIs, Count, Names, strum::VariantArray"
            )
    );

    let output = support::run(&directory, false, "kiki_grouped_derive_order")?;
    assert!(output.status.success(), "{output:?}");
    assert!(!support::has_lint(&output, "kiki_grouped_derive_order"), "{output:?}");
    Ok(())
}

#[test]
fn inactive_derives_do_not_produce_facts() -> Result<()> {
    let directory = support::fixture(
        "#[cfg(any())]\n#[derive(Missing, Eq, PartialEq)] struct Inactive;\n#[derive(PartialEq, Eq)] pub struct Active;\n",
    )?;

    let output = support::run(&directory, false, "kiki_grouped_derive_order")?;
    assert!(output.status.success(), "{output:?}");
    assert!(!support::has_lint(&output, "kiki_grouped_derive_order"), "{output:?}");
    Ok(())
}

#[test]
fn derive_more_derives_resolve_helpers_and_preserve_behavior() -> Result<()> {
    let source = r#"use derive_more::{Debug as MoreDebug, Display as MoreDisplay};
#[derive(derive_more::FromStr, derive_more::Constructor, derive_more::From, MoreDisplay, derive_more::Eq, derive_more::PartialEq, MoreDebug)]
#[cfg_attr(all(), cfg_attr(all(), display("number:{_0}")), allow(dead_code))]
#[debug("Number({_0})")]
#[from(u8, u16)]
pub struct Number(u32);
#[derive(derive_more::Error, derive_more::Display, Debug)]
#[display("failed")]
#[error(ignore)]
pub struct Failure;
#[derive(derive_more::IntoIterator, derive_more::IndexMut, derive_more::Index, derive_more::DerefMut, derive_more::Deref, derive_more::AsMut, derive_more::AsRef, Debug)]
#[into_iterator(owned, ref, ref_mut)]
#[deref(forward)]
#[deref_mut(forward)]
#[as_ref(Vec<T>)]
#[as_mut(Vec<T>)]
pub struct Buffer<T>(Vec<T>);
#[derive(derive_more::TryUnwrap, derive_more::Unwrap, derive_more::IsVariant, Debug)]
#[unwrap(owned, ref)]
#[try_unwrap(owned, ref_mut)]
pub enum Choice { Value(u32), #[is_variant(ignore)] #[unwrap(ignore)] #[try_unwrap(ignore)] Empty }
#[derive(derive_more::TryFrom, PartialEq, Debug)]
#[repr(u8)]
#[try_from(repr)]
pub enum Code { One = 1, Two = 2 }
#[derive(derive_more::IntoIterator, Debug)]
#[into_iterator(ref, ref_mut)]
pub struct Borrowed<T>(Vec<T>);
#[test] fn behavior() {
    let number = Number::new(7);
    assert_eq!(number, Number::from(7u8));
    assert_eq!(number, Number::from(7u16));
    assert_eq!("7".parse::<Number>().unwrap(), number);
    assert_eq!(number.to_string(), "number:7");
    assert_eq!(format!("{number:?}"), "Number(7)");
    let error: &dyn std::error::Error = &Failure;
    assert_eq!(error.to_string(), "failed");
    assert!(error.source().is_none());
    let mut buffer = Buffer(vec![1, 2]);
    let view: &Vec<i32> = buffer.as_ref();
    assert_eq!(view, &[1, 2]);
    let view: &mut Vec<i32> = buffer.as_mut();
    view.push(3);
    buffer[0] = 4;
    assert_eq!(&*buffer, &[4, 2, 3]);
    let slice: &mut [i32] = &mut buffer;
    slice[1] += 1;
    assert_eq!((&buffer).into_iter().copied().sum::<i32>(), 10);
    for value in &mut buffer { *value += 1; }
    assert_eq!(buffer.into_iter().collect::<Vec<_>>(), [5, 4, 4]);
    let mut choice = Choice::Value(5);
    assert!(choice.is_value());
    assert_eq!(*choice.unwrap_value_ref(), 5);
    *choice.try_unwrap_value_mut().unwrap() = 6;
    assert_eq!(choice.try_unwrap_value().unwrap(), 6);
    assert_eq!(Choice::Value(8).unwrap_value(), 8);
    assert_eq!(Code::try_from(2u8).unwrap(), Code::Two);
    assert!(Code::try_from(0u8).is_err());
    let mut borrowed = Borrowed(vec![1, 2]);
    for value in &mut borrowed { *value *= 2; }
    assert_eq!((&borrowed).into_iter().copied().collect::<Vec<_>>(), [2, 4]);
}
"#;

    let directory = support::fixture(source)?;
    support::dependency(
        &directory,
        "derive_more={version=\"=2.1.1\",features=[\"debug\",\"display\",\"error\",\"from\",\"try_from\",\"from_str\",\"as_ref\",\"deref\",\"deref_mut\",\"is_variant\",\"eq\",\"constructor\",\"unwrap\",\"try_unwrap\",\"into_iterator\",\"index\",\"index_mut\"]}",
    )?;

    for fix in [false, true] {
        let output = support::run(&directory, fix, "kiki_grouped_derive_order")?;
        assert!(output.status.success(), "{output:?}");
        if !fix {
            assert_eq!(
                support::diagnostics(&output, "kiki_grouped_derive_order").len(),
                6,
                "{output:?}"
            );
        }

        support::behavior(&directory)?;
    }

    let expected = source
        .replace("derive_more::FromStr, derive_more::Constructor, derive_more::From, MoreDisplay, derive_more::Eq, derive_more::PartialEq, MoreDebug", "MoreDebug, MoreDisplay, derive_more::Constructor, derive_more::PartialEq, derive_more::Eq, derive_more::From, derive_more::FromStr")
        .replace("derive_more::Error, derive_more::Display, Debug", "Debug, derive_more::Display, derive_more::Error")
        .replace("derive_more::IntoIterator, derive_more::IndexMut, derive_more::Index, derive_more::DerefMut, derive_more::Deref, derive_more::AsMut, derive_more::AsRef, Debug", "Debug, derive_more::AsRef, derive_more::AsMut, derive_more::Deref, derive_more::DerefMut, derive_more::Index, derive_more::IndexMut, derive_more::IntoIterator")
        .replace("derive_more::TryUnwrap, derive_more::Unwrap, derive_more::IsVariant, Debug", "Debug, derive_more::IsVariant, derive_more::Unwrap, derive_more::TryUnwrap")
        .replace("derive_more::TryFrom, PartialEq, Debug", "Debug, PartialEq, derive_more::TryFrom")
        .replace("derive_more::IntoIterator, Debug", "Debug, derive_more::IntoIterator");

    assert_eq!(support::source(&directory)?, expected);

    let output = support::run(&directory, false, "kiki_grouped_derive_order")?;
    assert!(output.status.success(), "{output:?}");
    assert!(!support::has_lint(&output, "kiki_grouped_derive_order"), "{output:?}");
    Ok(())
}

#[test]
fn additional_strum_derives_preserve_behavior_and_ignore_generated_types() -> Result<()> {
    let source = r#"use strum::Display as Text;
#[derive(strum::EnumMessage, strum::EnumProperty, strum::EnumIter, strum::FromRepr, strum::EnumString, strum::AsRefStr, Text, PartialEq, Debug)]
#[cfg_attr(all(), strum(serialize_all = "snake_case"))]
#[repr(u8)]
pub enum State {
    #[strum(message = "ready", detailed_message = "ready to run", props(color = "green"))]
    Ready = 1,
    Done = 2,
}
#[derive(strum::EnumTryAs, PartialEq, Debug)]
#[strum(crate = "strum")]
pub enum Payload<T> { Value(T), #[strum(disabled)] Hidden(T), Empty }
#[test] fn behavior() {
    use strum::{EnumMessage, EnumProperty, IntoEnumIterator};
    assert_eq!("ready".parse::<State>().unwrap(), State::Ready);
    assert!("missing".parse::<State>().is_err());
    assert_eq!(State::Done.to_string(), "done");
    let text: &str = State::Ready.as_ref();
    assert_eq!(text, "ready");
    assert_eq!(State::from_repr(2), Some(State::Done));
    assert_eq!(State::from_repr(0), None);
    assert_eq!(State::iter().collect::<Vec<_>>(), [State::Ready, State::Done]);
    assert_eq!(State::Ready.get_str("color"), Some("green"));
    assert_eq!(State::Done.get_str("color"), None);
    assert_eq!(State::Ready.get_message(), Some("ready"));
    assert_eq!(State::Ready.get_detailed_message(), Some("ready to run"));
    let mut value = Payload::Value(7u32);
    assert_eq!(value.try_as_value_ref(), Some(&7));
    *value.try_as_value_mut().unwrap() = 8;
    assert_eq!(value.try_as_value(), Some(8));
    assert_eq!(Payload::<u32>::Empty.try_as_value(), None);
}
"#;

    let directory = support::fixture(source)?;
    support::dependency(
        &directory,
        "strum={version=\"=0.28.0\",default-features=false,features=[\"derive\"]}",
    )?;

    for fix in [false, true] {
        let output = support::run(&directory, fix, "kiki_grouped_derive_order")?;
        assert!(output.status.success(), "{output:?}");
        if !fix {
            assert_eq!(
                support::diagnostics(&output, "kiki_grouped_derive_order").len(),
                2,
                "{output:?}"
            );
        }

        support::behavior(&directory)?;
    }

    assert_eq!(
        support::source(&directory)?,
        source
            .replace("strum::EnumMessage, strum::EnumProperty, strum::EnumIter, strum::FromRepr, strum::EnumString, strum::AsRefStr, Text, PartialEq, Debug", "Debug, Text, PartialEq, strum::EnumProperty, strum::EnumMessage, strum::EnumString, strum::FromRepr, strum::AsRefStr, strum::EnumIter")
            .replace("strum::EnumTryAs, PartialEq, Debug", "Debug, PartialEq, strum::EnumTryAs")
    );

    let output = support::run(&directory, false, "kiki_grouped_derive_order")?;
    assert!(output.status.success(), "{output:?}");
    assert!(!support::has_lint(&output, "kiki_grouped_derive_order"), "{output:?}");
    Ok(())
}
