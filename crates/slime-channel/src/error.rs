//! Errors of the development channel.

/// Why a connection, a message or a launch failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ChannelError {
    /// Reading from or writing to the connection, or starting the game, failed.
    #[error("the development channel failed")]
    Io(#[from] std::io::Error),
    /// The game refused the connection: another token, or another version of the core.
    #[error("the game refused the connection: {0}")]
    Refused(String),
    /// An answer that the protocol does not hold: a reply of the channel that does not read, an
    /// event or answer where none is awaited, or the game's word that it could not read what the
    /// tool sent, after which it closes the connection.
    #[error("not a message of the development channel: {0}")]
    Malformed(String),
    /// The core's session refused what came, or what was to be sent: a line that is not a message,
    /// larger than the connection allows, or a request of a family that the game does not serve.
    #[error("the session of the development channel failed")]
    Session(#[from] uwui_agent::AgentError),
    /// A token that is not 32 hexadecimal digits.
    #[error("a token of the development channel is 32 hexadecimal digits")]
    MalformedToken,
    /// The system gave no random bits to draw a token from.
    #[error("the system gives no random bits")]
    Randomness(#[source] getrandom::Error),
    /// The connection broke, or the game sent what is not a message of the protocol, at an earlier
    /// request: the connection is given up.
    #[error("the connection to the game broke at an earlier request: open another")]
    Broken,
    /// The request was cancelled before the game did it, and the game did not do it.
    #[error("the request was cancelled before the game did it")]
    Cancelled,
    /// The game started did not say where it listens within the time given, or ended first.
    #[error("the game started does not listen on its development channel: {0}")]
    NotListening(String),
}
