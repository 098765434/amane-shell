//! Reading what a newer program sends: an element this bar doesn't know (an
//! icon, a trailing kind, a tint, a menu entry or button it can't read) is
//! dropped with a warning and the rest of the payload kept, as Adaptive Cards
//! renderers do. Unknown properties are kept out by serde already.

use std::any::type_name;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// The short name of a type, for a warning.
fn short<T>() -> &'static str {
    let name = type_name::<T>();
    name.rsplit("::").next().unwrap_or(name)
}

/// Reads one element, or warns and gives `None` when it can't.
pub(crate) fn element<T: DeserializeOwned>(value: Value) -> Option<T> {
    match serde_json::from_value(value.clone()) {
        Ok(element) => Some(element),
        Err(error) => {
            log::warn!(
                "dropped a {} this bar doesn't know: {value} ({error})",
                short::<T>()
            );
            None
        }
    }
}

/// A field whose value may be newer than this bar: its default when it can't be read.
pub(crate) fn or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    Ok(element(Value::deserialize(deserializer)?).unwrap_or_default())
}

/// An optional field the same way: none when it can't be read.
pub(crate) fn or_none<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Ok(match Value::deserialize(deserializer)? {
        Value::Null => None,
        value => element(value),
    })
}

/// A list: the elements that can be read, in order, the rest dropped.
pub(crate) fn each<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Ok(match Value::deserialize(deserializer)? {
        Value::Array(values) => values.into_iter().filter_map(element).collect(),
        Value::Null => Vec::new(),
        other => {
            log::warn!(
                "dropped a list of {} that isn't a list: {other}",
                short::<T>()
            );
            Vec::new()
        }
    })
}
