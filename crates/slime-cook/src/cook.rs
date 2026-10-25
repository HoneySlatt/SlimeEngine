//! Cooking a directory of source files into a directory of cooked data: its files, named by what
//! made them, and its manifest.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use slime_assets::{
    AssetId, AssetKind, CookedMaterial, CookedMesh, CookedScene, CookedTexture, MANIFEST_FILE,
    Manifest, ManifestEntry,
};

use slime_world::{SceneError, SceneOutline};

use crate::gltf_import::{GltfFile, ImportError, lexical};
use crate::ids::texture_id;
use crate::image::{self, ImageError, ImageKind};
use crate::scan::{self, Mode, Problem, ScanError, Source};
use crate::texture::{TextureRole, cook_texture};

/// The version of what the cooker makes of an image: raised whenever the same image would give
/// other bytes, a change of filter, of encoder settings or of the encoder's version, so that every
/// texture is cooked again.
pub const TEXTURE_COOKER_VERSION: u32 = 2;

/// The version of what the cooker makes of a glTF file: raised whenever the same file would give
/// other meshes, materials or scenes, so that every glTF file is imported again.
pub const GLTF_COOKER_VERSION: u32 = 2;

/// The version of what the cooker makes of a scene file: raised whenever the same file would give
/// other bytes, so that every scene file is cooked again.
pub const SCENE_FILE_COOKER_VERSION: u32 = 1;

/// The version of what the cooker makes of a style sheet or a theme: raised whenever the same file
/// would give other bytes, so that every one is cooked again.
pub const STYLE_COOKER_VERSION: u32 = 1;

/// The extension of scene files (`docs/adr/0089-scenes-and-prefabs.md`).
pub const SCENE_EXTENSION: &str = "scene";

/// The extension of the interface's style sheets (`docs/adr/0141-style-sheets-and-themes.md`).
pub const STYLE_SHEET_EXTENSION: &str = "style";

/// The extension of the interface's themes (`docs/adr/0141-style-sheets-and-themes.md`).
pub const THEME_EXTENSION: &str = "theme";

/// What a cook did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cooked {
    /// The assets cooked, in the order of their source files.
    pub cooked: Vec<AssetId>,
    /// The assets whose cooked file was there already, from an earlier cook of the same source.
    pub reused: Vec<AssetId>,
    /// What the glTF files imported now have that the importer leaves out, and the paths that the
    /// tables of scene files give their assets where their sources are elsewhere, each after its
    /// file.
    pub warnings: Vec<String>,
    /// Metadata files without their source, reported and left alone.
    pub problems: Vec<Problem>,
}

/// Why a cook failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CookError {
    /// The directory of sources cannot be scanned.
    #[error(transparent)]
    Scan(#[from] ScanError),
    /// Two metadata files hold one ID: the cooker would cook two assets as one.
    #[error("{0}")]
    SameId(Problem),
    /// A file cannot be read or written.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// The operating system's error.
        source: io::Error,
    },
    /// An image cannot be imported.
    #[error("{} {source}", path.display())]
    Image {
        /// The image.
        path: PathBuf,
        /// Why.
        source: ImageError,
    },
    /// A glTF file cannot be imported.
    #[error("{}: {source}", path.display())]
    Import {
        /// The glTF file.
        path: PathBuf,
        /// Why.
        source: ImportError,
    },
    /// A scene file cannot be cooked: it says something wrong, names an asset that the sources do
    /// not make, places what is not a scene, or places scene files that place it; where, by its line.
    #[error("{}: {reason}", path.display())]
    Scene {
        /// The scene file.
        path: PathBuf,
        /// What is wrong, and where.
        reason: String,
    },
    /// A style sheet or a theme cannot be cooked: it is not a document, or not UTF-8 text; where,
    /// by its line.
    #[error("{}: {reason}", path.display())]
    Style {
        /// The style sheet or the theme.
        path: PathBuf,
        /// What is wrong, and where.
        reason: String,
    },
}

