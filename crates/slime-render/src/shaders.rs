//! The renderer's shaders: WGSL sources embedded in the program when it is compiled, which a game
//! can replace while it runs (ADR-0077).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The WGSL sources of the renderer's shaders, which [`Renderer::set_shaders`] compiles:
/// those embedded in the program by default ([`ShaderSources::embedded`]), or read from their
/// files ([`ShaderSources::read`]), as a game that hot-reloads does ([`ShaderWatch`]).
///
/// [`Renderer::set_shaders`]: crate::Renderer::set_shaders
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShaderSources {
    /// `shader.wgsl`: the physically based kind of material's passes, shadow and scene, and the sky
    /// pass.
    pub scene: String,
    /// `tone_mapping.wgsl`: the tone-mapping pass.
    pub tone_mapping: String,
    /// `bloom.wgsl`: the bloom's passes, down and up.
    pub bloom: String,
    /// `resolve.wgsl`: the resolve passes, of the scene's light and of its depth (ADR-0159).
    pub resolve: String,
    /// `unlit.wgsl`: the scene pass's unlit materials (ADR-0129).
    pub unlit: String,
    /// `lines.wgsl`: the lines that a client draws into a view (ADR-0129).
    pub lines: String,
    /// `canvas.wgsl`: the canvas, which an interface draws over a target (ADR-0137).
    pub canvas: String,
    /// `outline.wgsl`: the outline that a client draws around instances (ADR-0159).
    pub outline: String,
}

impl ShaderSources {
    /// The directory that holds the shaders' files: `crates/slime-render/src` in the workspace
    /// that compiled the program. A program run on another machine, such as those that CI ships,
    /// does not find it.
    pub const DIRECTORY: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/src");

    /// The sources embedded in the program when it was compiled, which [`Renderer::new`] uses.
    ///
    /// [`Renderer::new`]: crate::Renderer::new
    pub fn embedded() -> Self {
        Self {
            scene: include_str!("shader.wgsl").to_owned(),
            tone_mapping: include_str!("tone_mapping.wgsl").to_owned(),
            bloom: include_str!("bloom.wgsl").to_owned(),
            resolve: include_str!("resolve.wgsl").to_owned(),
            unlit: include_str!("unlit.wgsl").to_owned(),
            lines: include_str!("lines.wgsl").to_owned(),
            canvas: include_str!("canvas.wgsl").to_owned(),
            outline: include_str!("outline.wgsl").to_owned(),
        }
    }

    /// The sources in the files of `directory`, named as in [`ShaderSources::DIRECTORY`].
    ///
    /// # Errors
    ///
    /// The operating system's error if a file cannot be read, or is not UTF-8, with the file's path.
    pub fn read(directory: &Path) -> io::Result<Self> {
        let read = |name: &str| {
            let path = directory.join(name);
            fs::read_to_string(&path).map_err(|error| {
                io::Error::new(error.kind(), format!("{}: {error}", path.display()))
            })
        };
        Ok(Self {
            scene: read("shader.wgsl")?,
            tone_mapping: read("tone_mapping.wgsl")?,
            bloom: read("bloom.wgsl")?,
            resolve: read("resolve.wgsl")?,
            unlit: read("unlit.wgsl")?,
            lines: read("lines.wgsl")?,
            canvas: read("canvas.wgsl")?,
            outline: read("outline.wgsl")?,
        })
    }
}

/// Tells a game that hot-reloads when the renderer's shader files change (ADR-0077): it reads them
/// again at most every [`ShaderWatch::INTERVAL`], and compares them with the last sources it gave,
/// or with the embedded ones, which a new renderer uses.
#[derive(Debug)]
pub struct ShaderWatch {
    directory: PathBuf,
    /// The sources it gave last, or the embedded ones.
    seen: ShaderSources,
    /// When it reads the files next.
    next: Instant,
    /// Whether the last read failed, so that a failure is told once.
    failing: bool,
}

