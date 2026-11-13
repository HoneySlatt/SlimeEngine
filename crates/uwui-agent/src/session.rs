//! Both ends of a session: the program's, which answers, and the tool's, which asks.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::CORE_VERSION;
use crate::error::AgentError;
use crate::message::{Fault, Frame, Limits, MAX_EXACT_INTEGER, Message, encode};

/// The core's own method: the first message of a connection.
pub const HELLO: &str = "hello";

/// The core's own event, from the tool: it gives up the request whose number it names,
/// `{"id": 12}`. The program answers that request still, once: with its result if it had changed
/// something, with [`Fault::CANCELLED`] if it had not, and then never does it. A cancel of a
/// request already answered, or of none, is ignored
/// (`docs/adr/0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md`).
pub const CANCEL: &str = "cancel";

/// How many requests a connection may have in flight, sent and not yet answered, and how many
/// bytes of their lines: the program states it in its welcome, the tool waits for an answer
/// rather than pass it, and the program answers a request past it with [`Fault::BUSY`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InFlight {
    /// The most requests.
    pub requests: usize,
    /// The most bytes of their lines, their ends not counted ([`Frame::line`]).
    pub bytes: usize,
}

impl InFlight {
    /// 16 requests, and 1 MiB of their lines: a tool that carries an agent's calls side by side
    /// keeps a few in flight; the bytes bound what the program holds of a connection's requests
    /// while it parses them and until it answers.
    pub const DEFAULT: Self = Self {
        requests: 16,
        bytes: 1 << 20,
    };
}

/// Who or what, as it says it is: a name and a version, which nothing proves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Named {
    /// Its name, `claude-code`.
    pub name: String,
    /// Its version, as it writes it; empty when it gives none.
    pub version: String,
}

impl Named {
    /// `name`, at `version`.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }
}

impl fmt::Display for Named {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.version.is_empty() {
            f.write_str(&self.name)
        } else {
            write!(f, "{} {}", self.name, self.version)
        }
    }
}

/// A family of requests and events, `play`, and the version of it that an end speaks: a program
/// serves each family at one version, and a tool asks for each at one.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Family {
    /// Its name, which its methods begin with: `play` for `play/pause`.
    pub name: String,
    /// Its version, raised at each change of its messages that an older end would misread.
    pub version: u32,
}

impl Family {
    /// The family `name`, at `version`.
    pub fn new(name: impl Into<String>, version: u32) -> Self {
        Self {
            name: name.into(),
            version,
        }
    }
}

/// A connection's first message: the core's version, the agent as it names itself, the program
/// that carries it, the secret that the carrier asks for, the families that it speaks, and how
/// large a message it reads. Its `Debug` form hides the secret.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// [`CORE_VERSION`] of the tool.
    pub core: u32,
    /// The agent, as it names itself.
    pub agent: Named,
    /// The program that carries the agent's requests, an MCP server.
    pub client: Named,
    /// What the carrier asks of each connection, as a token; empty where it asks nothing.
    pub secret: String,
    /// The families that the tool speaks, each at its version.
    pub families: Vec<Family>,
    /// How large an answer's line, and the bytes after it, the tool reads: the program answers
    /// one larger with [`Fault::TOO_LARGE`] in its place.
    pub limits: Limits,
}

impl fmt::Debug for Hello {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hello")
            .field("core", &self.core)
            .field("agent", &self.agent)
            .field("client", &self.client)
            .field("secret", &"hidden")
            .field("families", &self.families)
            .field("limits", &self.limits)
            .finish()
    }
}

/// The program's answer to a hello it accepts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Welcome {
    /// [`CORE_VERSION`] of the program.
    pub core: u32,
    /// The program's name.
    pub program: String,
    /// The families that it serves on the connection, of those that the tool asked for.
    pub served: Vec<Family>,
    /// Those that it does not, and why.
    pub refused: Vec<Refusal>,
    /// How many requests the connection may have in flight.
    pub in_flight: InFlight,
}

/// A family that the tool asked for and the program does not serve.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    /// The family, as the tool asked for it.
    pub family: Family,
    /// Why: the program serves another version, or none.
    pub reason: String,
}

/// The family that a method names: what comes before its `/`.
fn family_of(method: &str) -> Option<(&str, &str)> {
    method
        .split_once('/')
        .filter(|(family, name)| !family.is_empty() && !name.is_empty())
}

/// The program's end of a session: it reads the hello, refuses it or welcomes it with the families
/// it serves, hands each request of a family served to the program, and writes the answers.
///
/// A connection's requests are in flight from the moment the session hands them to the program
/// until the carrier takes their answers to write them ([`ProgramSession::written`]): the program
/// may answer them in
/// any order, each once, by its number. The session refuses a request past its
/// [`InFlight`], and one whose number is in flight already.
///
/// It holds no connection: its carrier gives it each message that a [`Reader`](crate::Reader)
/// read, and sends the bytes that it gives back.
#[derive(Debug)]
pub struct ProgramSession {
    program: String,
    families: Vec<Family>,
    in_flight: InFlight,
    /// The families served on this connection, once its hello is welcomed.
    served: Option<BTreeSet<String>>,
    /// What the tool reads, as its hello said.
    reads: Limits,
    /// The requests in flight, by number: the size of each one's line.
    flying: BTreeMap<u64, usize>,
}

