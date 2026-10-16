//! Metadata files: the `<file>.meta` beside each source file, which holds its asset ID.
//!
//! A metadata file is a document of a [`Metadata`] value, read strictly and written by the engine's
//! serialization (`slime-serialize`, `docs/adr/0086-serialization.md`): `id = "<uuid>"` and a line
//! feed, which TOML lets people write with comments, blank lines and CR LF endings too.

use slime_assets::AssetId;
use slime_reflect::Reflect;
use slime_serialize::{Mode, ReadError};

/// What a metadata file holds: the asset ID of the source file beside it, which every asset cooked
/// from it keeps.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "Metadata")]
pub struct Metadata {
    /// The source file's asset ID.
    pub id: AssetId,
}

/// The asset ID that the text of a metadata file holds.
///
/// ```
/// use slime_assets::AssetId;
/// use slime_cook::metadata;
///
/// let text = "# The cooker gave this ID.\nid = \"6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c\"\n";
/// let id = AssetId::from_u128(0x6f1c2b3a_0d4e_4f5a_8b6c_7d8e9f0a1b2c);
/// assert_eq!(metadata::read(text).expect("a metadata file"), id);
/// assert_eq!(metadata::read(&metadata::write(id)).expect("written as one"), id);
/// ```
///
/// # Errors
///
/// [`ReadError`] if the text is not a document of a [`Metadata`] value, with the line and the
/// column: not TOML, the ID missing, not an asset ID, or given beside a key that metadata files do
/// not have.
pub fn read(text: &str) -> Result<AssetId, ReadError> {
    let (metadata, _) = slime_serialize::read_as::<Metadata>(text, Mode::Strict)?;
    Ok(metadata.id)
}

/// The text of the metadata file that holds `id`, which [`read`] reads back:
/// `id = "<uuid>"` and a line feed.
///
/// # Panics
///
/// Never: the serialization refuses only references to entities, which metadata files do not hold.
pub fn write(id: AssetId) -> String {
    slime_serialize::write_as(&Metadata { id }).expect("a metadata file holds no entity")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: AssetId = AssetId::from_u128(0x6f1c2b3a_0d4e_4f5a_8b6c_7d8e9f0a1b2c);

    #[test]
    fn what_is_written_is_read_back() {
        assert_eq!(write(ID), "id = \"6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c\"\n");
        assert_eq!(read(&write(ID)).expect("written as one"), ID);
    }

    #[test]
    fn blank_lines_comments_spaces_and_crlf_are_accepted() {
        let text =
            "# A comment.\r\n\r\n  id=\"6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c\"   # why\r\n\n";
        assert_eq!(read(text).expect("a metadata file"), ID);
        let upper = "id   =   \"6F1C2B3A-0D4E-4F5A-8B6C-7D8E9F0A1B2C\"";
        assert_eq!(read(upper).expect("in capitals"), ID);
    }

    #[test]
    fn anything_else_is_refused_with_its_line() {
        let id = "\"6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c\"";
        for (text, line, message) in [
            (String::new(), 1, "the field `id` is missing"),
            (
                "# only a comment\n".to_owned(),
                1,
                "the field `id` is missing",
            ),
            (format!("\nname = {id}\n"), 2, "no field `name`"),
            (format!("identity = {id}\n"), 1, "no field `identity`"),
            (
                "id = 6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c\n".to_owned(),
                1,
                "not TOML",
            ),
            (
                "id = \"6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c\n".to_owned(),
                1,
                "not TOML",
            ),
            (format!("id = {id} x\n"), 1, "not TOML"),
            (
                "id = \"not a uuid\"\n".to_owned(),
                1,
                "`not a uuid` is not an asset ID",
            ),
            (format!("id = {id}\n# again\nid = {id}\n"), 3, "not TOML"),
        ] {
            let error = read(&text).expect_err(&text);
            assert_eq!(
                error.line_and_column().map(|(line, _)| line),
                Some(line),
                "{text:?}: {error}"
            );
            assert!(error.to_string().contains(message), "{text:?}: {error}");
        }
    }

    /// The metadata files under `directory`, but for build directories.
    fn metadata_files(directory: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        let entries = std::fs::read_dir(directory).expect("a directory of the repository");
        for entry in entries {
            let path = entry.expect("an entry").path();
            if path.is_dir() && !path.ends_with("target") {
                metadata_files(&path, files);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "meta")
            {
                files.push(path);
            }
        }
    }

    /// Each metadata file of the examples and the game is read, and written back to the same bytes.
    #[test]
    fn every_metadata_file_is_read_and_written_back_to_its_bytes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut files = Vec::new();
        for top in ["examples", "games"] {
            metadata_files(&root.join(top), &mut files);
        }
        assert!(files.len() >= 68, "{} metadata files", files.len());
        for file in files {
            let text = std::fs::read_to_string(&file).expect("a metadata file");
            let id = read(&text).unwrap_or_else(|error| panic!("{}: {error}", file.display()));
            assert_eq!(write(id), text, "{}: written back", file.display());
        }
    }
}
