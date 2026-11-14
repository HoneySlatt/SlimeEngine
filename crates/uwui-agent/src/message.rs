//! The messages, JSON-RPC 2.0 a message a line, and the bytes that a message announces.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::error::AgentError;

/// The largest whole number that JSON carries exactly to every reader: JavaScript's numbers are
/// doubles. An identifier above it is refused; a value of a family that may pass it is written as
/// text (`docs/adr/0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md`).
pub const MAX_EXACT_INTEGER: u64 = (1 << 53) - 1;

/// A message: one of JSON-RPC 2.0's three kinds.
///
/// A method is `family/name`, `play/pause`; the core's own, `hello`, names no family.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    /// A request, answered by a [`Message::Response`] of the same number.
    Request {
        /// Its number, from 0 to [`MAX_EXACT_INTEGER`], which its answer gives back.
        id: u64,
        /// What it asks, `family/name`.
        method: String,
        /// Its parameters; `Value::Null` when it has none, and then none are written.
        params: Value,
    },
    /// The answer to a request.
    Response {
        /// The request's number; `None` for the answer to a line that could not be read as a
        /// request, after which the connection is closed.
        id: Option<u64>,
        /// Its result, or why it failed.
        outcome: Result<Value, Fault>,
    },
    /// An event, which nothing answers: a notification of JSON-RPC's.
    Event {
        /// What it tells, `family/name`.
        method: String,
        /// What it holds; `Value::Null` when nothing, and then nothing is written.
        params: Value,
    },
}

/// Why a request failed: an error of JSON-RPC's, its code and its message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fault {
    /// What kind of failure: one of this type's constants, or a family's own.
    pub code: i64,
    /// Why, for a person or an agent to read.
    pub message: String,
}

impl Fault {
    /// A line that is not JSON: JSON-RPC's parse error.
    pub const PARSE: i64 = -32700;
    /// A message that is not a request of JSON-RPC's, or one out of its turn.
    pub const INVALID: i64 = -32600;
    /// A method that the family does not have.
    pub const NO_METHOD: i64 = -32601;
    /// Parameters that the method does not read.
    pub const BAD_PARAMS: i64 = -32602;
    /// The program refused what was asked, and says why: a request that it read and could not do.
    pub const REFUSED: i64 = -32000;
    /// The program refused the hello: another version of the core, or another secret.
    pub const NOT_WELCOME: i64 = -32001;
    /// A request of a family that the program does not serve on the connection.
    pub const NOT_SERVED: i64 = -32002;
    /// A request past the requests in flight that the program takes
    /// ([`InFlight`](crate::InFlight)): refused before it is read, the connection kept. It may be
    /// asked again once an answer came.
    pub const BUSY: i64 = -32003;
    /// An answer larger than the tool said it reads ([`Hello::limits`](crate::Hello::limits)), in
    /// place of it: the request was done, and its answer dropped.
    pub const TOO_LARGE: i64 = -32004;
    /// A cursor of a list that no longer holds: of another process, or of a world, an interface or
    /// a journal that another took the place of. The tool lists it again from its start
    /// ([`page`](crate::page)).
    pub const STALE: i64 = -32005;
    /// A request cancelled before it changed anything ([`CANCEL`](crate::CANCEL)), and never done:
    /// LSP's `RequestCancelled`.
    pub const CANCELLED: i64 = -32800;

    /// A fault of `code`, saying `message`.
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// A message, and the bytes that it announces, which follow its line on the connection: the
/// pixels of an image. Empty for most.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The message.
    pub message: Message,
    /// The bytes that follow it.
    pub bytes: Vec<u8>,
    /// The size of its line, its `\n` not counted: what a request in flight counts
    /// ([`InFlight::bytes`](crate::InFlight::bytes)).
    pub line: usize,
}

/// How large a connection lets a message's line, and the bytes that follow it, be: larger ones are
/// refused before they are read whole. A tool gives its own in its hello, so that the program
/// answers what it cannot read with a fault rather than a line that the tool refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    /// The most bytes of a line, its `\n` not counted; a `\r` before it is, as the whitespace that
    /// JSON lets a line end with.
    pub line: usize,
    /// The most bytes that may follow a line.
    pub bytes: u64,
}

impl Limits {
    /// Before the hello: 4 KiB of a line, and no bytes after it, so that who has no secret makes
    /// the program read little.
    pub const HELLO: Self = Self {
        line: 4096,
        bytes: 0,
    };
}

