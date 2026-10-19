//! What every benchmark shares: the GPU, the examples' sample cooked, and the figures of a series
//! of times.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use slime_assets::{AssetId, AssetKind, CookedAssets};
use slime_gfx::Gpu;

/// One line of the table: what was measured, and its value with its unit.
pub struct Row {
    pub what: String,
    pub value: String,
}

impl Row {
    pub fn new(what: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            what: what.into(),
            value: value.into(),
        }
    }

    /// The median of `times` and their 90th percentile, as two rows about `what`, which say how
    /// many times were taken.
    pub fn times(what: &str, times: &[Duration]) -> [Self; 2] {
        let count = times.len();
        [
            Self::new(
                format!("{what}, median of {count}"),
                ms(percentile(times, 0.5)),
            ),
            Self::new(
                format!("{what}, 90th percentile of {count}"),
                ms(percentile(times, 0.9)),
            ),
        ]
    }
}

/// How long the assets of a frame may take to load before a benchmark gives up.
pub const LOAD_LIMIT: Duration = Duration::from_secs(120);

/// What the benchmarks of one run share.
pub struct Context {
    /// The GPU, opened once, without a window, as `Gpu::new` chooses it: lavapipe when
    /// `VK_DRIVER_FILES` names it.
    pub gpu: Gpu,
    /// The examples' sample, once cooked.
    sample: Option<CookedAssets>,
}

impl Context {
    pub fn new() -> Result<Self, String> {
        let gpu = Gpu::new().map_err(|error| format!("no GPU: {error}"))?;
        Ok(Self { gpu, sample: None })
    }

    /// The machine the figures come from: its logical CPUs, its system, its GPU and the build.
    pub fn machine(&self) -> String {
        let cpus = std::thread::available_parallelism().map_or(0, usize::from);
        let build = if cfg!(debug_assertions) {
            "a debug build, whose figures are not comparable"
        } else {
            "a release build"
        };
        format!(
            "{cpus} logical CPUs, {} on {}, GPU `{}`, {build}.",
            std::env::consts::OS,
            std::env::consts::ARCH,
            self.gpu.adapter_name()
        )
    }

    /// The workspace's directory.
    pub fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the benchmarks' crate is in the workspace")
            .to_path_buf()
    }

    /// The examples' sample, `examples/assets`, cooked into `target/benches/sample` by the first
    /// call of a run, which a later run's cook reuses.
    pub fn sample(&mut self) -> Result<CookedAssets, String> {
        if let Some(sample) = &self.sample {
            return Ok(sample.clone());
        }
        let root = Self::root();
        let output = root.join("target/benches/sample");
        slime_cook::cook(&root.join("examples/assets"), &output)
            .map_err(|error| format!("cannot cook the examples' sample: {error}"))?;
        let sample = CookedAssets::open(&output)
            .map_err(|error| format!("cannot read the examples' sample: {error}"))?;
        self.sample = Some(sample.clone());
        Ok(sample)
    }
}

/// The meshes of `assets`, by their IDs.
pub fn meshes_of(assets: &CookedAssets) -> Vec<AssetId> {
    assets
        .manifest()
        .iter()
        .filter(|(_, entry)| entry.kind == AssetKind::Mesh)
        .map(|(id, _)| id)
        .collect()
}

/// The time at `share` of the way through `times` sorted: 0.5 for the median, 0.9 for the 90th
/// percentile, the nearest of them taken.
pub fn percentile(times: &[Duration], share: f64) -> Duration {
    let mut sorted = times.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() - 1) as f64 * share).round() as usize;
    sorted[index]
}

/// `duration` in milliseconds, to a hundredth, or in microseconds below a millisecond.
pub fn ms(duration: Duration) -> String {
    let milliseconds = duration.as_secs_f64() * 1000.0;
    if milliseconds < 1.0 {
        format!("{:.1} µs", milliseconds * 1000.0)
    } else {
        format!("{milliseconds:.2} ms")
    }
}

/// What `run` gives, and how long it took.
pub fn timed<T>(run: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = run();
    (value, start.elapsed())
}