/// What the program's end makes of a message from the tool.
#[derive(Debug)]
pub enum FromTool {
    /// The hello, welcomed: who connected, and the answer to send.
    Welcomed {
        /// The agent, as it names itself.
        agent: Named,
        /// The program that carries it.
        client: Named,
        /// The welcome, to send.
        reply: Vec<u8>,
    },
    /// The first message refused, as not a hello, of another core, or without the carrier's
    /// secret: the answer to send, after which the connection is closed.
    Refused {
        /// Why, which the answer says too. It may quote what the tool sent: a log escapes it.
        reason: String,
        /// The refusal, to send.
        reply: Vec<u8>,
    },
    /// A request of a family served, for the program to answer ([`ProgramSession::answer`]),
    /// now in flight.
    Request {
        /// Its number, which the answer gives back.
        id: u64,
        /// Its family, `play`.
        family: String,
        /// What it asks of the family, `pause`.
        name: String,
        /// Its parameters, `Value::Null` for none.
        params: Value,
        /// The bytes that followed it.
        bytes: Vec<u8>,
    },
    /// A message that the session answers itself: a request of a family not served, past the
    /// requests in flight, of a number in flight already, or a second hello. The answer, to send.
    Answered {
        /// The answer, to send.
        reply: Vec<u8>,
    },
    /// The tool gives up the request in flight numbered `id` ([`CANCEL`]): the program answers it
    /// still, with [`Fault::CANCELLED`] if it changed nothing yet.
    Cancel {
        /// The request's number.
        id: u64,
    },
    /// A message that asks nothing of the program: a cancel of no request in flight, another
    /// event, or an answer, which a tool does not send it.
    Ignored,
}

impl ProgramSession {
    /// The end of `program`, which serves `families`, each at one version, and takes `in_flight`
    /// requests at once.
    pub fn new(program: impl Into<String>, families: Vec<Family>, in_flight: InFlight) -> Self {
        Self {
            program: program.into(),
            families,
            in_flight,
            served: None,
            reads: Limits::HELLO,
            flying: BTreeMap::new(),
        }
    }

    /// Whether the hello was welcomed.
    pub fn welcomed(&self) -> bool {
        self.served.is_some()
    }

    /// What the program makes of `frame`, the next message from the tool. `secret_is` says
    /// whether the hello's secret is the carrier's, compared as the carrier must: a token in a time
    /// that does not depend on where it differs.
    ///
    /// # Errors
    ///
    /// [`AgentError::Unwritable`] when an answer cannot be written, which no answer of the core's
    /// own is.
    pub fn receive(
        &mut self,
        frame: Frame,
        secret_is: impl FnOnce(&str) -> bool,
    ) -> Result<FromTool, AgentError> {
        let Some(served) = &self.served else {
            return self.hello(frame.message, secret_is);
        };
        let (id, method, params) = match frame.message {
            Message::Request { id, method, params } => (id, method, params),
            Message::Event { method, params } if method == CANCEL => {
                // A cancel that does not read, or names no request in flight, is ignored: it may
                // have crossed its request's answer.
                return Ok(params
                    .get("id")
                    .and_then(Value::as_u64)
                    .filter(|id| self.flying.contains_key(id))
                    .map_or(FromTool::Ignored, |id| FromTool::Cancel { id }));
            }
            _ => return Ok(FromTool::Ignored),
        };
        let fault = |code, message: String| {
            encode(
                &Message::Response {
                    id: Some(id),
                    outcome: Err(Fault::new(code, message)),
                },
                0,
            )
            .map(|reply| FromTool::Answered { reply })
        };
        if method == HELLO {
            return fault(
                Fault::INVALID,
                "a hello again: the connection is open".to_owned(),
            );
        }
        let Some((family, name)) = family_of(&method) else {
            return fault(
                Fault::NO_METHOD,
                format!("{method:?} names no family: a method is `family/name`"),
            );
        };
        if !served.contains(family) {
            return fault(
                Fault::NOT_SERVED,
                format!("the program does not serve the family {family:?} on this connection"),
            );
        }
        if self.flying.contains_key(&id) {
            return fault(
                Fault::INVALID,
                format!("the request {id} is in flight already: each in flight has its own number"),
            );
        }
        let bytes = self.flying.values().sum::<usize>();
        if self.flying.len() >= self.in_flight.requests
            || bytes.saturating_add(frame.line) > self.in_flight.bytes
        {
            return fault(
                Fault::BUSY,
                format!(
                    "{} requests in flight, of {} bytes, and the program takes {} of {}: ask again \
                     once an answer came",
                    self.flying.len(),
                    bytes,
                    self.in_flight.requests,
                    self.in_flight.bytes
                ),
            );
        }
        self.flying.insert(id, frame.line);
        Ok(FromTool::Request {
            id,
            family: family.to_owned(),
            name: name.to_owned(),
            params,
            bytes: frame.bytes,
        })
    }

