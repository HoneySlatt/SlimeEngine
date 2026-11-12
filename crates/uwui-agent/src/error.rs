//! Errors of the core's session.

/// Why bytes could not be read as a message, or a message could not be written or sent.
///
/// [`AgentError::Malformed`] and [`AgentError::TooLarge`] lose the connection's framing: what
/// follows cannot be found, and the connection is closed. The others leave it usable.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AgentError {
    /// A line that is not a message of JSON-RPC 2.0 as the core speaks it: not JSON, not an
    /// object, without `"jsonrpc": "2.0"`, with an identifier that is not a whole number from 0 to
    /// 2^53 - 1, an answer with both a result and an error or neither, or an answer to no request.
    #[error("not a message of the core for agents: {0}")]
    Malformed(String),
    /// A message's line, or the bytes that it announces, larger than the connection's
    /// [`Limits`](crate::Limits) allow: refused before it is read whole.
    #[error("{what} of {size} bytes, past the {limit} that the connection allows")]
    TooLarge {
        /// What is too large: a line, or the bytes that follow one.
        what: &'static str,
        /// Its size, or as much of it as was read when it passed the limit.
        size: u64,
        /// The limit.
        limit: u64,
    },
    /// A request of a family that the program does not serve on this connection, refused by the
    /// tool's end before it is sent.
    #[error("the program does not serve the family {0} on this connection")]
    NotServed(String),
    /// A request before the program's welcome, or a second hello.
    #[error("{0}")]
    OutOfTurn(&'static str),
    /// A request of a family's enum that is not written as one, an object of a `method` and its
    /// `params` alone ([`call`](crate::call)).
    #[error("a request that is not written as its method and its parameters")]
    NotACall,
    /// A value that JSON cannot write, as a map whose keys are not text: the writer's error as its
    /// source.
    #[error("a message could not be written")]
    Unwritable(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl AgentError {
    /// The error of a value that JSON could not write.
    pub(crate) fn unwritable(error: serde_json::Error) -> Self {
        Self::Unwritable(Box::new(error))
    }
}
