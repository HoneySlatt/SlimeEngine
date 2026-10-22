//! The level that the editor changes: a scene file, opened into a world and written back with its
//! comments (`docs/adr/0134-the-editors-core.md`).

use std::fs;
use std::path::{Path, PathBuf};

use slime_serialize::keep_comments;
use slime_world::{PersistentId, SceneLibrary, SetAside, World};

use crate::editor::EditError;

/// A level opened: its scene file, the key that its scene is spawned under, and the text of its
/// file as the editor last read or wrote it, whose comments it keeps.
pub(crate) struct Level {
    pub(crate) file: PathBuf,
    pub(crate) key: String,
    pub(crate) text: String,
}

impl Level {
    /// Opens the scene file `file` into `world`, as [`Level::open_as`] does with the file's text.
    pub(crate) fn open(
        world: &mut World,
        scenes: &mut SceneLibrary,
        file: &Path,
        key: &str,
    ) -> Result<Self, EditError> {
        let text = read(file)?;
        Self::open_as(world, scenes, file, key, text)
    }

    /// Opens the scene file `file` into `world` as if it held `text`, spawned under `key`, its asset
    /// ID read from its metadata file, and the prefabs that it places resolved through `scenes`.
    pub(crate) fn open_as(
        world: &mut World,
        scenes: &mut SceneLibrary,
        file: &Path,
        key: &str,
        text: String,
    ) -> Result<Self, EditError> {
        let metadata = slime_cook::scan::metadata_path(file);
        let id =
            slime_cook::metadata::read(&read(&metadata)?).map_err(|error| EditError::Metadata {
                path: metadata.clone(),
                error,
            })?;
        let name = file.file_name().map_or_else(
            || file.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        scenes.insert_file(id, name, text.clone());
        let scene = scenes
            .resolve(world.registry(), id)
            .map_err(EditError::Scene)?;
        world
            .spawn_scene(key, &scene)
            .map_err(|error| EditError::Spawn(Box::new(error)))?;
        Ok(Self {
            file: file.to_owned(),
            key: key.to_owned(),
            text,
        })
    }

    /// The text that writing the level from `world` gives, the comments of its text kept, and what
    /// the file cannot hold, without writing it.
    pub(crate) fn written(&self, world: &mut World) -> Result<(String, Vec<SetAside>), EditError> {
        let id = PersistentId::new([self.key.as_str()]);
        let root = world.find(&id).ok_or(EditError::NoLevel(id))?;
        let written = world.write_scene(root).map_err(EditError::Write)?;
        Ok((keep_comments(&self.text, &written.text), written.left_out))
    }

    /// Writes the level back to its file from `world`, whole or not at all, and returns what the
    /// file could not hold.
    pub(crate) fn write(&mut self, world: &mut World) -> Result<Vec<SetAside>, EditError> {
        let (text, left_out) = self.written(world)?;
        slime_cook::write(&self.file, text.as_bytes()).map_err(|error| EditError::Io {
            path: self.file.clone(),
            error,
        })?;
        self.text = text;
        Ok(left_out)
    }
}

/// The text of the file `path`.
pub(crate) fn read(path: &Path) -> Result<String, EditError> {
    fs::read_to_string(path).map_err(|error| EditError::Io {
        path: path.to_owned(),
        error,
    })
}
