//! The cooked scene that the `scene` example and its image tests draw: the asset of
//! `examples/assets/scene.glb` by default, or any scene that `slime-cook` cooked, lit by the lights
//! its nodes place, or by the sample's sun when they place none, and by its sky, in physical units
//! (ADR-0037, ADR-0039, ADR-0069).

use std::error::Error;
use std::f32::consts::PI;
use std::time::{Duration, Instant};

use slime_assets::{AssetId, CookedAssets};
use slime_color::LinearColor;
use slime_gfx::{Gpu, RenderTarget};
use slime_render::glam::{DVec3, Vec3};
use slime_render::{
    Camera, DirectionalLight, Environment, Light, LoadedScene, RenderError, RenderStats, Renderer,
    Scene, SceneLoad, ShaderSources, Sky, View, ViewDraw,
};

/// The ID of the sample asset, `examples/assets/scene.glb`, made in Blender by
/// `examples/assets/make_scene.py`: the ID of its default scene too, which `scene.glb.meta` holds.
pub const SCENE: AssetId = AssetId::from_u128(0x850ad551_5098_42f9_ae42_26ed6174951a);

/// The smallest box around a loaded scene, in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: DVec3,
    pub max: DVec3,
}

impl Bounds {
    /// The center of the box.
    pub fn center(&self) -> DVec3 {
        (self.min + self.max) / 2.0
    }

    /// Half the diagonal of the box: the radius of the sphere around it.
    pub fn radius(&self) -> f32 {
        ((self.max - self.min).length() / 2.0) as f32
    }
}

/// The illuminance of the sample's sun, in lux: direct sunlight. The asset's sun has it too
/// (`make_scene.py`).
const SUN_ILLUMINANCE: f32 = 100_000.0;

/// The exposure the sample is drawn at: under it, a white matte surface facing the sun reflects
/// 0.97 (ADR-0035), shown at sRGB 239. A photographer's exposure value of 14.7 at ISO 100, near the
/// 15 of the "sunny 16" rule for direct sunlight.
pub const EXPOSURE: f32 = PI / SUN_ILLUMINANCE;

/// The share of the exposed light that the sample's bloom spreads around where it lands (ADR-0040):
/// the sun's disc glows over about a third of the screen, and the ring, four times as bright as
/// sunlit white, faintly.
pub const BLOOM: f32 = 0.04;

/// A clear day's sky around the sun, in nits: a blue zenith of luminance 3 900 and a whiter horizon
/// of 9 400, and dark soil below them, of albedo about 0.1, lit by the sun 52° high and by the sky:
/// 0.1 × (100 000 × sin 52° + 18 000) / π ≈ 3 100. Once exposed: (0.053, 0.123, 0.314),
/// (0.239, 0.302, 0.383) and (0.097, 0.093, 0.085). Without its sun, which [`SampleScene`] gives
/// it from the file's lights.
pub const SKY: Sky = Sky {
    zenith: LinearColor::rgb(1700.0, 3900.0, 10_000.0),
    horizon: LinearColor::rgb(7600.0, 9600.0, 12_200.0),
    ground: LinearColor::rgb(3100.0, 2950.0, 2700.0),
    sun: None,
};

/// The sun that lights a file without lights: slightly warm, from above. The asset's sun is the
/// same light. Its shadow distance is set each frame (see [`SampleScene::render`]).
const SUN: DirectionalLight = DirectionalLight {
    direction: Vec3::new(-0.5, -1.0, -0.6),
    color: LinearColor::rgb(1.0, 0.98, 0.9),
    illuminance: SUN_ILLUMINANCE,
    casts_shadows: true,
    shadow_distance: 50.0,
};

/// A loaded cooked scene, its sky and the renderer that draws them, for targets of one format.
pub struct SampleScene {
    renderer: Renderer,
    /// What drawing the scene keeps from one frame to the next.
    view: View,
    loaded: LoadedScene,
    bounds: Bounds,
    /// The sun of its sky, which `environment` was computed around.
    sun: Option<DirectionalLight>,
    environment: Environment,
    environment_time: Duration,
}

impl SampleScene {
    /// The cooked scene `loaded`, which `renderer` loaded (`Renderer::load_scene`, or
    /// `Renderer::load_scene_in_background` as the `scene` example does), drawn by `renderer`; its
    /// sky is computed here.
    ///
    /// # Errors
    ///
    /// The renderer's error when the sky cannot be created; an error when the scene places no mesh.
    pub fn new(
        gpu: &Gpu,
        renderer: Renderer,
        loaded: LoadedScene,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let (bounds, sun) = framing(&loaded)?;
        let (environment, environment_time) = sky(gpu, sun)?;
        let view = renderer.create_view(gpu, "sample scene")?;
        Ok(Self {
            renderer,
            view,
            loaded,
            bounds,
            sun,
            environment,
            environment_time,
        })
    }