/// Cooks the source files of `sources` into `output`, which it creates if need be, and writes its
/// manifest there, [`MANIFEST_FILE`].
///
/// It scans `sources` first ([`scan::scan`]), giving an ID to each source file without one. It
/// imports each glTF file ([`crate::gltf_import`]): its meshes, materials and scenes, by the IDs of
/// sub-assets, the default scene by the file's own ID; and the textures that its materials read,
/// each in its role: the images embedded in the file with it, the image files beside it with the
/// images. Each image file becomes a texture for each role the materials read it in, or a texture
/// of colors ([`TextureRole::Color`]) if none reads it, whose ID is the image's: a game loads it
/// by that ID. The texture of an image in another role has an ID derived from the image's
/// ([`crate::ids::texture_id`]). Each scene file becomes its document in the engine's binary
/// encoding (`slime_serialize::encode`), under its own ID, of the kind [`AssetKind::SceneFile`],
/// which needs the assets of its table (`docs/adr/0095-cooked-scene-files.md`). Each style sheet
/// and theme of the interface becomes its document in the binary encoding, of the kind
/// [`AssetKind::StyleSheet`] or [`AssetKind::Theme`], its syntax checked, its types left to
/// `slime-ui`, which reads it (`docs/adr/0141-style-sheets-and-themes.md`). A scene file is read
/// first without the types of its values ([`slime_world::outline`]), and refused if it says
/// something wrong there, if its table names an asset that the sources do not make, if it places an
/// asset that is not a scene, or if scene files place each other; the path that its table gives an
/// asset whose source is elsewhere is a warning.
///
/// Each cooked file is named by a BLAKE3 hash of all that made it: for a texture of an image file,
/// the cooker's version ([`TEXTURE_COOKER_VERSION`]), the role, the image's ID and bytes; for what
/// a glTF file gives, the cooker's versions ([`GLTF_COOKER_VERSION`]), the file's ID and bytes, the
/// bytes of its buffers and the IDs of the image files it names, then the asset's ID; for a scene
/// file, its cooker's version ([`SCENE_FILE_COOKER_VERSION`]), its ID and its encoding; for a
/// style sheet or a theme, its kind, its cooker's version ([`STYLE_COOKER_VERSION`]), its ID and
/// its encoding. A record of each glTF file's import, named by the same hash, holds what it gave.
/// What is in `output` already under its name, and is whole, is not cooked again; so a second cook
/// of unchanged sources cooks nothing, and a source moved with its metadata neither. Whole, a
/// cooked file is a file that its kind's reader reads, of the size that the record of its import
/// says for what a glTF file gave, and a scene file's, a style sheet's or a theme's encoding byte
/// for byte: a cooked file deleted, emptied or cut short is cooked again. Every cooked file reused
/// is read: a cook of *Hunna Wood*'s zone that changes nothing,
/// which reads its 248 MB, took 0.23 s in a release build, where it took 0.09 s without the check.
/// Damage that leaves a file readable, texels changed, is not found. The same sources cooked into
/// two empty directories give the same bytes, on one platform.
///
/// Files are written under a temporary name of the process's own, forced to the disk, then renamed,
/// so that a cook stopped half-way, or a power cut, leaves no damaged file under a cooked name, and
/// two cooks of the same sources at once, which write the same bytes under the same names, leave
/// the cooked data whole. The manifest is written last, and only when what it lists changed: a game
/// that loads its data again when its manifest changes ([`slime_assets::ManifestWatch`]) loads
/// nothing again after a cook that changed nothing. Files that no source makes any more stay in
/// `output`.
///
/// # Errors
///
/// [`CookError::Scan`] if `sources` cannot be scanned, [`CookError::SameId`] if two metadata
/// files hold one ID, [`CookError::Import`] if a glTF file cannot be imported, [`CookError::Image`]
/// if an image cannot be, [`CookError::Scene`] if a scene file cannot be, by its line,
/// [`CookError::Style`] if a style sheet or a theme cannot be, by its line, and [`CookError::Io`]
/// if a file cannot be read or written. Files cooked before the error stay.
pub fn cook(sources: &Path, output: &Path) -> Result<Cooked, CookError> {
    let scan = scan::scan(sources, Mode::GiveIds)?;
    let mut result = Cooked::default();
    for problem in scan.problems {
        match problem {
            Problem::SameId { .. } => return Err(CookError::SameId(problem)),
            _ => result.problems.push(problem),
        }
    }
    fs::create_dir_all(output).map_err(|source| CookError::Io {
        path: output.to_owned(),
        source,
    })?;
    let images: Vec<(&Source, ImageKind)> = scan
        .sources
        .iter()
        .filter_map(|source| Some((source, ImageKind::from_extension(extension(&source.path))?)))
        .collect();
    let image_ids: HashMap<PathBuf, AssetId> = images
        .iter()
        .map(|(source, _)| (lexical(&source.path), source.id))
        .collect();

    let mut manifest = Manifest::new();
    // The roles the glTF files' materials read each image file in.
    let mut requests: HashMap<AssetId, Vec<TextureRole>> = HashMap::new();
    let scene_files = read_scene_files(&scan.sources, sources)?;
    for source in &scan.sources {
        if ImageKind::from_extension(extension(&source.path)).is_some()
            || [SCENE_EXTENSION, STYLE_SHEET_EXTENSION, THEME_EXTENSION]
                .iter()
                .any(|known| extension(&source.path).eq_ignore_ascii_case(known))
        {
            continue;
        }
        let record = cook_gltf(source, sources, output, &image_ids, &mut result)?;
        for (id, entry) in record.entries.iter() {
            manifest.insert(id, entry.clone());
        }
        for (image, role) in record.requests {
            let roles = requests.entry(image).or_default();
            if !roles.contains(&role) {
                roles.push(role);
            }
        }
    }

    for (source, kind) in images {
        let roles = requests
            .get(&source.id)
            .cloned()
            .unwrap_or_else(|| vec![TextureRole::Color]);
        for role in roles {
            let id = texture_id(source.id, role);
            let (file, size, fresh) = cook_image(source, kind, role, output)?;
            if fresh {
                result.cooked.push(id);
            } else {
                result.reused.push(id);
            }
            manifest.insert(
                id,
                ManifestEntry {
                    kind: AssetKind::Texture,
                    file,
                    size,
                    needs: Vec::new(),
                    source: relative(sources, &source.path),
                },
            );
        }
    }
    scene_files.cook(sources, output, &mut manifest, &mut result)?;
    cook_styles(&scan.sources, sources, output, &mut manifest, &mut result)?;
    let path = output.join(MANIFEST_FILE);
    let bytes = manifest.to_bytes();
    if read_output(&path, None).as_deref() != Some(bytes.as_slice()) {
        write(&path, &bytes)?;
    }
    Ok(result)
}