    /// The answer to the request numbered `id`, its outcome, followed by `bytes` bytes: its line,
    /// and how many bytes the carrier sends after it at once, either `bytes` or none. An answer
    /// larger than the tool reads ([`Hello::limits`]) is replaced by [`Fault::TOO_LARGE`], with
    /// no bytes after it. The request stays in flight until the carrier says that it takes the
    /// answer to write it ([`ProgramSession::written`]).
    ///
    /// # Errors
    ///
    /// [`AgentError::Unwritable`] when the result cannot be written as JSON.
    pub fn answer(
        &self,
        id: u64,
        outcome: Result<Value, Fault>,
        bytes: usize,
    ) -> Result<(Vec<u8>, usize), AgentError> {
        let line = encode(
            &Message::Response {
                id: Some(id),
                outcome,
            },
            bytes,
        )?;
        // The line's end is not counted, as a reader counts it.
        let size = line.len() - 1;
        if size <= self.reads.line && bytes as u64 <= self.reads.bytes {
            return Ok((line, bytes));
        }
        let too_large = Fault::new(
            Fault::TOO_LARGE,
            format!(
                "the answer's line holds {size} bytes and {bytes} follow it, past the {} and {} \
                 that the tool reads",
                self.reads.line, self.reads.bytes
            ),
        );
        let line = encode(
            &Message::Response {
                id: Some(id),
                outcome: Err(too_large),
            },
            0,
        )?;
        Ok((line, 0))
    }

    /// The carrier takes the answer to the request numbered `id` to write it: the request is no
    /// longer in flight, and its place is free. A carrier says so before any byte of the answer
    /// can reach the tool, which frees its own place once it reads the answer and may send again
    /// at once: said after, that request would find the place still held, and be answered
    /// [`Fault::BUSY`]. A number not in flight is ignored.
    pub fn written(&mut self, id: u64) {
        self.flying.remove(&id);
    }

    /// The refusal of a connection's first message, the request numbered `id`, or none when it was
    /// not a request, for `reason` that the carrier gives, as when it welcomes as many connections
    /// as it takes: the answer to send, after which the connection is closed.
    ///
    /// # Errors
    ///
    /// As [`ProgramSession::answer`], which no refusal is.
    pub fn refuse(&self, id: Option<u64>, reason: &str) -> Result<Vec<u8>, AgentError> {
        encode(
            &Message::Response {
                id,
                outcome: Err(Fault::new(Fault::NOT_WELCOME, reason)),
            },
            0,
        )
    }

    /// The answer to a line that could not be read as a message, sent before the connection is
    /// closed: no request is known to answer, and its number is none.
    ///
    /// # Errors
    ///
    /// As [`ProgramSession::answer`].
    pub fn unreadable(&self, why: &str) -> Result<Vec<u8>, AgentError> {
        encode(
            &Message::Response {
                id: None,
                outcome: Err(Fault::new(Fault::PARSE, why)),
            },
            0,
        )
    }

    fn hello(
        &mut self,
        message: Message,
        secret_is: impl FnOnce(&str) -> bool,
    ) -> Result<FromTool, AgentError> {
        // A hello's version of the core, read before the rest: another core's hello may not read
        // as this one's, and its refusal says why.
        let mut core = None;
        let (id, hello) = match message {
            Message::Request { id, method, params } if method == HELLO => {
                core = params.get("core").and_then(Value::as_u64);
                (Some(id), serde_json::from_value::<Hello>(params))
            }
            // Answered by its number, so that the tool reads the refusal as its own.
            Message::Request { id, .. } => (Some(id), Err(serde::de::Error::custom("not a hello"))),
            _ => (None, Err(serde::de::Error::custom("not a hello"))),
        };
        let refuse = |reason: String| {
            encode(
                &Message::Response {
                    id,
                    outcome: Err(Fault::new(Fault::NOT_WELCOME, reason.clone())),
                },
                0,
            )
            .map(|reply| FromTool::Refused { reason, reply })
        };
        if let Some(core) = core.filter(|&core| core != u64::from(CORE_VERSION)) {
            return refuse(format!(
                "the tool speaks version {core} of the core, the program version {CORE_VERSION}"
            ));
        }
        let hello = match hello {
            Ok(hello) => hello,
            Err(error) => return refuse(format!("the first message is not a hello: {error}")),
        };
        if !secret_is(&hello.secret) {
            return refuse("the secret is not the program's".to_owned());
        }
        let mut served = Vec::new();
        let mut refused = Vec::new();
        for asked in hello.families {
            match self
                .families
                .iter()
                .find(|family| family.name == asked.name)
            {
                Some(family) if family.version == asked.version => served.push(asked),
                Some(family) => refused.push(Refusal {
                    reason: format!(
                        "the program serves version {} of {:?}",
                        family.version, family.name
                    ),
                    family: asked,
                }),
                None => refused.push(Refusal {
                    reason: format!("the program does not serve {:?}", asked.name),
                    family: asked,
                }),
            }
        }
        let welcome = Welcome {
            core: CORE_VERSION,
            program: self.program.clone(),
            served,
            refused,
            in_flight: self.in_flight,
        };
        let reply = encode(
            &Message::Response {
                id,
                outcome: Ok(serde_json::to_value(&welcome).map_err(AgentError::unwritable)?),
            },
            0,
        )?;
        self.served = Some(
            welcome
                .served
                .into_iter()
                .map(|family| family.name)
                .collect(),
        );
        self.reads = hello.limits;
        Ok(FromTool::Welcomed {
            agent: hello.agent,
            client: hello.client,
            reply,
        })
    }
}