    /// Starts loading the scene `id` from `assets` again, whose cooked data changed, keeping what
    /// did not change of the scene drawn (`Renderer::reload_scene_in_background`).
    pub fn reload(&self, gpu: &Gpu, assets: &CookedAssets, id: AssetId) -> SceneLoad {
        self.renderer
            .reload_scene_in_background(gpu, assets, id, &self.loaded)
    }

    /// Draws `loaded`, the scene loaded again, from now on, under its sky, computed again if its sun
    /// moved.
    ///
    /// # Errors
    ///
    /// As [`SampleScene::new`]; the scene drawn before stays then.
    pub fn replace(
        &mut self,
        gpu: &Gpu,
        loaded: LoadedScene,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (bounds, sun) = framing(&loaded)?;
        if sun != self.sun {
            (self.environment, self.environment_time) = sky(gpu, sun)?;
            self.sun = sun;
        }
        self.bounds = bounds;
        self.loaded = loaded;
        Ok(())
    }

    /// Makes the renderer's pipelines again from `sources` (`Renderer::set_shaders`), as the
    /// `scene` example does with `--watch`.
    ///
    /// # Errors
    ///
    /// As `Renderer::set_shaders`: the scene is drawn as before then.
    pub fn set_shaders(&mut self, gpu: &Gpu, sources: &ShaderSources) -> Result<(), RenderError> {
        self.renderer.set_shaders(gpu, sources)
    }

    /// How long the CPU took to compute the sky's environment, when the scene was loaded.
    pub fn environment_time(&self) -> Duration {
        self.environment_time
    }

    /// The smallest box around the scene, which the camera orbits.
    pub fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    /// Draws the scene into `target` as `camera` sees it, its light multiplied by `exposure`
    /// ([`EXPOSURE`] for the sample), with a bloom of strength `bloom` ([`BLOOM`] for the sample)
    /// mixed in before tone mapping. The directional lights cast shadows as far as the far side of
    /// the scene's bounds from the camera, and no farther: the cascades cover the scene and
    /// nothing beyond it, as a game sets the distance for its own scenes (ADR-0050).
    ///
    /// # Errors
    ///
    /// As [`Renderer::render`].
    pub fn render(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        camera: Camera,
        exposure: f32,
        bloom: f32,
    ) -> Result<RenderStats, RenderError> {
        let instances = self.loaded.instances();
        let sun = [Light::Directional(SUN)];
        let lights = if self.loaded.lights.is_empty() {
            &sun[..]
        } else {
            &self.loaded.lights[..]
        };
        let bounds = &self.bounds;
        let reach = camera.eye.distance(bounds.center()) as f32 + bounds.radius();
        let lights: Vec<Light> = lights
            .iter()
            .map(|&light| match light {
                Light::Directional(light) => Light::Directional(DirectionalLight {
                    shadow_distance: reach,
                    ..light
                }),
                other => other,
            })
            .collect();
        let mut scene = Scene::new(instances.clone());
        scene.lights = lights.to_vec();
        scene.environment = Some(self.environment.clone());
        scene.exposure = exposure;
        scene.bloom = bloom;
        self.renderer.render(
            gpu,
            &scene,
            &mut [ViewDraw::new(&mut self.view, camera, target)],
        )?;
        Ok(self.view.stats())
    }
}

/// The box around `loaded`, and the sun of its sky.
///
/// # Errors
///
/// When the scene places no mesh.
fn framing(
    loaded: &LoadedScene,
) -> Result<(Bounds, Option<DirectionalLight>), Box<dyn Error + Send + Sync>> {
    let (min, max) = loaded
        .bounds
        .ok_or("the scene draws nothing: no node places a mesh")?;
    let bounds = Bounds { min, max };
    // The sky's sun is the scene's: the file's first directional light, or the sample's sun
    // for a file without lights, so that its disc and glow lie where the light and its shadows
    // come from.
    let sun = loaded
        .lights
        .iter()
        .find_map(|light| match light {
            Light::Directional(light) => Some(*light),
            _ => None,
        })
        .or(loaded.lights.is_empty().then_some(SUN));
    Ok((bounds, sun))
}

/// The environment of the sky around `sun`, and the time the CPU took to compute it.
///
/// # Errors
///
/// [`Environment::new`]'s error when the sky cannot be created.
fn sky(gpu: &Gpu, sun: Option<DirectionalLight>) -> Result<(Environment, Duration), RenderError> {
    let started = Instant::now();
    let environment = Environment::new(gpu, &Sky { sun, ..SKY })?;
    let environment_time = started.elapsed();
    tracing::info!(?environment_time, "the sky's environment is computed");
    Ok((environment, environment_time))
}