/// The envelope as written: JSON-RPC 2.0's members, and `bytes`, the count of those that follow.
#[derive(Serialize)]
struct Written<'a> {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Option<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a Fault>,
    #[serde(skip_serializing_if = "is_zero")]
    bytes: u64,
}

fn is_zero(count: &u64) -> bool {
    *count == 0
}

/// The envelope as read: each member present or not, `result: null` told from no result.
#[derive(Deserialize)]
struct Read {
    jsonrpc: String,
    #[serde(default, deserialize_with = "present")]
    id: Option<Value>,
    method: Option<String>,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default, deserialize_with = "present")]
    result: Option<Value>,
    error: Option<Fault>,
    #[serde(default)]
    bytes: u64,
}

/// A member that is present, `null` included: called only when the member is.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

/// Writes `message` as its line, its end included, announcing `bytes` that the caller sends after
/// it at once, on the same connection, with nothing between them.
///
/// # Errors
///
/// [`AgentError::Unwritable`] when a value cannot be written as JSON, as a map whose keys are not
/// text.
pub fn encode(message: &Message, bytes: usize) -> Result<Vec<u8>, AgentError> {
    let nothing = |params: &Value| params.is_null();
    let envelope = match message {
        Message::Request { id, method, params } => Written {
            id: Some(Some(*id)),
            method: Some(method),
            params: (!nothing(params)).then_some(params),
            ..envelope()
        },
        Message::Response { id, outcome } => Written {
            id: Some(*id),
            result: outcome.as_ref().ok(),
            error: outcome.as_ref().err(),
            ..envelope()
        },
        Message::Event { method, params } => Written {
            method: Some(method),
            params: (!nothing(params)).then_some(params),
            ..envelope()
        },
    };
    let mut line = serde_json::to_vec(&Written {
        bytes: bytes as u64,
        ..envelope
    })
    .map_err(AgentError::unwritable)?;
    line.push(b'\n');
    Ok(line)
}

fn envelope<'a>() -> Written<'a> {
    Written {
        jsonrpc: "2.0",
        id: None,
        method: None,
        params: None,
        result: None,
        error: None,
        bytes: 0,
    }
}

/// Reads the messages of a connection from its bytes as they come, in pieces of any size: the
/// carrier gives it what it received ([`Reader::push`]), and takes each whole message
/// ([`Reader::message`]).
///
/// A line ends at `\n`; a `\r` before it is whitespace, which JSON ignores. A line or the bytes it
/// announces past the [`Limits`] are refused before they are read whole; a line that is not a
/// message of JSON-RPC 2.0, an empty one among them, is refused too. Each refusal loses the framing: the connection is closed then.
#[derive(Debug)]
pub struct Reader {
    limits: Limits,
    buffer: Vec<u8>,
    /// How much of the buffer was looked through for a line's end.
    scanned: usize,
    /// A message read, waiting for the bytes it announced, their count, and its line's size.
    waiting: Option<(Message, usize, usize)>,
}

impl Reader {
    /// A reader within `limits`.
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            buffer: Vec::new(),
            scanned: 0,
            waiting: None,
        }
    }

    /// Reads within `limits` from now on, as a connection does once its hello is accepted.
    pub fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
    }

    /// Adds bytes that the connection received.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// The next whole message and its bytes, or `None` until more bytes are pushed.
    ///
    /// # Errors
    ///
    /// [`AgentError::TooLarge`] for a line or bytes past the limits, as soon as they pass them;
    /// [`AgentError::Malformed`] for a line that is not a message.
    pub fn message(&mut self) -> Result<Option<Frame>, AgentError> {
        if self.waiting.is_none() {
            let Some(end) = self.buffer[self.scanned..]
                .iter()
                .position(|&byte| byte == b'\n')
                .map(|at| self.scanned + at)
            else {
                self.scanned = self.buffer.len();
                return self.check_line(self.buffer.len()).map(|()| None);
            };
            self.check_line(end)?;
            let (message, bytes) = decode(&self.buffer[..end])?;
            if bytes > self.limits.bytes {
                return Err(AgentError::TooLarge {
                    what: "the bytes after a message",
                    size: bytes,
                    limit: self.limits.bytes,
                });
            }
            self.buffer.drain(..=end);
            self.scanned = 0;
            // Within the limit, which a `usize` holds wherever the limit is held in memory.
            self.waiting = Some((message, usize::try_from(bytes).unwrap_or(usize::MAX), end));
        }
        match self.waiting.take() {
            Some((message, count, line)) if self.buffer.len() >= count => {
                let rest = self.buffer.split_off(count);
                let bytes = std::mem::replace(&mut self.buffer, rest);
                Ok(Some(Frame {
                    message,
                    bytes,
                    line,
                }))
            }
            waiting => {
                self.waiting = waiting;
                Ok(None)
            }
        }
    }

    fn check_line(&self, length: usize) -> Result<(), AgentError> {
        if length > self.limits.line {
            return Err(AgentError::TooLarge {
                what: "a line",
                size: length as u64,
                limit: self.limits.line as u64,
            });
        }
        Ok(())
    }
}