/// The tool's end of a session: it writes the hello and each request, numbered, and reads the
/// program's answers and events.
///
/// Several requests may be in flight, within the [`InFlight`] that the program's welcome states:
/// the session refuses one past it ([`AgentError::Full`]), and the carrier waits for an answer to
/// send it. Answers come in any order, each matched to its request by its number.
///
/// It holds no connection: its carrier sends the bytes that it gives, and gives it each message
/// that a [`Reader`](crate::Reader) read.
#[derive(Debug, Default)]
pub struct ToolSession {
    next: u64,
    /// The requests sent and not answered yet, by number: the size of each one's line.
    waiting: BTreeMap<u64, usize>,
    /// The families served, once the program welcomed the hello.
    served: Option<BTreeSet<String>>,
    /// How many requests the program takes in flight, as its welcome said.
    in_flight: Option<InFlight>,
    /// Whether the hello was sent.
    greeted: bool,
}

/// What the tool's end makes of a message from the program.
#[derive(Debug, PartialEq)]
pub enum FromProgram {
    /// The hello welcomed.
    Welcomed(Welcome),
    /// The hello refused, and why; the program closes the connection.
    Refused(String),
    /// The answer to a request.
    Response {
        /// The request's number.
        id: u64,
        /// Its result, or why it failed.
        outcome: Result<Value, Fault>,
        /// The bytes that followed it.
        bytes: Vec<u8>,
    },
    /// An event.
    Event {
        /// What it tells, `family/name`.
        method: String,
        /// What it holds.
        params: Value,
        /// The bytes that followed it.
        bytes: Vec<u8>,
    },
    /// The program could not read a line that the tool sent, and closes the connection.
    Unreadable(Fault),
}

impl ToolSession {
    /// A session that has said nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The hello, the first message, to send.
    ///
    /// # Errors
    ///
    /// [`AgentError::OutOfTurn`] for a second hello; [`AgentError::Unwritable`] when it cannot be
    /// written.
    pub fn hello(&mut self, hello: &Hello) -> Result<Vec<u8>, AgentError> {
        if self.greeted {
            return Err(AgentError::OutOfTurn("the hello was sent already"));
        }
        let params = serde_json::to_value(hello).map_err(AgentError::unwritable)?;
        let (id, bytes) = self.numbered(HELLO, params)?;
        self.next += 1;
        self.waiting.insert(id, bytes.len() - 1);
        self.greeted = true;
        Ok(bytes)
    }

    /// The request `method`, `family/name`, with `params`, to send: its number, and its bytes.
    /// It is in flight until its answer is read.
    ///
    /// # Errors
    ///
    /// [`AgentError::OutOfTurn`] before the welcome; [`AgentError::NotServed`] for a family that
    /// the program does not serve on the connection; [`AgentError::Full`] when it would pass the
    /// requests in flight that the program takes, until an answer comes; [`AgentError::TooLarge`]
    /// for a request larger alone than their bytes; [`AgentError::Unwritable`] when the
    /// parameters cannot be written. None of them uses a number.
    pub fn request(&mut self, method: &str, params: Value) -> Result<(u64, Vec<u8>), AgentError> {
        let (Some(served), Some(in_flight)) = (&self.served, self.in_flight) else {
            return Err(AgentError::OutOfTurn(
                "a request before the program's welcome",
            ));
        };
        let family = family_of(method).map_or(method, |(family, _)| family);
        if !served.contains(family) {
            return Err(AgentError::NotServed(family.to_owned()));
        }
        let (id, bytes) = self.numbered(method, params)?;
        // The line's end is not counted, as the program's reader counts it.
        let size = bytes.len() - 1;
        if size > in_flight.bytes {
            return Err(AgentError::TooLarge {
                what: "a request's line",
                size: size as u64,
                limit: in_flight.bytes as u64,
            });
        }
        if self.waiting.len() >= in_flight.requests
            || self.waiting.values().sum::<usize>() + size > in_flight.bytes
        {
            return Err(AgentError::Full);
        }
        self.next += 1;
        self.waiting.insert(id, size);
        Ok((id, bytes))
    }

