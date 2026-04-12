//! Serde helpers for deserializing empty strings as None.
//!
//! Frontend date/uuid inputs often send `""` instead of `null` when the field is left blank.
//! These helpers treat empty strings as None so the API doesn't reject the request.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer};
use uuid::Uuid;

/// Deserialize `Option<DateTime<Utc>>` treating `""` as `None`.
pub fn empty_string_as_none_datetime<'de, D>(
    deserializer: D,
) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    match opt {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(ref s)) if s.is_empty() => Ok(None),
        Some(v) => DateTime::<Utc>::deserialize(v)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

/// Deserialize `Option<Option<DateTime<Utc>>>` treating `""` as `Some(None)` (explicit clear).
/// Field absent → `None` (via `#[serde(default)]`), `null`/`""` → `Some(None)`, valid → `Some(Some(dt))`.
pub fn empty_string_as_none_option_datetime<'de, D>(
    deserializer: D,
) -> Result<Option<Option<DateTime<Utc>>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(serde_json::Value::Null) => Ok(Some(None)),
        Some(serde_json::Value::String(ref s)) if s.is_empty() => Ok(Some(None)),
        Some(v) => DateTime::<Utc>::deserialize(v)
            .map(|dt| Some(Some(dt)))
            .map_err(serde::de::Error::custom),
    }
}

/// Deserialize `Option<Uuid>` treating `""` as `None`.
pub fn empty_string_as_none_uuid<'de, D>(deserializer: D) -> Result<Option<Uuid>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    match opt {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(ref s)) if s.is_empty() => Ok(None),
        Some(v) => Uuid::deserialize(v)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

/// Deserialize `Option<Option<Uuid>>` treating `""` as `Some(None)` (explicit clear).
pub fn empty_string_as_none_option_uuid<'de, D>(
    deserializer: D,
) -> Result<Option<Option<Uuid>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(serde_json::Value::Null) => Ok(Some(None)),
        Some(serde_json::Value::String(ref s)) if s.is_empty() => Ok(Some(None)),
        Some(v) => Uuid::deserialize(v)
            .map(|u| Some(Some(u)))
            .map_err(serde::de::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct TestDt {
        #[serde(default, deserialize_with = "empty_string_as_none_datetime")]
        dt: Option<DateTime<Utc>>,
    }

    #[derive(Debug, Deserialize)]
    struct TestOptDt {
        #[serde(default, deserialize_with = "empty_string_as_none_option_datetime")]
        dt: Option<Option<DateTime<Utc>>>,
    }

    #[derive(Debug, Deserialize)]
    struct TestUuid {
        #[serde(default, deserialize_with = "empty_string_as_none_uuid")]
        id: Option<Uuid>,
    }

    #[derive(Debug, Deserialize)]
    struct TestOptUuid {
        #[serde(default, deserialize_with = "empty_string_as_none_option_uuid")]
        id: Option<Option<Uuid>>,
    }

    // --- DateTime ---

    #[test]
    fn datetime_null_becomes_none() {
        let t: TestDt = serde_json::from_str(r#"{"dt": null}"#).unwrap();
        assert!(t.dt.is_none());
    }

    #[test]
    fn datetime_empty_string_becomes_none() {
        let t: TestDt = serde_json::from_str(r#"{"dt": ""}"#).unwrap();
        assert!(t.dt.is_none());
    }

    #[test]
    fn datetime_absent_becomes_none() {
        let t: TestDt = serde_json::from_str(r#"{}"#).unwrap();
        assert!(t.dt.is_none());
    }

    #[test]
    fn datetime_valid_parses() {
        let t: TestDt = serde_json::from_str(r#"{"dt": "2026-04-12T00:00:00Z"}"#).unwrap();
        assert!(t.dt.is_some());
    }

    #[test]
    fn datetime_invalid_errors() {
        let r = serde_json::from_str::<TestDt>(r#"{"dt": "not-a-date"}"#);
        assert!(r.is_err());
    }

    // --- Option<Option<DateTime>> ---

    #[test]
    fn opt_datetime_absent_is_none() {
        let t: TestOptDt = serde_json::from_str(r#"{}"#).unwrap();
        assert!(t.dt.is_none()); // field absent = don't update
    }

    #[test]
    fn opt_datetime_null_is_some_none() {
        let t: TestOptDt = serde_json::from_str(r#"{"dt": null}"#).unwrap();
        assert_eq!(t.dt, Some(None)); // explicit clear
    }

    #[test]
    fn opt_datetime_empty_is_some_none() {
        let t: TestOptDt = serde_json::from_str(r#"{"dt": ""}"#).unwrap();
        assert_eq!(t.dt, Some(None)); // empty string = clear
    }

    #[test]
    fn opt_datetime_valid_parses() {
        let t: TestOptDt = serde_json::from_str(r#"{"dt": "2026-04-12T00:00:00Z"}"#).unwrap();
        assert!(matches!(t.dt, Some(Some(_))));
    }

    // --- Uuid ---

    #[test]
    fn uuid_null_becomes_none() {
        let t: TestUuid = serde_json::from_str(r#"{"id": null}"#).unwrap();
        assert!(t.id.is_none());
    }

    #[test]
    fn uuid_empty_string_becomes_none() {
        let t: TestUuid = serde_json::from_str(r#"{"id": ""}"#).unwrap();
        assert!(t.id.is_none());
    }

    #[test]
    fn uuid_absent_becomes_none() {
        let t: TestUuid = serde_json::from_str(r#"{}"#).unwrap();
        assert!(t.id.is_none());
    }

    #[test]
    fn uuid_valid_parses() {
        let t: TestUuid =
            serde_json::from_str(r#"{"id": "550e8400-e29b-41d4-a716-446655440000"}"#).unwrap();
        assert!(t.id.is_some());
    }

    // --- Option<Option<Uuid>> ---

    #[test]
    fn opt_uuid_absent_is_none() {
        let t: TestOptUuid = serde_json::from_str(r#"{}"#).unwrap();
        assert!(t.id.is_none());
    }

    #[test]
    fn opt_uuid_null_is_some_none() {
        let t: TestOptUuid = serde_json::from_str(r#"{"id": null}"#).unwrap();
        assert_eq!(t.id, Some(None));
    }

    #[test]
    fn opt_uuid_empty_is_some_none() {
        let t: TestOptUuid = serde_json::from_str(r#"{"id": ""}"#).unwrap();
        assert_eq!(t.id, Some(None));
    }

    #[test]
    fn opt_uuid_valid_parses() {
        let t: TestOptUuid =
            serde_json::from_str(r#"{"id": "550e8400-e29b-41d4-a716-446655440000"}"#).unwrap();
        assert!(matches!(t.id, Some(Some(_))));
    }
}