/// How often [`watch`] looks for changes in the directory of sources: as often as a game reads its
/// manifest ([`slime_assets::ManifestWatch`]).
pub const WATCH_INTERVAL: Duration = slime_assets::ManifestWatch::INTERVAL;

/// Cooks `sources` into `output` as [`cook`] does, then again each time a file inside `sources`
/// changes, appears or goes, which it looks for every [`WATCH_INTERVAL`] by the size and the time
/// of change of every file: a source, a metadata file, or a file that a source reads, such as a
/// glTF file's buffers. It cooks once the files have stayed as they are for one interval, so that an
/// export that writes several files, a glTF file and its buffers, is cooked whole. It gives `report`
/// the result of each cook, and returns once `report` returns [`ControlFlow::Break`]. A cook that
/// fails does not end the watch: an image read while an editor was still saving it fails to decode,
/// and the end of the save, a change, cooks it again.
///
/// It reads no file between cooks, only the sizes and times of the files, and uses the operating
/// system's clock rather than its notifications: a change is cooked between one and two intervals
/// after it is made.
///
/// # Errors
///
/// [`ScanError::Io`] if `sources` itself cannot be read while it looks for changes, as when it is
/// removed; a directory inside it that goes while it is read is a change.
pub fn watch(
    sources: &Path,
    output: &Path,
    mut report: impl FnMut(Result<Cooked, CookError>) -> ControlFlow<()>,
) -> Result<(), ScanError> {
    loop {
        // The IDs first, so that the metadata files the cook would write are not taken for a
        // change; what goes wrong is reported by the cook, which scans again.
        let _ = scan::scan(sources, Mode::GiveIds);
        // Taken before the cook, so that a change made while it runs is cooked after it.
        let before = files(sources)?;
        if report(cook(sources, output)).is_break() {
            return Ok(());
        }
        // Cooked again once the files differ from before the cook, and stayed so for an interval.
        let mut last = files(sources)?;
        loop {
            thread::sleep(WATCH_INTERVAL);
            let now = files(sources)?;
            if now != before && now == last {
                break;
            }
            last = now;
        }
    }
}

