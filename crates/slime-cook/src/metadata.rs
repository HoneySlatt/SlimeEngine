//! Metadata files: the `<file>.meta` beside each source file, which holds its asset ID.
//!
//! The format is a subset of TOML with one key, so that it can grow into TOML with import settings
//! once the engine has a serialization format (Stage 5): a line `id = "<uuid>"`, with blank lines
//! and comment lines, which start with `#`, around it, and a comment after it. Lines may end with
//! CR LF, as a checkout on Windows may give them.

use slime_assets::AssetId;

/// Why a metadata file cannot be read. Lines are counted from 1.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MetadataError {
    /// A line that is neither blank, a comment, nor `id = "<uuid>"`.
    #[error("line {line}: expected `id = \"<uuid>\"`, the only key of a metadata file")]
    Unexpected {
        /// The line.
        line: usize,
    },
    /// A quoted value that is not a UUID.
    #[error("line {line}: `{text}` is not an asset ID, a UUID")]
    NotAnId {
        /// The line.
        line: usize,
        /// The text between the quotes.
        text: String,
    },
    /// A second `id` line.
    #[error("line {line}: a second `id`")]
    SecondId {
        /// The line of the second.
        line: usize,
    },
    /// No `id` line.
    #[error("no `id`")]
    NoId,
}

/// The asset ID that the text of a metadata file holds.
///
/// # Errors
///
/// [`MetadataError`] if the text is not in the format the module describes, with its line.
///
/// ```
/// use slime_assets::AssetId;
/// use slime_cook::metadata;
///
/// let text = "# The cooker gave this ID.\nid = \"6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c\"\n";
/// assert_eq!(
///     metadata::read(text),
///     Ok(AssetId::from_u128(0x6f1c2b3a_0d4e_4f5a_8b6c_7d8e9f0a1b2c))
/// );
/// assert_eq!(metadata::read(&metadata::write(AssetId::from_u128(7))), Ok(AssetId::from_u128(7)));
/// ```
pub fn read(text: &str) -> Result<AssetId, MetadataError> {
    let mut found = None;
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let unexpected = MetadataError::Unexpected { line: line_number };
        let value = line
            .strip_prefix("id")
            .map(str::trim_start)
            .and_then(|rest| rest.strip_prefix('='))
            .map(str::trim_start)
            .and_then(|rest| rest.strip_prefix('"'))
            .ok_or(unexpected.clone())?;
        let (quoted, after) = value.split_once('"').ok_or(unexpected.clone())?;
        let after = after.trim_start();
        if !(after.is_empty() || after.starts_with('#')) {
            return Err(unexpected);
        }
        let id = quoted.parse().map_err(|_| MetadataError::NotAnId {
            line: line_number,
            text: quoted.to_owned(),
        })?;
        if found.is_some() {
            return Err(MetadataError::SecondId { line: line_number });
        }
        found = Some(id);
    }
    found.ok_or(MetadataError::NoId)
}

/// The text of the metadata file that holds `id`, which [`read`] reads back.
pub fn write(id: AssetId) -> String {
    format!("id = \"{id}\"\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: AssetId = AssetId::from_u128(0x6f1c2b3a_0d4e_4f5a_8b6c_7d8e9f0a1b2c);

    #[test]
    fn what_is_written_is_read_back() {
        assert_eq!(write(ID), "id = \"6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c\"\n");
        assert_eq!(read(&write(ID)), Ok(ID));
    }

    #[test]
    fn blank_lines_comments_spaces_and_crlf_are_accepted() {
        let text =
            "# A comment.\r\n\r\n  id=\"6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c\"   # why\r\n\n";
        assert_eq!(read(text), Ok(ID));
        assert_eq!(
            read("id   =   \"6F1C2B3A-0D4E-4F5A-8B6C-7D8E9F0A1B2C\""),
            Ok(ID)
        );
    }

    #[test]
    fn anything_else_is_refused_with_its_line() {
        let id = "\"6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c\"";
        for (text, error) in [
            (String::new(), MetadataError::NoId),
            ("# only a comment\n".to_owned(), MetadataError::NoId),
            (
                format!("\nname = {id}\n"),
                MetadataError::Unexpected { line: 2 },
            ),
            (
                format!("identity = {id}\n"),
                MetadataError::Unexpected { line: 1 },
            ),
            (
                "id = 6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c\n".to_owned(),
                MetadataError::Unexpected { line: 1 },
            ),
            (
                "id = \"6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c\n".to_owned(),
                MetadataError::Unexpected { line: 1 },
            ),
            (
                format!("id = {id} x\n"),
                MetadataError::Unexpected { line: 1 },
            ),
            (
                "id = \"not a uuid\"\n".to_owned(),
                MetadataError::NotAnId {
                    line: 1,
                    text: "not a uuid".to_owned(),
                },
            ),
            (
                format!("id = {id}\n# again\nid = {id}\n"),
                MetadataError::SecondId { line: 3 },
            ),
        ] {
            assert_eq!(read(&text), Err(error), "{text:?}");
        }
    }
}
