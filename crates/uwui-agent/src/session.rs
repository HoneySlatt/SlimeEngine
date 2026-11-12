//! Both ends of a session: the program's, which answers, and the tool's, which asks.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::CORE_VERSION;
use crate::error::AgentError;
use crate::message::{Fault, Frame, MAX_EXACT_INTEGER, Message, encode};

/// The core's own method: the first message of a connection.
pub const HELLO: &str = "hello";

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
/// that carries it, the secret that the carrier asks for, and the families that it speaks. Its
/// `Debug` form hides the secret.
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
}

impl fmt::Debug for Hello {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hello")
            .field("core", &self.core)
            .field("agent", &self.agent)
            .field("client", &self.client)
            .field("secret", &"hidden")
            .field("families", &self.families)
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
/// It holds no connection: its carrier gives it each message that a [`Reader`](crate::Reader)
/// read, and sends the bytes that it gives back.
#[derive(Debug)]
pub struct ProgramSession {
    program: String,
    families: Vec<Family>,
    /// The families served on this connection, once its hello is welcomed.
    served: Option<BTreeSet<String>>,
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
    /// A request of a family served, for the program to answer ([`ProgramSession::answer`]).
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
    /// A message that the session answers itself, a request of a family not served or a second
    /// hello: the answer, to send.
    Answered {
        /// The answer, to send.
        reply: Vec<u8>,
    },
    /// A message that asks nothing of the program: an event or an answer, which a tool does not
    /// send it yet.
    Ignored,
}

impl ProgramSession {
    /// The end of `program`, which serves `families`, each at one version.
    pub fn new(program: impl Into<String>, families: Vec<Family>) -> Self {
        Self {
            program: program.into(),
            families,
            served: None,
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
        let Message::Request { id, method, params } = frame.message else {
            return Ok(FromTool::Ignored);
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
        Ok(FromTool::Request {
            id,
            family: family.to_owned(),
            name: name.to_owned(),
            params,
            bytes: frame.bytes,
        })
    }

    /// The answer to the request numbered `id`: its outcome, and the count of the bytes that the
    /// carrier sends after it at once.
    ///
    /// # Errors
    ///
    /// [`AgentError::Unwritable`] when the result cannot be written as JSON.
    pub fn answer(
        &self,
        id: u64,
        outcome: Result<Value, Fault>,
        bytes: usize,
    ) -> Result<Vec<u8>, AgentError> {
        encode(
            &Message::Response {
                id: Some(id),
                outcome,
            },
            bytes,
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
        let (id, hello) = match message {
            Message::Request { id, method, params } if method == HELLO => {
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
        let hello = match hello {
            Ok(hello) => hello,
            Err(error) => return refuse(format!("the first message is not a hello: {error}")),
        };
        if hello.core != CORE_VERSION {
            return refuse(format!(
                "the tool speaks version {} of the core, the program version {CORE_VERSION}",
                hello.core
            ));
        }
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
/// It holds no connection: its carrier sends the bytes that it gives, and gives it each message
/// that a [`Reader`](crate::Reader) read.
#[derive(Debug, Default)]
pub struct ToolSession {
    next: u64,
    /// The requests sent and not answered yet, by number.
    waiting: BTreeSet<u64>,
    /// The families served, once the program welcomed the hello.
    served: Option<BTreeSet<String>>,
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
        let (_, bytes) = self.numbered(HELLO, params)?;
        self.greeted = true;
        Ok(bytes)
    }

    /// The request `method`, `family/name`, with `params`, to send: its number, and its bytes.
    ///
    /// # Errors
    ///
    /// [`AgentError::OutOfTurn`] before the welcome; [`AgentError::NotServed`] for a family that
    /// the program does not serve on the connection; [`AgentError::Unwritable`] when the
    /// parameters cannot be written.
    pub fn request(&mut self, method: &str, params: Value) -> Result<(u64, Vec<u8>), AgentError> {
        let Some(served) = &self.served else {
            return Err(AgentError::OutOfTurn(
                "a request before the program's welcome",
            ));
        };
        let family = family_of(method).map_or(method, |(family, _)| family);
        if !served.contains(family) {
            return Err(AgentError::NotServed(family.to_owned()));
        }
        self.numbered(method, params)
    }

    fn numbered(&mut self, method: &str, params: Value) -> Result<(u64, Vec<u8>), AgentError> {
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
        self.next += 1;
        self.waiting.insert(id);
        Ok((id, bytes))
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
                if !self.waiting.remove(&id) {
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
        }
    }

    fn program() -> ProgramSession {
        ProgramSession::new(
            "a game",
            vec![Family::new("play", 1), Family::new("world", 2)],
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
        let answer = program
            .answer(id, Ok(json!({ "reply": "done" })), 3)
            .expect("written");
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
                params: json!({ "core": 1 }),
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
        let stray = program.answer(99, Ok(Value::Null), 0).expect("written");
        assert!(matches!(
            tool.receive(frame(&stray)),
            Err(AgentError::Malformed(_))
        ));
        let (id, _) = tool.request("play/state", Value::Null).expect("served");
        let answer = program.answer(id, Ok(Value::Null), 0).expect("written");
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

    #[test]
    fn a_hello_hides_its_secret() {
        let shown = format!("{:?}", hello(&[]));
        assert!(!shown.contains("sesame"), "{shown}");
        assert!(shown.contains("hidden"), "{shown}");
    }
}