/// A file's size and time of change, if it can be read.
type FileState = Option<(u64, SystemTime)>;

/// Every file inside `directory`, with its state, in the order of the paths.
fn files(directory: &Path) -> Result<Vec<(PathBuf, FileState)>, ScanError> {
    let mut paths = Vec::new();
    if let Err(error) = scan::walk(directory, &mut paths) {
        match error {
            // A directory inside that went while the walk read it stands in the list without a
            // state: a change, which the next look settles.
            ScanError::Io { path, .. } if path != directory => paths.push(path),
            error => return Err(error),
        }
    }
    paths.sort();
    Ok(paths
        .into_iter()
        .map(|path| {
            // A file removed since the walk read its directory is a change too.
            let state = fs::metadata(&path)
                .ok()
                .and_then(|metadata| Some((metadata.len(), metadata.modified().ok()?)));
            (path, state)
        })
        .collect())
}

/// The extension of `path`, or nothing.
fn extension(path: &Path) -> &str {
    path.extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
}

/// What importing a glTF file gave: the manifest's entries of its assets, and the image files it
/// asked for in each role.
struct Record {
    entries: Manifest,
    requests: Vec<(AssetId, TextureRole)>,
}

/// The four bytes a record starts with, then its version.
const RECORD_MAGIC: &[u8; 4] = b"SLIR";
const RECORD_VERSION: u32 = 1;

impl Record {
    /// The record's file: its magic and version, the count of requests and each, an image's ID (16
    /// bytes) and a role's tag (1 byte), then the manifest of its entries.
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = RECORD_MAGIC.to_vec();
        bytes.extend_from_slice(&RECORD_VERSION.to_le_bytes());
        bytes.extend_from_slice(&(self.requests.len() as u32).to_le_bytes());
        for (image, role) in &self.requests {
            bytes.extend_from_slice(&image.as_u128().to_le_bytes());
            bytes.push(role.tag());
        }
        bytes.extend_from_slice(&self.entries.to_bytes());
        bytes
    }

    /// The record of a file of [`Record::to_bytes`], or `None` if it is not one: the import is
    /// then made again.
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let rest = bytes.strip_prefix(RECORD_MAGIC.as_slice())?;
        let (version, rest) = rest.split_first_chunk::<4>()?;
        if u32::from_le_bytes(*version) != RECORD_VERSION {
            return None;
        }
        let (count, mut rest) = rest.split_first_chunk::<4>()?;
        let mut requests = Vec::new();
        for _ in 0..u32::from_le_bytes(*count) {
            let (image, after) = rest.split_first_chunk::<16>()?;
            let (&role, after) = after.split_first()?;
            requests.push((
                AssetId::from_u128(u128::from_le_bytes(*image)),
                TextureRole::from_tag(role)?,
            ));
            rest = after;
        }
        Some(Self {
            entries: Manifest::from_bytes(rest).ok()?,
            requests,
        })
    }
}

