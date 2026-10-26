//! The system's clipboard, of text.

use crate::PlatformError;

/// The system's clipboard, of text: what a copy writes and a paste reads, shared with the other
/// programs. On Linux, it speaks the Wayland protocol of the compositors that give it, wlroots' and
/// niri's and Hyprland's among them, and X11's otherwise, through XWayland where a compositor runs
/// one; on Windows, the system's own.
pub struct Clipboard {
    inner: arboard::Clipboard,
}

impl std::fmt::Debug for Clipboard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Clipboard")
    }
}

impl Clipboard {
    /// The system's clipboard.
    ///
    /// # Errors
    ///
    /// [`PlatformError::Clipboard`] when the system gives none: on Linux, neither a compositor
    /// that gives Wayland's protocol of the clipboard nor an X server.
    pub fn open() -> Result<Self, PlatformError> {
        let inner =
            arboard::Clipboard::new().map_err(|error| PlatformError::Clipboard(Box::new(error)))?;
        Ok(Self { inner })
    }

    /// The text that the clipboard holds; none when it holds no text, or cannot be read, which the
    /// log tells.
    pub fn text(&mut self) -> Option<String> {
        match self.inner.get_text() {
            Ok(text) => Some(text),
            Err(arboard::Error::ContentNotAvailable) => None,
            Err(error) => {
                tracing::warn!(%error, "the clipboard cannot be read");
                None
            }
        }
    }

    /// Puts `text` in the clipboard, in place of what it held; a failure is told by the log.
    pub fn set_text(&mut self, text: &str) {
        if let Err(error) = self.inner.set_text(text) {
            tracing::warn!(%error, "the clipboard cannot be written");
        }
    }
}
