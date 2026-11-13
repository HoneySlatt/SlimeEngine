//! A family's own types, written into a message and read out of one.
//!
//! A family types its requests as an enum that `serde` tags *adjacently*, its method the tag and
//! its fields the content, `#[serde(tag = "method", content = "params")]`, each variant renamed
//! `family/name`; its answers as values of its own.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::AgentError;
use crate::message::Fault;

/// `value` as the JSON that a message holds.
///
/// A float that is not finite is written as `null`, which JSON alone holds in its place: a family's
/// float reads `null` as a [`Fault::BAD_PARAMS`], so that such a float is refused where it is read.
/// A family therefore holds no optional float, which would read `null` as none.
///
/// # Errors
///
/// [`AgentError::Unwritable`] when JSON cannot write it, as a map whose keys are not text.
pub fn write(value: &impl Serialize) -> Result<Value, AgentError> {
    serde_json::to_value(value).map_err(AgentError::unwritable)
}

/// How many bytes of JSON `value` writes, counted as it is written, without holding them: what a
/// carrier counts of an answer that waits to be written. A value that JSON cannot write counts
/// what it wrote before it failed.
pub fn json_size(value: &impl Serialize) -> usize {
    /// Counts what is written to it, and keeps none of it.
    struct Counter(usize);

    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += bytes.len();
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut counter = Counter(0);
    let _ = serde_json::to_writer(&mut counter, value);
    counter.0
}

/// A value of a family's own type read out of the JSON that a message holds.
///
/// # Errors
///
/// A [`Fault::BAD_PARAMS`] saying why, when `value` does not read as a `T`.
pub fn read<T: DeserializeOwned>(value: Value) -> Result<T, Fault> {
    serde_json::from_value(value).map_err(|error| Fault::new(Fault::BAD_PARAMS, error.to_string()))
}

/// The method and the parameters of `call`, a request of a family's enum, tagged adjacently.
///
/// # Errors
///
/// [`AgentError::NotACall`] when `call` is not written as an object with a `method` of text and
/// its `params`, if any; [`AgentError::Unwritable`] when JSON cannot write it.
pub fn call(call: &impl Serialize) -> Result<(String, Value), AgentError> {
    let not_a_call = || AgentError::NotACall;
    let Value::Object(mut written) = write(call)? else {
        return Err(not_a_call());
    };
    let Some(Value::String(method)) = written.remove("method") else {
        return Err(not_a_call());
    };
    let params = written.remove("params").unwrap_or(Value::Null);
    if !written.is_empty() {
        return Err(not_a_call());
    }
    Ok((method, params))
}

/// The request of a family's enum, tagged adjacently, that `method` and `params` make.
///
/// # Errors
///
/// A [`Fault::NO_METHOD`] when the enum has no such method; a [`Fault::BAD_PARAMS`] when the
/// method's parameters do not read.
pub fn read_call<T: DeserializeOwned>(method: &str, params: Value) -> Result<T, Fault> {
    let mut written = serde_json::Map::new();
    written.insert("method".to_owned(), Value::String(method.to_owned()));
    if !params.is_null() {
        written.insert("params".to_owned(), params);
    }
    serde_json::from_value(Value::Object(written)).map_err(|error| {
        let message = error.to_string();
        // serde names the tag it did not know: the method, or a value of the parameters' own.
        let code = if message.starts_with(&format!("unknown variant `{method}`")) {
            Fault::NO_METHOD
        } else {
            Fault::BAD_PARAMS
        };
        Fault::new(code, message)
    })
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(tag = "method", content = "params")]
    enum Play {
        #[serde(rename = "play/pause")]
        Pause,
        #[serde(rename = "play/step")]
        Step { steps: u32 },
        #[serde(rename = "play/act")]
        Act { kind: Kind },
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Kind {
        Press,
    }

    #[test]
    fn a_family_s_requests_cross_as_their_method_and_their_parameters() {
        assert_eq!(
            call(&Play::Pause).expect("a call"),
            ("play/pause".to_owned(), Value::Null)
        );
        let (method, params) = call(&Play::Step { steps: 3 }).expect("a call");
        assert_eq!(
            (method.as_str(), &params),
            ("play/step", &json!({ "steps": 3 }))
        );
        assert_eq!(
            read_call::<Play>(&method, params),
            Ok(Play::Step { steps: 3 })
        );
        assert_eq!(
            read_call::<Play>("play/pause", Value::Null),
            Ok(Play::Pause)
        );
        assert_eq!(
            read_call::<Play>("play/rewind", Value::Null).map_err(|fault| fault.code),
            Err(Fault::NO_METHOD)
        );
        assert_eq!(
            read_call::<Play>("play/step", json!({ "steps": "three" })).map_err(|fault| fault.code),
            Err(Fault::BAD_PARAMS)
        );
        assert_eq!(
            read_call::<Play>("play/act", json!({ "kind": "jump" })).map_err(|fault| fault.code),
            Err(Fault::BAD_PARAMS),
            "a value of the parameters that the family does not know is no unknown method"
        );
        assert!(matches!(call(&3), Err(AgentError::NotACall)));
        assert!(matches!(
            call(&json!({ "method": "a/b", "more": 1 })),
            Err(AgentError::NotACall)
        ));
        assert_eq!(read::<u32>(json!(7)), Ok(7));
        assert_eq!(
            read::<u32>(json!(-7)).map_err(|fault| fault.code),
            Err(Fault::BAD_PARAMS)
        );
    }

    #[test]
    fn a_value_s_size_is_the_bytes_of_its_json() {
        let value = serde_json::json!({ "text": "lamp 3", "steps": [1, 2, 3] });
        assert_eq!(
            json_size(&value),
            serde_json::to_vec(&value).expect("written").len()
        );
    }
}
