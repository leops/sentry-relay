use relay_protocol::{Annotated, Empty, FromValue, Getter, IntoValue, Object, Val, Value};

use crate::processor::ProcessValue;
use crate::protocol::{JsonLenientString, Mechanism, RawStacktrace, Stacktrace, ThreadId};

/// A single exception.
///
/// Multiple values inside of an [event](#typedef-Event) represent chained exceptions and should be sorted oldest to newest. For example, consider this Python code snippet:
///
/// ```python
/// try:
///     raise Exception("random boring invariant was not met!")
/// except Exception as e:
///     raise ValueError("something went wrong, help!") from e
/// ```
///
/// `Exception` would be described first in the values list, followed by a description of `ValueError`:
///
/// ```json
/// {
///   "exception": {
///     "values": [
///       {"type": "Exception": "value": "random boring invariant was not met!"},
///       {"type": "ValueError", "value": "something went wrong, help!"},
///     ]
///   }
/// }
/// ```
#[derive(Clone, Debug, Default, PartialEq, Empty, FromValue, IntoValue, ProcessValue)]
#[metastructure(process_func = "process_exception", value_type = "Exception")]
pub struct Exception {
    /// Exception type, e.g. `ValueError`.
    ///
    /// At least one of `type` or `value` is required, otherwise the exception is discarded.
    // (note: requirement checked in checked in StoreNormalizeProcessor)
    #[metastructure(field = "type", max_chars = 256, max_chars_allowance = 20)]
    pub ty: Annotated<String>,

    /// Human readable display value.
    ///
    /// At least one of `type` or `value` is required, otherwise the exception is discarded.
    #[metastructure(max_chars = 8192, max_chars_allowance = 200, pii = "true")]
    pub value: Annotated<JsonLenientString>,

    /// The optional module, or package which the exception type lives in.
    #[metastructure(max_chars = 256, max_chars_allowance = 20)]
    pub module: Annotated<String>,

    /// Stack trace containing frames of this exception.
    #[metastructure(
        legacy_alias = "sentry.interfaces.Stacktrace",
        skip_serialization = "empty"
    )]
    pub stacktrace: Annotated<Stacktrace>,

    /// Optional unprocessed stack trace.
    #[metastructure(skip_serialization = "empty", omit_from_schema)]
    pub raw_stacktrace: Annotated<RawStacktrace>,

    /// An optional value that refers to a [thread](#typedef-Thread).
    #[metastructure(max_chars = 128)]
    pub thread_id: Annotated<ThreadId>,

    /// Mechanism by which this exception was generated and handled.
    pub mechanism: Annotated<Mechanism>,

    /// Additional arbitrary fields for forwards compatibility.
    #[metastructure(additional_properties)]
    pub other: Object<Value>,
}

impl Getter for Exception {
    fn get_value(&self, path: &str) -> Option<Val<'_>> {
        Some(match path {
            "ty" => self.ty.as_str()?.into(),
            "value" => self.value.as_str()?.into(),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use relay_protocol::Map;
    use similar_asserts::assert_eq;

    use super::*;

    #[test]
    fn test_exception_roundtrip() {
        // stack traces and mechanism are tested separately
        let json = r#"{
  "type": "mytype",
  "value": "myvalue",
  "module": "mymodule",
  "thread_id": 42,
  "other": "value"
}"#;
        let exception = Annotated::new(Exception {
            ty: Annotated::new("mytype".to_owned()),
            value: Annotated::new("myvalue".to_owned().into()),
            module: Annotated::new("mymodule".to_owned()),
            thread_id: Annotated::new(ThreadId::Int(42)),
            other: {
                let mut map = Map::new();
                map.insert(
                    "other".to_owned(),
                    Annotated::new(Value::String("value".to_owned())),
                );
                map
            },
            ..Default::default()
        });

        assert_eq!(exception, Annotated::from_json(json).unwrap());
        assert_eq!(json, exception.to_json_pretty().unwrap());
    }

    #[test]
    fn test_exception_default_values() {
        let json = r#"{"type":"mytype"}"#;
        let exception = Annotated::new(Exception {
            ty: Annotated::new("mytype".to_owned()),
            ..Default::default()
        });

        assert_eq!(exception, Annotated::from_json(json).unwrap());
        assert_eq!(json, exception.to_json().unwrap());
    }

    #[test]
    fn test_exception_empty_fields() {
        let json = r#"{"type":"","value":""}"#;
        let exception = Annotated::new(Exception {
            ty: Annotated::new("".to_owned()),
            value: Annotated::new("".to_owned().into()),
            ..Default::default()
        });

        assert_eq!(exception, Annotated::from_json(json).unwrap());
        assert_eq!(json, exception.to_json().unwrap());
    }

    #[test]
    fn test_coerces_object_value_to_string() {
        let input = r#"{"value":{"unauthorized":true}}"#;
        let output = r#"{"value":"{\"unauthorized\":true}"}"#;

        let exception = Annotated::new(Exception {
            value: Annotated::new(r#"{"unauthorized":true}"#.to_owned().into()),
            ..Default::default()
        });

        assert_eq!(exception, Annotated::from_json(input).unwrap());
        assert_eq!(output, exception.to_json().unwrap());
    }

    #[test]
    fn test_explicit_none() {
        let json = r#"{
  "value": null,
  "type": "ZeroDivisionError"
}"#;

        let exception = Annotated::new(Exception {
            ty: Annotated::new("ZeroDivisionError".to_owned()),
            ..Default::default()
        });

        assert_eq!(exception, Annotated::from_json(json).unwrap());
        assert_eq!(
            r#"{"type":"ZeroDivisionError"}"#,
            exception.to_json().unwrap()
        );
    }
}