impl ShaderWatch {
    /// How often it reads the files: as often as a game reads its manifest
    /// (`slime_assets::ManifestWatch`).
    pub const INTERVAL: Duration = slime_assets::ManifestWatch::INTERVAL;

    /// A watch of the shader files of `directory`, usually [`ShaderSources::DIRECTORY`], which
    /// starts from the embedded sources; it reads the files first [`ShaderWatch::INTERVAL`] after
    /// `now`.
    pub fn new(directory: impl Into<PathBuf>, now: Instant) -> Self {
        Self {
            directory: directory.into(),
            seen: ShaderSources::embedded(),
            next: now + Self::INTERVAL,
            failing: false,
        }
    }

    /// Starts again from the embedded sources, as a renderer made again does: after a new GPU
    /// device, the files that differ from them are given again.
    pub fn start_again(&mut self) {
        self.seen = ShaderSources::embedded();
    }

    /// The sources in the files, if [`ShaderWatch::INTERVAL`] has passed since the last read, at
    /// `now`, and they differ from the last ones given; `None` otherwise.
    ///
    /// # Errors
    ///
    /// As [`ShaderSources::read`], the first time the files cannot be read after they could; the
    /// reads that fail after that give `None`, so that a game tells the failure once.
    pub fn poll(&mut self, now: Instant) -> io::Result<Option<ShaderSources>> {
        if now < self.next {
            return Ok(None);
        }
        self.next = now + Self::INTERVAL;
        match ShaderSources::read(&self.directory) {
            Ok(sources) => {
                self.failing = false;
                if sources == self.seen {
                    return Ok(None);
                }
                self.seen = sources.clone();
                Ok(Some(sources))
            }
            Err(_) if self.failing => Ok(None),
            Err(error) => {
                self.failing = true;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_files_hold_the_embedded_sources() {
        let read = ShaderSources::read(Path::new(ShaderSources::DIRECTORY)).expect("read");
        assert!(read == ShaderSources::embedded());
    }

    #[test]
    fn a_watch_gives_the_sources_once_each_time_a_file_changes() {
        let directory =
            std::env::temp_dir().join(format!("slime-render-shaders-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("created");
        let embedded = ShaderSources::embedded();
        let write = |sources: &ShaderSources| {
            for (name, text) in [
                ("shader.wgsl", &sources.scene),
                ("tone_mapping.wgsl", &sources.tone_mapping),
                ("bloom.wgsl", &sources.bloom),
                ("resolve.wgsl", &sources.resolve),
                ("unlit.wgsl", &sources.unlit),
                ("lines.wgsl", &sources.lines),
                ("canvas.wgsl", &sources.canvas),
                ("outline.wgsl", &sources.outline),
            ] {
                fs::write(directory.join(name), text).expect("written");
            }
        };
        write(&embedded);
        let start = Instant::now();
        let later = |intervals: u32| start + ShaderWatch::INTERVAL * intervals;
        let mut watch = ShaderWatch::new(&directory, start);
        assert!(
            watch.poll(later(1)).expect("read").is_none(),
            "the embedded ones"
        );
        let changed = ShaderSources {
            bloom: format!("{}\n// changed\n", embedded.bloom),
            ..embedded.clone()
        };
        write(&changed);
        assert!(watch.poll(later(1)).expect("read").is_none(), "not yet");
        assert!(watch.poll(later(2)).expect("read") == Some(changed.clone()));
        assert!(watch.poll(later(3)).expect("read").is_none(), "given once");
        watch.start_again();
        assert!(watch.poll(later(4)).expect("read") == Some(changed));
        fs::remove_file(directory.join("bloom.wgsl")).expect("removed");
        assert!(watch.poll(later(5)).is_err());
        assert!(watch.poll(later(6)).expect("told once").is_none());
        fs::remove_dir_all(&directory).expect("removed");
    }
}