/// Imports the glTF file `source` of `sources` into `output`, or reads the record of an import of
/// the same file, buffers and image IDs made before.
fn cook_gltf(
    source: &Source,
    sources: &Path,
    output: &Path,
    image_ids: &HashMap<PathBuf, AssetId>,
    result: &mut Cooked,
) -> Result<Record, CookError> {
    let bytes = read(&source.path)?;
    let import_error = |error| CookError::Import {
        path: source.path.clone(),
        source: error,
    };
    let file = GltfFile::open(&source.path, &bytes, sources).map_err(import_error)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"gltf");
    hasher.update(&GLTF_COOKER_VERSION.to_le_bytes());
    hasher.update(&TEXTURE_COOKER_VERSION.to_le_bytes());
    hasher.update(&source.id.as_u128().to_le_bytes());
    for part in std::iter::once(bytes.as_slice()).chain(file.buffers().iter().map(Vec::as_slice)) {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    for image in file.image_files() {
        let id = image_ids.get(&image).map_or(0, |id| id.as_u128());
        hasher.update(&id.to_le_bytes());
    }
    let key = hasher.finalize();
    let record_path = output.join(format!("{}.import", key.to_hex()));
    if let Some(record) = read_output(&record_path, None)
        .and_then(|bytes| Record::from_bytes(&bytes))
        .filter(|record| {
            record
                .entries
                .iter()
                .all(|(_, entry)| intact(&output.join(&entry.file), entry.kind, Some(entry.size)))
        })
    {
        result
            .reused
            .extend(record.entries.iter().map(|(id, _)| id));
        return Ok(record);
    }

    let imported = file.import(source.id, image_ids).map_err(import_error)?;
    let path = relative(sources, &source.path);
    result.warnings.extend(
        imported
            .warnings
            .iter()
            .map(|warning| format!("{path}: {warning}")),
    );
    let mut record = Record {
        entries: Manifest::new(),
        requests: imported.external.clone(),
    };
    // Each asset's file is named by the import's hash and the asset's ID.
    let mut add = |id: AssetId,
                   kind: AssetKind,
                   extension: &str,
                   bytes: &[u8],
                   needs: Vec<AssetId>|
     -> Result<(), CookError> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(key.as_bytes());
        hasher.update(&id.as_u128().to_le_bytes());
        let name = format!("{}.{extension}", hasher.finalize().to_hex());
        write(&output.join(&name), bytes)?;
        result.cooked.push(id);
        record.entries.insert(
            id,
            ManifestEntry {
                kind,
                file: name,
                size: bytes.len() as u64,
                needs,
                source: path.clone(),
            },
        );
        Ok(())
    };
    for image in &imported.embedded {
        let decoded =
            image::decode(image.kind, &image.bytes).map_err(|error| CookError::Image {
                path: source.path.join(&image.name),
                source: error,
            })?;
        for &role in &image.roles {
            let texture = cook_texture(&decoded, role).to_ktx2();
            add(
                texture_id(image.id, role),
                AssetKind::Texture,
                "ktx2",
                &texture,
                Vec::new(),
            )?;
        }
    }
    for (id, material) in &imported.materials {
        add(
            *id,
            AssetKind::Material,
            "material",
            &material.to_bytes(),
            material.needs(),
        )?;
    }
    for (id, mesh) in &imported.meshes {
        add(
            *id,
            AssetKind::Mesh,
            "mesh",
            &mesh.to_bytes(),
            vec![mesh.material],
        )?;
    }
    for (id, scene) in &imported.scenes {
        add(
            *id,
            AssetKind::Scene,
            "scene",
            &scene.to_bytes(),
            scene.needs(),
        )?;
    }
    // Written last: its presence means that every file it lists was written.
    write(&record_path, &record.to_bytes())?;
    Ok(record)
}

fn read(path: &Path) -> Result<Vec<u8>, CookError> {
    fs::read(path).map_err(|source| CookError::Io {
        path: path.to_owned(),
        source,
    })
}

