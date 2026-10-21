//! The secret that a game asks of each connection to its channel.

use std::fmt;
use std::str::FromStr;

use crate::error::ChannelError;

/// The secret of a game's development channel: 128 bits drawn from the system's randomness by
/// whoever starts the game, given to it in its environment ([`TOKEN_ENV_VAR`](crate::TOKEN_ENV_VAR)),
/// and asked of each connection (`docs/adr/0110-how-the-editor-runs-a-games-code.md`). It is
/// written as 32 lowercase hexadecimal digits. Its `Debug` form hides it, so that a log never
/// holds it; its `Display` form is the text that the environment carries.
#[derive(Clone)]
pub struct Token([u8; 16]);

impl Token {
    /// A token of 128 bits drawn from the system's randomness.
    ///
    /// # Errors
    ///
    /// [`ChannelError::Randomness`] when the system gives no random bits.
    pub fn generate() -> Result<Self, ChannelError> {
        let mut bits = [0; 16];
        getrandom::fill(&mut bits).map_err(ChannelError::Randomness)?;
        Ok(Self(bits))
    }

    /// Whether `text` is this token, compared in a time that does not depend on where they
    /// differ, so that a connection cannot learn the token a digit at a time.
    pub fn matches(&self, text: &str) -> bool {
        let Ok(other) = text.parse::<Token>() else {
            return false;
        };
        self.0
            .iter()
            .zip(other.0)
            .fold(0, |differ, (a, b)| differ | (a ^ b))
            == 0
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(hidden)")
    }
}

impl FromStr for Token {
    type Err = ChannelError;

    /// The token that `text`, 32 hexadecimal digits, writes.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let malformed = || ChannelError::MalformedToken;
        if text.len() != 32 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(malformed());
        }
        let mut bits = [0; 16];
        for (byte, pair) in bits.iter_mut().zip(text.as_bytes().chunks(2)) {
            let digits = std::str::from_utf8(pair).map_err(|_| malformed())?;
            *byte = u8::from_str_radix(digits, 16).map_err(|_| malformed())?;
        }
        Ok(Self(bits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_written_as_hexadecimal_and_read_back() {
        let token = Token::generate().expect("random bits");
        let text = token.to_string();
        assert_eq!(text.len(), 32);
        assert!(token.matches(&text));
        assert!(token.matches(&text.to_uppercase()), "digits of either case");
        assert_eq!(format!("{token:?}"), "Token(hidden)");
        let other = Token::generate().expect("random bits");
        assert!(!token.matches(&other.to_string()));
        for wrong in [
            "",
            "00",
            &text[..31],
            &format!("{text}0"),
            &"g".repeat(32),
            &"+0".repeat(16),
        ] {
            assert!(!token.matches(wrong), "{wrong}");
        }
    }
}