/// Reads a line as a message, and the count of the bytes it announces.
fn decode(line: &[u8]) -> Result<(Message, u64), AgentError> {
    let malformed = |why: String| AgentError::Malformed(why);
    let read: Read = serde_json::from_slice(line).map_err(|error| malformed(format!("{error}")))?;
    if read.jsonrpc != "2.0" {
        return Err(malformed(format!(
            "JSON-RPC {:?}, not \"2.0\"",
            read.jsonrpc
        )));
    }
    let id = match read.id {
        None => None,
        Some(Value::Null) => Some(None),
        Some(id) => match id.as_u64().filter(|&id| id <= MAX_EXACT_INTEGER) {
            Some(id) => Some(Some(id)),
            None => {
                return Err(malformed(format!(
                    "an identifier {id}, not a whole number from 0 to 2^53 - 1"
                )));
            }
        },
    };
    let params = read.params.unwrap_or(Value::Null);
    let message = match (id, read.method, read.result, read.error) {
        (Some(Some(id)), Some(method), None, None) => Message::Request { id, method, params },
        (None, Some(method), None, None) => Message::Event { method, params },
        (Some(id), None, Some(result), None) => Message::Response {
            id,
            outcome: Ok(result),
        },
        (Some(id), None, None, Some(fault)) => Message::Response {
            id,
            outcome: Err(fault),
        },
        _ => {
            return Err(malformed(
                "neither a request, an answer with a result or an error, nor an event".to_owned(),
            ));
        }
    };
    Ok((message, read.bytes))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const WIDE: Limits = Limits {
        line: 1 << 20,
        bytes: 1 << 20,
    };

    /// Reads `bytes` given in pieces of `piece` bytes: the frames, in their order.
    fn read_in_pieces(bytes: &[u8], piece: usize) -> Vec<Frame> {
        let mut reader = Reader::new(WIDE);
        let mut frames = Vec::new();
        for chunk in bytes.chunks(piece) {
            reader.push(chunk);
            while let Some(frame) = reader.message().expect("a message") {
                frames.push(frame);
            }
        }
        frames
    }

    #[test]
    fn every_kind_of_message_comes_back_with_its_bytes_whatever_the_pieces() {
        let mut frames = [
            Frame {
                message: Message::Request {
                    id: 7,
                    method: "play/pause".to_owned(),
                    params: Value::Null,
                },
                bytes: Vec::new(),
                line: 0,
            },
            Frame {
                message: Message::Response {
                    id: Some(7),
                    outcome: Ok(json!({ "reply": "image", "width": 2, "height": 1 })),
                },
                bytes: vec![1, 2, b'\n', 4, 5, 6, 7, 8],
                line: 0,
            },
            Frame {
                message: Message::Response {
                    id: Some(8),
                    outcome: Ok(Value::Null),
                },
                bytes: Vec::new(),
                line: 0,
            },
            Frame {
                message: Message::Response {
                    id: None,
                    outcome: Err(Fault::new(Fault::PARSE, "not JSON")),
                },
                bytes: Vec::new(),
                line: 0,
            },
            Frame {
                message: Message::Event {
                    method: "play/time".to_owned(),
                    params: json!({ "paused": true }),
                },
                bytes: Vec::new(),
                line: 0,
            },
        ];
        let mut bytes = Vec::new();
        for frame in &mut frames {
            let line = encode(&frame.message, frame.bytes.len()).expect("written");
            // Its size as read: its end not counted.
            frame.line = line.len() - 1;
            bytes.extend(line);
            bytes.extend(&frame.bytes);
        }
        for piece in [1, 3, 17, bytes.len()] {
            assert_eq!(
                read_in_pieces(&bytes, piece),
                frames,
                "in pieces of {piece}"
            );
        }
    }

    #[test]
    fn a_line_is_json_rpc_s_own() {
        let line = encode(
            &Message::Request {
                id: 1,
                method: "world/entity".to_owned(),
                params: json!({ "entity": "hero" }),
            },
            0,
        )
        .expect("written");
        assert_eq!(
            String::from_utf8(line).expect("text"),
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"world/entity\",\"params\":{\"entity\":\"hero\"}}\n"
        );
        let event = encode(
            &Message::Event {
                method: "play/time".to_owned(),
                params: Value::Null,
            },
            0,
        )
        .expect("written");
        assert_eq!(
            String::from_utf8(event).expect("text"),
            "{\"jsonrpc\":\"2.0\",\"method\":\"play/time\"}\n",
            "no parameters are written for none"
        );
        let mut reader = Reader::new(WIDE);
        reader.push(b"{\"jsonrpc\":\"2.0\",\"method\":\"play/time\"}\r\n");
        assert_eq!(
            reader.message().expect("read").map(|frame| frame.message),
            Some(Message::Event {
                method: "play/time".to_owned(),
                params: Value::Null
            }),
            "a line may end in \\r\\n"
        );
    }

    #[test]
    fn what_is_not_a_message_of_the_core_is_refused() {
        for line in [
            "",
            "not json",
            "[1, 2]",
            "{\"jsonrpc\":\"1.0\",\"id\":1,\"method\":\"a/b\"}",
            "{\"id\":1,\"method\":\"a/b\"}",
            "{\"jsonrpc\":\"2.0\",\"id\":-1,\"method\":\"a/b\"}",
            "{\"jsonrpc\":\"2.0\",\"id\":1.5,\"method\":\"a/b\"}",
            "{\"jsonrpc\":\"2.0\",\"id\":\"one\",\"method\":\"a/b\"}",
            "{\"jsonrpc\":\"2.0\",\"id\":9007199254740992,\"method\":\"a/b\"}",
            "{\"jsonrpc\":\"2.0\",\"id\":1}",
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":1,\"error\":{\"code\":1,\"message\":\"\"}}",
            "{\"jsonrpc\":\"2.0\",\"id\":null,\"method\":\"a/b\"}",
            "{\"jsonrpc\":\"2.0\",\"method\":\"a/b\",\"result\":1}",
        ] {
            let mut reader = Reader::new(WIDE);
            reader.push(line.as_bytes());
            reader.push(b"\n");
            assert!(
                matches!(reader.message(), Err(AgentError::Malformed(_))),
                "{line}"
            );
        }
        let mut reader = Reader::new(WIDE);
        reader.push(b"{\"jsonrpc\":\"2.0\",\"id\":9007199254740991,\"method\":\"a/b\"}\n");
        assert!(
            matches!(reader.message(), Ok(Some(_))),
            "2^53 - 1 is the largest identifier"
        );
    }

    #[test]
    fn a_line_or_its_bytes_past_the_limits_are_refused_before_they_are_read_whole() {
        let limits = Limits { line: 64, bytes: 4 };
        let mut reader = Reader::new(limits);
        reader.push(&[b' '; 64]);
        assert!(
            matches!(reader.message(), Ok(None)),
            "64 bytes may still end"
        );
        reader.push(b" ");
        assert!(
            matches!(
                reader.message(),
                Err(AgentError::TooLarge { what: "a line", .. })
            ),
            "a line's 65th byte, its end not seen"
        );
        let mut reader = Reader::new(limits);
        reader.push(b"{\"jsonrpc\":\"2.0\",\"method\":\"a/b\",\"bytes\":5}\n");
        assert!(matches!(
            reader.message(),
            Err(AgentError::TooLarge {
                size: 5,
                limit: 4,
                ..
            })
        ));
        let mut reader = Reader::new(Limits::HELLO);
        reader.push(b"{\"jsonrpc\":\"2.0\",\"method\":\"a/b\",\"bytes\":1}\n");
        assert!(
            matches!(reader.message(), Err(AgentError::TooLarge { .. })),
            "no bytes follow a hello"
        );
    }

    #[test]
    fn a_message_waits_for_the_bytes_it_announces() {
        let mut reader = Reader::new(WIDE);
        reader.push(
            &encode(
                &Message::Event {
                    method: "a/b".to_owned(),
                    params: Value::Null,
                },
                3,
            )
            .expect("written"),
        );
        reader.push(&[1, 2]);
        assert!(matches!(reader.message(), Ok(None)));
        reader.push(&[3, 9]);
        let frame = reader.message().expect("read").expect("whole");
        assert_eq!(frame.bytes, [1, 2, 3]);
        assert!(
            matches!(reader.message(), Ok(None)),
            "the 9 waits for its line's end"
        );
    }
}
