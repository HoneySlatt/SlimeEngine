//! Error messages with their causes, for the examples' logs.

/// The error's message followed by the messages of its sources: "error: cause: cause of cause".
pub fn error_chain(error: &dyn std::error::Error) -> String {
    let mut chain = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        chain.push_str(": ");
        chain.push_str(&cause.to_string());
        source = cause.source();
    }
    chain
}
