//! Errors of the development channel.

/// Why a connection, a message or a launch failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ChannelError {
    /// Reading from or writing to the connection, or starting the game, failed.
    #[error("the development channel failed")]
    Io(#[from] std::io::Error),
    /// The game refused the connection: another token, or another version of the protocol.
    #[error("the game refused the connection: {0}")]
    Refused(String),
    /// A message that the protocol does not hold: not in the binary encoding, not a message of the
    /// protocol, larger than the protocol allows, or cut short.
    #[error("not a message of the development channel: {0}")]
    Malformed(String),
    /// A token that is not 32 hexadecimal digits.
    #[error("a token of the development channel is 32 hexadecimal digits")]
    MalformedToken,
    /// The system gave no random bits to draw a token from.
    #[error("the system gives no random bits")]
    Randomness(#[source] getrandom::Error),
    /// The connection broke, or a reply did not come in time, at an earlier request: the replies
    /// that follow could answer other requests, so the connection is given up.
    #[error("the connection to the game broke at an earlier request: open another")]
    Broken,
    /// The game started did not say where it listens within the time given, or ended first.
    #[error("the game started does not listen on its development channel: {0}")]
    NotListening(String),
}