    /// The next number and the line of the request `method` with `params`, the number not yet
    /// used.
    fn numbered(&self, method: &str, params: Value) -> Result<(u64, Vec<u8>), AgentError> {
        let id = self.next;
        if id > MAX_EXACT_INTEGER {
            return Err(AgentError::OutOfTurn("every number of a request was used"));
        }
        let bytes = encode(
            &Message::Request {
                id,
                method: method.to_owned(),
                params,
            },
            0,
        )?;
        Ok((id, bytes))
    }

    /// The cancel of the request numbered `id` ([`CANCEL`]), to send, or `None` when no such
    /// request waits for its answer. The request stays in flight until its answer comes, which it
    /// still does.
    ///
    /// # Errors
    ///
    /// [`AgentError::Unwritable`], which no cancel is.
    pub fn cancel(&self, id: u64) -> Result<Option<Vec<u8>>, AgentError> {
        if !self.waiting.contains_key(&id) {
            return Ok(None);
        }
        encode(
            &Message::Event {
                method: CANCEL.to_owned(),
                params: serde_json::json!({ "id": id }),
            },
            0,
        )
        .map(Some)
    }

    /// What the tool makes of `frame`, the next message from the program.
    ///
    /// # Errors
    ///
    /// [`AgentError::Malformed`] for an answer to no request waiting, for a request from the
    /// program, which the core does not have yet, and for a welcome that does not read as one.
    pub fn receive(&mut self, frame: Frame) -> Result<FromProgram, AgentError> {
        match frame.message {
            Message::Response { id: None, outcome } => Ok(FromProgram::Unreadable(
                outcome
                    .err()
                    .unwrap_or_else(|| Fault::new(Fault::PARSE, "unreadable")),
            )),
            Message::Response {
                id: Some(id),
                outcome,
            } => {
                if self.waiting.remove(&id).is_none() {
                    return Err(AgentError::Malformed(format!(
                        "an answer to the request {id}, which waits for none"
                    )));
                }
                if self.served.is_some() {
                    return Ok(FromProgram::Response {
                        id,
                        outcome,
                        bytes: frame.bytes,
                    });
                }
                match outcome {
                    Ok(welcome) => {
                        let welcome: Welcome =
                            serde_json::from_value(welcome).map_err(|error| {
                                AgentError::Malformed(format!(
                                    "a welcome that does not read: {error}"
                                ))
                            })?;
                        self.served = Some(
                            welcome
                                .served
                                .iter()
                                .map(|family| family.name.clone())
                                .collect(),
                        );
                        self.in_flight = Some(welcome.in_flight);
                        Ok(FromProgram::Welcomed(welcome))
                    }
                    Err(fault) => Ok(FromProgram::Refused(fault.message)),
                }
            }
            Message::Event { method, params } => Ok(FromProgram::Event {
                method,
                params,
                bytes: frame.bytes,
            }),
            Message::Request { method, .. } => Err(AgentError::Malformed(format!(
                "a request from the program, {method:?}, which the core does not take"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::message::{Limits, Reader};

    /// Reads the one message that `bytes` hold.
    fn frame(bytes: &[u8]) -> Frame {
        let mut reader = Reader::new(Limits {
            line: 1 << 20,
            bytes: 1 << 20,
        });
        reader.push(bytes);
        let frame = reader.message().expect("a message").expect("whole");
        assert!(reader.message().expect("nothing more").is_none());
        frame
    }

    fn hello(families: &[(&str, u32)]) -> Hello {
        Hello {
            core: CORE_VERSION,
            agent: Named::new("an agent", "1.0"),
            client: Named::new("a server", ""),
            secret: "open sesame".to_owned(),
            families: families
                .iter()
                .map(|&(name, version)| Family::new(name, version))
                .collect(),
            limits: Limits {
                line: 1 << 20,
                bytes: 1 << 20,
            },
        }
    }

    fn program() -> ProgramSession {
        ProgramSession::new(
            "a game",
            vec![Family::new("play", 1), Family::new("world", 2)],
            InFlight::DEFAULT,
        )
    }

    fn the_secret(secret: &str) -> bool {
        secret == "open sesame"
    }

    /// Opens a session between both ends, the tool asking for `families`: the welcome.
    fn open(
        tool: &mut ToolSession,
        program: &mut ProgramSession,
        families: &[(&str, u32)],
    ) -> Welcome {
        let sent = tool.hello(&hello(families)).expect("written");
        let FromTool::Welcomed {
            agent,
            client,
            reply,
        } = program.receive(frame(&sent), the_secret).expect("read")
        else {
            panic!("welcomed");
        };
        assert_eq!(
            (agent.to_string(), client.to_string()),
            ("an agent 1.0".to_owned(), "a server".to_owned())
        );
        match tool.receive(frame(&reply)).expect("read") {
            FromProgram::Welcomed(welcome) => welcome,
            other => panic!("welcomed, not {other:?}"),
        }
    }

    #[test]
    fn a_family_of_another_version_is_refused_alone_and_the_rest_served() {
        let (mut tool, mut program) = (ToolSession::new(), program());
        let welcome = open(
            &mut tool,
            &mut program,
            &[("play", 1), ("world", 1), ("music", 3)],
        );
        assert_eq!(welcome.program, "a game");
        assert_eq!(welcome.served, [Family::new("play", 1)]);
        assert_eq!(
            welcome.refused,
            [
                Refusal {
                    family: Family::new("world", 1),
                    reason: "the program serves version 2 of \"world\"".to_owned()
                },
                Refusal {
                    family: Family::new("music", 3),
                    reason: "the program does not serve \"music\"".to_owned()
                },
            ]
        );
        let (id, sent) = tool.request("play/pause", Value::Null).expect("served");
        let FromTool::Request {
            id: asked,
            family,
            name,
            params,
            ..
        } = program.receive(frame(&sent), the_secret).expect("read")
        else {
            panic!("a request for the program");
        };
        assert_eq!(
            (asked, family.as_str(), name.as_str(), params),
            (id, "play", "pause", Value::Null)
        );
        let (answer, follow) = program
            .answer(id, Ok(json!({ "reply": "done" })), 3)
            .expect("written");
        assert_eq!(follow, 3, "the bytes follow the answer");
        let mut answered = answer.clone();
        answered.extend([1, 2, 3]);
        assert_eq!(
            tool.receive(frame(&answered)).expect("read"),
            FromProgram::Response {
                id,
                outcome: Ok(json!({ "reply": "done" })),
                bytes: vec![1, 2, 3]
            }
        );
        assert!(matches!(
            tool.request("world/entities", Value::Null),
            Err(AgentError::NotServed(family)) if family == "world"
        ));
    }

    #[test]
    fn the_program_refuses_a_family_that_it_does_not_serve_on_the_connection() {
        let (mut tool, mut program) = (ToolSession::new(), program());
        open(&mut tool, &mut program, &[("play", 1)]);
        for (method, code) in [
            ("world/entities", Fault::NOT_SERVED),
            ("pause", Fault::NO_METHOD),
            ("play/", Fault::NO_METHOD),
            (HELLO, Fault::INVALID),
        ] {
            let sent = encode(
                &Message::Request {
                    id: 41,
                    method: method.to_owned(),
                    params: Value::Null,
                },
                0,
            )
            .expect("written");
            let FromTool::Answered { reply } =
                program.receive(frame(&sent), the_secret).expect("read")
            else {
                panic!("{method}: answered by the session");
            };
            let Message::Response { id, outcome } = frame(&reply).message else {
                panic!("an answer");
            };
            assert_eq!(id, Some(41));
            assert_eq!(outcome.map_err(|fault| fault.code), Err(code), "{method}");
        }
    }

    #[test]
    fn a_hello_of_another_core_without_the_secret_or_none_is_refused() {
        let refused = |sent: Vec<u8>, why: &str| {
            let mut program = program();
            let FromTool::Refused { reason, reply } =
                program.receive(frame(&sent), the_secret).expect("read")
            else {
                panic!("refused");
            };
            assert!(reason.contains(why), "{reason}");
            assert!(!program.welcomed());
            let mut tool = ToolSession::new();
            tool.hello(&hello(&[])).expect("written");
            assert_eq!(
                tool.receive(frame(&reply)).expect("read"),
                FromProgram::Refused(reason)
            );
        };
        let mut other_core = hello(&[]);
        other_core.core = CORE_VERSION + 1;
        refused(
            ToolSession::new().hello(&other_core).expect("written"),
            "version",
        );
        // A hello of the core before this one, which has no limits, is refused for its version.
        let mut before = serde_json::to_value(hello(&[])).expect("written");
        before["core"] = json!(CORE_VERSION - 1);
        before.as_object_mut().expect("an object").remove("limits");
        let before = encode(
            &Message::Request {
                id: 0,
                method: HELLO.to_owned(),
                params: before,
            },
            0,
        )
        .expect("written");
        refused(before, "version");
        let mut no_secret = hello(&[]);
        no_secret.secret = "guess".to_owned();
        refused(
            ToolSession::new().hello(&no_secret).expect("written"),
            "secret",
        );
        let request = encode(
            &Message::Request {
                id: 0,
                method: "play/pause".to_owned(),
                params: Value::Null,
            },
            0,
        )
        .expect("written");
        refused(request, "not a hello");
        let bad = encode(
            &Message::Request {
                id: 0,
                method: HELLO.to_owned(),
                params: json!({ "core": CORE_VERSION }),
            },
            0,
        )
        .expect("written");
        refused(bad, "not a hello");
    }

    #[test]
    fn the_tool_refuses_what_answers_no_request_and_asks_nothing_out_of_its_turn() {
        let mut tool = ToolSession::new();
        assert!(matches!(
            tool.request("play/pause", Value::Null),
            Err(AgentError::OutOfTurn(_))
        ));
        let mut program = program();
        open(&mut tool, &mut program, &[("play", 1)]);
        assert!(matches!(
            tool.hello(&hello(&[])),
            Err(AgentError::OutOfTurn(_))
        ));
        let (stray, _) = program.answer(99, Ok(Value::Null), 0).expect("written");
        assert!(matches!(
            tool.receive(frame(&stray)),
            Err(AgentError::Malformed(_))
        ));
        let (id, _) = tool.request("play/state", Value::Null).expect("served");
        let (answer, _) = program.answer(id, Ok(Value::Null), 0).expect("written");
        tool.receive(frame(&answer)).expect("answered once");
        assert!(
            matches!(tool.receive(frame(&answer)), Err(AgentError::Malformed(_))),
            "a request is answered once"
        );
        let unreadable = program.unreadable("not JSON").expect("written");
        assert_eq!(
            tool.receive(frame(&unreadable)).expect("read"),
            FromProgram::Unreadable(Fault::new(Fault::PARSE, "not JSON"))
        );
    }

    /// The program's end taking `in_flight`, opened by a tool that asks for `play`.
    fn opened(in_flight: InFlight) -> (ToolSession, ProgramSession, Welcome) {
        let mut tool = ToolSession::new();
        let mut program = ProgramSession::new("a game", vec![Family::new("play", 1)], in_flight);
        let welcome = open(&mut tool, &mut program, &[("play", 1)]);
        (tool, program, welcome)
    }

    /// The line of the request `id` to `play/state`, written as a tool that keeps no count would.
    fn raw_request(id: u64) -> Vec<u8> {
        encode(
            &Message::Request {
                id,
                method: "play/state".to_owned(),
                params: Value::Null,
            },
            0,
        )
        .expect("written")
    }

    /// The fault's code of `answer`, a reply that the session wrote itself.
    fn fault_code(answer: FromTool) -> i64 {
        let FromTool::Answered { reply } = answer else {
            panic!("answered by the session, not {answer:?}");
        };
        match frame(&reply).message {
            Message::Response {
                outcome: Err(fault),
                ..
            } => fault.code,
            other => panic!("a fault, not {other:?}"),
        }
    }

    #[test]
    fn requests_in_flight_are_answered_in_any_order_each_once_within_the_program_s_bounds() {
        let (mut tool, mut program, welcome) = opened(InFlight {
            requests: 2,
            bytes: 1 << 20,
        });
        assert_eq!(
            welcome.in_flight.requests, 2,
            "the welcome states its bounds"
        );
        let (first, sent_first) = tool.request("play/state", Value::Null).expect("in flight");
        let (second, sent_second) = tool.request("play/pause", Value::Null).expect("in flight");
        assert!(
            matches!(
                tool.request("play/stop", Value::Null),
                Err(AgentError::Full)
            ),
            "the tool holds back a third"
        );
        for sent in [&sent_first, &sent_second] {
            assert!(matches!(
                program.receive(frame(sent), the_secret).expect("read"),
                FromTool::Request { .. }
            ));
        }
        assert_eq!(program.flying.len(), 2);
        // The second answered first.
        for id in [second, first] {
            let (answer, _) = program.answer(id, Ok(json!(id)), 0).expect("written");
            program.written(id);
            assert_eq!(
                tool.receive(frame(&answer)).expect("its own answer"),
                FromProgram::Response {
                    id,
                    outcome: Ok(json!(id)),
                    bytes: Vec::new()
                }
            );
        }
        assert_eq!(program.flying.len(), 0);
        let (third, _) = tool.request("play/stop", Value::Null).expect("room again");
        assert_eq!(third, second + 1, "a request held back used no number");
    }

    #[test]
    fn the_program_answers_busy_past_its_bounds_and_frees_a_place_once_its_answer_is_written() {
        let (_, mut program, _) = opened(InFlight {
            requests: 2,
            bytes: 1 << 20,
        });
        for id in [10, 11] {
            assert!(matches!(
                program.receive(frame(&raw_request(id)), the_secret),
                Ok(FromTool::Request { .. })
            ));
        }
        let busy = program.receive(frame(&raw_request(12)), the_secret);
        assert_eq!(fault_code(busy.expect("read")), Fault::BUSY);
        // Answered, not yet written: still in flight.
        program.answer(10, Ok(Value::Null), 0).expect("written");
        let busy = program.receive(frame(&raw_request(13)), the_secret);
        assert_eq!(fault_code(busy.expect("read")), Fault::BUSY);
        program.written(10);
        assert!(matches!(
            program.receive(frame(&raw_request(14)), the_secret),
            Ok(FromTool::Request { id: 14, .. })
        ));
        // The bytes too: a line fits, and a second one passes them.
        let line = raw_request(20).len() - 1;
        let (mut tool, mut program, _) = opened(InFlight {
            requests: 16,
            bytes: line + line / 2,
        });
        // The tool holds back what would pass them too, whose line is as long, and refuses one
        // larger alone than their bytes, using no number.
        assert!(matches!(
            tool.request("play/state", json!("x".repeat(line))),
            Err(AgentError::TooLarge { .. })
        ));
        tool.request("play/state", Value::Null).expect("in flight");
        assert!(matches!(
            tool.request("play/state", Value::Null),
            Err(AgentError::Full)
        ));
        assert!(matches!(
            program.receive(frame(&raw_request(20)), the_secret),
            Ok(FromTool::Request { .. })
        ));
        let busy = program.receive(frame(&raw_request(21)), the_secret);
        assert_eq!(fault_code(busy.expect("read")), Fault::BUSY);
    }

    #[test]
    fn a_number_already_in_flight_is_refused() {
        let (_, mut program, _) = opened(InFlight::DEFAULT);
        assert!(matches!(
            program.receive(frame(&raw_request(5)), the_secret),
            Ok(FromTool::Request { .. })
        ));
        let again = program.receive(frame(&raw_request(5)), the_secret);
        assert_eq!(fault_code(again.expect("read")), Fault::INVALID);
        assert_eq!(program.flying.len(), 1, "the refused one is not in flight");
    }

    #[test]
    fn a_cancel_names_a_request_in_flight_and_is_ignored_otherwise() {
        let (mut tool, mut program, _) = opened(InFlight::DEFAULT);
        let (id, sent) = tool.request("play/state", Value::Null).expect("in flight");
        program.receive(frame(&sent), the_secret).expect("read");
        let cancel = tool.cancel(id).expect("written").expect("in flight");
        assert!(matches!(
            program.receive(frame(&cancel), the_secret),
            Ok(FromTool::Cancel { id: cancelled }) if cancelled == id
        ));
        assert!(
            tool.cancel(id + 1).expect("written").is_none(),
            "no cancel of what was not asked"
        );
        // One that crossed its request's answer, or that does not read.
        let (answer, _) = program
            .answer(id, Err(Fault::new(Fault::CANCELLED, "cancelled")), 0)
            .expect("written");
        program.written(id);
        assert!(matches!(
            program.receive(frame(&cancel), the_secret),
            Ok(FromTool::Ignored)
        ));
        let unreadable = encode(
            &Message::Event {
                method: CANCEL.to_owned(),
                params: json!({ "id": "twelve" }),
            },
            0,
        )
        .expect("written");
        assert!(matches!(
            program.receive(frame(&unreadable), the_secret),
            Ok(FromTool::Ignored)
        ));
        assert!(matches!(
            tool.receive(frame(&answer)).expect("its answer"),
            FromProgram::Response {
                outcome: Err(Fault {
                    code: Fault::CANCELLED,
                    ..
                }),
                ..
            }
        ));
    }

    #[test]
    fn an_answer_past_what_the_tool_reads_becomes_a_fault_without_its_bytes() {
        let mut tool = ToolSession::new();
        let mut program = program();
        let mut asked = hello(&[("play", 1)]);
        asked.limits = Limits {
            line: 200,
            bytes: 10,
        };
        let sent = tool.hello(&asked).expect("written");
        let FromTool::Welcomed { reply, .. } =
            program.receive(frame(&sent), the_secret).expect("read")
        else {
            panic!("welcomed");
        };
        tool.receive(frame(&reply)).expect("welcomed");
        let too_large = |(mut line, follow): (Vec<u8>, usize)| {
            line.resize(line.len() + follow, 0);
            match frame(&line).message {
                Message::Response {
                    outcome: Err(fault),
                    ..
                } => fault.code == Fault::TOO_LARGE && follow == 0,
                _ => false,
            }
        };
        let long = json!("x".repeat(300));
        assert!(too_large(program.answer(1, Ok(long), 0).expect("written")));
        assert!(too_large(
            program.answer(2, Ok(Value::Null), 11).expect("written")
        ));
        let within = program.answer(3, Ok(Value::Null), 10).expect("written");
        assert_eq!(within.1, 10, "its bytes follow it");
        assert!(!too_large(within), "within what it reads");
    }

    #[test]
    fn a_hello_hides_its_secret() {
        let shown = format!("{:?}", hello(&[]));
        assert!(!shown.contains("sesame"), "{shown}");
        assert!(shown.contains("hidden"), "{shown}");
    }
}