/// Cooks the image `source` of `kind` into `output` as a texture for `role`, unless its cooked
/// file is there already. Returns the file's name, its size, and whether it was cooked now.
fn cook_image(
    source: &Source,
    kind: ImageKind,
    role: TextureRole,
    output: &Path,
) -> Result<(String, u64, bool), CookError> {
    let bytes = read(&source.path)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"texture");
    hasher.update(&TEXTURE_COOKER_VERSION.to_le_bytes());
    hasher.update(&[role.tag()]);
    hasher.update(&source.id.as_u128().to_le_bytes());
    hasher.update(&bytes);
    let name = format!("{}.ktx2", hasher.finalize().to_hex());
    let path = output.join(&name);
    if intact(&path, AssetKind::Texture, None)
        && let Ok(metadata) = fs::metadata(&path)
    {
        return Ok((name, metadata.len(), false));
    }
    let image = image::decode(kind, &bytes).map_err(|error| CookError::Image {
        path: source.path.clone(),
        source: error,
    })?;
    let file = cook_texture(&image, role).to_ktx2();
    write(&path, &file)?;
    Ok((name, file.len() as u64, true))
}

/// `path`, inside `root`, relative to it, with `/` between its directories on every platform.
fn relative(root: &Path, path: &Path) -> String {
    let inside = path.strip_prefix(root).unwrap_or(path);
    inside
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Writes `bytes` to `path`, whole or not at all ([`crate::write`]).
fn write(path: &Path, bytes: &[u8]) -> Result<(), CookError> {
    crate::write(path, bytes).map_err(|source| CookError::Io {
        path: path.to_owned(),
        source,
    })
}

/// Whether the cooked file at `path` can be reused: a file that reads as an asset of `kind`, of
/// `size` bytes if it is given. A cooked file deleted, emptied or cut short, as a power cut or a
/// full disk leaves one, is cooked again; damage that leaves it readable, a texel changed, is not
/// found.
fn intact(path: &Path, kind: AssetKind, size: Option<u64>) -> bool {
    let Some(bytes) = read_output(path, size) else {
        return false;
    };
    match kind {
        AssetKind::Texture => CookedTexture::from_ktx2(&bytes).is_ok(),
        AssetKind::Mesh => CookedMesh::from_bytes(&bytes).is_ok(),
        AssetKind::Material => CookedMaterial::from_bytes(&bytes).is_ok(),
        AssetKind::Scene => CookedScene::from_bytes(&bytes).is_ok(),
        // Scene files are compared with their encoding, which a cook makes again.
        _ => false,
    }
}

/// The bytes of the file of the output at `path`, which the cooker reuses: `None` if it is not a
/// file, as a directory, a pipe or a device is not, or is not of `size` bytes when it is given, both
/// checked before it is read, or if it cannot be read; the cooker makes it again then.
fn read_output(path: &Path, size: Option<u64>) -> Option<Vec<u8>> {
    let metadata = fs::metadata(path).ok()?;
    let size = size.unwrap_or(metadata.len());
    if !metadata.is_file() || metadata.len() != size {
        return None;
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(size).read_to_end(&mut bytes))
        .ok()?;
    Some(bytes)
}

/// A step of a walk of the placements of scene files: a file, the files that it places with the
/// lines of those placements, and how many of them the walk took.
type Step = (AssetId, Vec<(AssetId, usize)>, usize);

/// Cooks each style sheet and theme among `sources`, inside `root`, into its document's binary
/// encoding, of the kind [`AssetKind::StyleSheet`] or [`AssetKind::Theme`]: its syntax checked, its
/// types left to `slime-ui`, which reads it, as a scene file's are left to the world.
fn cook_styles(
    sources: &[Source],
    root: &Path,
    output: &Path,
    manifest: &mut Manifest,
    result: &mut Cooked,
) -> Result<(), CookError> {
    for source in sources {
        let found = extension(&source.path);
        let (kind, made) = if found.eq_ignore_ascii_case(STYLE_SHEET_EXTENSION) {
            (AssetKind::StyleSheet, STYLE_SHEET_EXTENSION)
        } else if found.eq_ignore_ascii_case(THEME_EXTENSION) {
            (AssetKind::Theme, THEME_EXTENSION)
        } else {
            continue;
        };
        let refuse = |reason: String| CookError::Style {
            path: source.path.clone(),
            reason,
        };
        let text = String::from_utf8(read(&source.path)?)
            .map_err(|_| refuse("not UTF-8 text".to_owned()))?;
        let bytes = slime_serialize::encode(&text).map_err(|error| refuse(error.to_string()))?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(kind.name().as_bytes());
        hasher.update(&STYLE_COOKER_VERSION.to_le_bytes());
        hasher.update(&source.id.as_u128().to_le_bytes());
        hasher.update(&bytes);
        let file = format!("{}.{made}", hasher.finalize().to_hex());
        let path = output.join(&file);
        if read_output(&path, Some(bytes.len() as u64)).is_some_and(|cooked| cooked == bytes) {
            result.reused.push(source.id);
        } else {
            write(&path, &bytes)?;
            result.cooked.push(source.id);
        }
        manifest.insert(
            source.id,
            ManifestEntry {
                kind,
                file,
                size: bytes.len() as u64,
                needs: Vec::new(),
                source: relative(root, &source.path),
            },
        );
    }
    Ok(())
}

/// The scene files of a cook, by their IDs: their paths and texts, and what checking them read.
struct SceneFiles<'s> {
    files: Vec<(&'s Source, String, SceneOutline)>,
}

/// Reads and checks the scene files among `sources`, inside `root`: each one's layout, without the
/// types of its values ([`slime_world::outline`]).
fn read_scene_files<'s>(sources: &'s [Source], root: &Path) -> Result<SceneFiles<'s>, CookError> {
    let mut files = Vec::new();
    for source in sources
        .iter()
        .filter(|source| extension(&source.path).eq_ignore_ascii_case(SCENE_EXTENSION))
    {
        let bytes = read(&source.path)?;
        let text = String::from_utf8(bytes).map_err(|_| CookError::Scene {
            path: source.path.clone(),
            reason: "not UTF-8 text".to_owned(),
        })?;
        let name = relative(root, &source.path);
        let outline = slime_world::outline(&name, &text).map_err(|error| CookError::Scene {
            path: source.path.clone(),
            reason: match error {
                SceneError::File { error, .. } => error.to_string(),
                error => error.to_string(),
            },
        })?;
        files.push((source, text, outline));
    }
    Ok(SceneFiles { files })
}

impl SceneFiles<'_> {
    /// Checks each scene file against what the cook makes, `manifest`, and the other scene files:
    /// each asset of its table one that the cook makes, each prefab a scene, no scene files that
    /// place each other; then encodes each, and adds it to `manifest`. The path that a table gives
    /// an asset, which differs from its source's, is a warning.
    fn cook(
        &self,
        root: &Path,
        output: &Path,
        manifest: &mut Manifest,
        result: &mut Cooked,
    ) -> Result<(), CookError> {
        let ours: HashMap<AssetId, &(&Source, String, SceneOutline)> =
            self.files.iter().map(|file| (file.0.id, file)).collect();
        let refuse = |source: &Source, line: usize, reason: String| CookError::Scene {
            path: source.path.clone(),
            reason: format!("line {line}: {reason}"),
        };
        for (source, _, outline) in &self.files {
            let directory = source.path.parent().unwrap_or(root);
            for asset in &outline.assets {
                let (kind, actual) = match (manifest.get(asset.id), ours.get(&asset.id)) {
                    (Some(entry), _) => (entry.kind, entry.source.clone()),
                    (None, Some(file)) => (AssetKind::SceneFile, relative(root, &file.0.path)),
                    (None, None) => {
                        return Err(refuse(
                            source,
                            asset.line,
                            format!(
                                "the asset `{}`, {}, is not one that the sources make",
                                asset.name, asset.id
                            ),
                        ));
                    }
                };
                if let Some(path) = &asset.path {
                    let given = relative(&lexical(root), &lexical(&directory.join(path)));
                    if given != actual {
                        result.warnings.push(format!(
                            "{}:{}: the table gives the asset `{}` the path `{path}`, where its \
                             source is `{actual}`",
                            relative(root, &source.path),
                            asset.line,
                            asset.name
                        ));
                    }
                }
                for entity in outline
                    .entities
                    .iter()
                    .filter(|entity| entity.prefab == Some(asset.id))
                {
                    if !matches!(kind, AssetKind::Scene | AssetKind::SceneFile) {
                        return Err(refuse(
                            source,
                            entity.line,
                            format!(
                                "`{}` places the asset `{}`, a {kind}, where a scene goes",
                                entity.id, asset.name
                            ),
                        ));
                    }
                }
            }
        }
        self.refuse_cycles(&ours, root)?;
        for (source, text, outline) in &self.files {
            let bytes = slime_serialize::encode(text).map_err(|error| CookError::Scene {
                path: source.path.clone(),
                reason: error.to_string(),
            })?;
            let mut hasher = blake3::Hasher::new();
            hasher.update(b"scene file");
            hasher.update(&SCENE_FILE_COOKER_VERSION.to_le_bytes());
            hasher.update(&source.id.as_u128().to_le_bytes());
            hasher.update(&bytes);
            let file = format!("{}.scenefile", hasher.finalize().to_hex());
            let path = output.join(&file);
            if read_output(&path, Some(bytes.len() as u64)).is_some_and(|cooked| cooked == bytes) {
                result.reused.push(source.id);
            } else {
                write(&path, &bytes)?;
                result.cooked.push(source.id);
            }
            let mut needs: Vec<AssetId> = Vec::new();
            for asset in &outline.assets {
                if !needs.contains(&asset.id) {
                    needs.push(asset.id);
                }
            }
            manifest.insert(
                source.id,
                ManifestEntry {
                    kind: AssetKind::SceneFile,
                    file,
                    size: bytes.len() as u64,
                    needs,
                    source: relative(root, &source.path),
                },
            );
        }
        Ok(())
    }

    /// Refuses scene files that place each other, through their prefabs: where a walk of the
    /// placements from the first file, in their order, first meets a file that it went through, at
    /// the line of the placement that the cycle takes from that file.
    fn refuse_cycles(
        &self,
        ours: &HashMap<AssetId, &(&Source, String, SceneOutline)>,
        root: &Path,
    ) -> Result<(), CookError> {
        // The placements of a file that place another file: each prefab, and the line.
        let placements = |id: AssetId| -> Vec<(AssetId, usize)> {
            ours[&id]
                .2
                .entities
                .iter()
                .filter_map(|entity| Some((entity.prefab?, entity.line)))
                .filter(|(prefab, _)| ours.contains_key(prefab))
                .collect()
        };
        // Files whose walk ended, and those that the walk goes through.
        let mut done: Vec<AssetId> = Vec::new();
        for (source, _, _) in &self.files {
            if done.contains(&source.id) {
                continue;
            }
            // Each file the walk goes through, its placements, and how many it took.
            let mut walk: Vec<Step> = vec![(source.id, placements(source.id), 0)];
            while let Some(top) = walk.len().checked_sub(1) {
                let Some(&(prefab, _)) = walk[top].1.get(walk[top].2) else {
                    done.push(walk[top].0);
                    walk.pop();
                    continue;
                };
                walk[top].2 += 1;
                if let Some(start) = walk.iter().position(|(id, _, _)| *id == prefab) {
                    let (first, taken, count) = &walk[start];
                    let line = taken[count - 1].1;
                    let names: Vec<String> = walk[start..]
                        .iter()
                        .map(|(id, _, _)| relative(root, &ours[id].0.path))
                        .chain(std::iter::once(relative(root, &ours[&prefab].0.path)))
                        .collect();
                    return Err(CookError::Scene {
                        path: ours[first].0.path.clone(),
                        reason: format!(
                            "line {line}: scene files that place each other: {}",
                            names.join(" → ")
                        ),
                    });
                }
                if !done.contains(&prefab) {
                    walk.push((prefab, placements(prefab), 0));
                }
            }
        }
        Ok(())
    }
}
