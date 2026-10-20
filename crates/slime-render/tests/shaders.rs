//! The renderer's shaders given while it runs (ADR-0077): the sources of the shaders' files,
//! which equal the embedded ones, draw the same pixels, and other sources the next frame; a shader
//! that does not compile, or that reads more values than the renderer gives it, leaves every
//! pipeline as it was, and says why. Prints how long making the pipelines again takes, which CI
//! shows on Direct3D 12. Runs on whatever adapter the machine offers.

use std::path::Path;
use std::time::Instant;

use slime_color::LinearColor;
use slime_gfx::{GfxError, Gpu, RenderTarget, TextureFormat};
use slime_render::glam::{DAffine3, DVec3, Vec3};
use slime_render::{
    Camera, DirectionalLight, Environment, Instance, Light, Material, MaterialData, Mesh, MeshData,
    RenderError, Renderer, Scene, ShaderSources, Sky,
};

const SIZE: u32 = 64;

/// A sphere lit by a sun under a sky, drawn with every pass: shadows, scene, sky, resolve, bloom
/// and tone mapping.
struct Setup {
    gpu: Gpu,
    renderer: Renderer,
    target: RenderTarget,
    sphere: Mesh,
    material: Material,
    environment: Environment,
}

impl Setup {
    fn new() -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let renderer =
            Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer is created");
        let target = gpu
            .create_render_target("shaders", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb)
            .expect("the GPU has memory for it");
        let sphere = renderer
            .create_mesh(&gpu, "sphere", &MeshData::sphere(16, 32))
            .expect("the sphere is valid");
        let material = renderer
            .create_material(
                &gpu,
                "orange",
                &MaterialData {
                    base_color: LinearColor::rgb(0.8, 0.4, 0.1),
                    roughness: 0.4,
                    ..MaterialData::default()
                },
            )
            .expect("the GPU has memory for it");
        let environment = renderer
            .create_environment(
                &gpu,
                &Sky {
                    zenith: LinearColor::rgb(0.2, 0.3, 0.8),
                    horizon: LinearColor::rgb(0.7, 0.7, 0.7),
                    ground: LinearColor::rgb(0.1, 0.1, 0.1),
                    sun: None,
                },
            )
            .expect("the GPU has memory for it");
        Self {
            gpu,
            renderer,
            target,
            sphere,
            material,
            environment,
        }
    }

    /// The pixels of the scene, drawn by the renderer as its pipelines are.
    fn drawn(&mut self) -> Vec<u8> {
        let instances = [Instance::new(
            &self.sphere,
            &self.material,
            DAffine3::IDENTITY,
        )];
        let lights = [Light::Directional(DirectionalLight {
            direction: Vec3::new(-0.3, -1.0, -0.4),
            illuminance: 3.0,
            ..DirectionalLight::default()
        })];
        let camera = Camera {
            eye: DVec3::new(0.0, 0.5, 2.5),
            ..Camera::default()
        };
        let mut scene = Scene::new(camera, &instances);
        scene.lights = &lights;
        scene.environment = Some(&self.environment);
        scene.bloom = 0.04;
        self.renderer
            .render(&self.gpu, &self.target, &scene)
            .expect("the frame is drawn");
        self.gpu
            .read_render_target(&self.target)
            .expect("the image is read back")
            .pixels
    }
}

#[test]
fn the_sources_of_the_files_draw_as_the_embedded_ones_do() {
    let mut setup = Setup::new();
    let before = setup.drawn();
    let sources = ShaderSources::read(Path::new(ShaderSources::DIRECTORY)).expect("read");
    let started = Instant::now();
    setup
        .renderer
        .set_shaders(&setup.gpu, &sources)
        .expect("the shaders compile");
    // What a game waits for when it gives the renderer new shaders, at one frame.
    eprintln!(
        "the renderer's pipelines made again in {:?} on {}",
        started.elapsed(),
        setup.gpu.adapter_name()
    );
    assert!(setup.drawn() == before, "the same pixels");
}

/// The embedded sources, but the tone-mapping pass's output inverted: a change that every pixel
/// shows.
fn inverted() -> ShaderSources {
    let embedded = ShaderSources::embedded();
    let output = "return vec4<f32>(color, 1.0);";
    assert!(embedded.tone_mapping.contains(output));
    ShaderSources {
        tone_mapping: embedded
            .tone_mapping
            .replace(output, "return vec4<f32>(1.0 - color, 1.0);"),
        ..embedded
    }
}

#[test]
fn new_sources_draw_the_next_frame() {
    let mut setup = Setup::new();
    let before = setup.drawn();
    setup
        .renderer
        .set_shaders(&setup.gpu, &inverted())
        .expect("the shaders compile");
    assert!(setup.drawn() != before, "the change shows");
}

#[test]
fn a_shader_that_does_not_compile_leaves_every_pipeline_as_it_was() {
    let mut setup = Setup::new();
    setup
        .renderer
        .set_shaders(&setup.gpu, &inverted())
        .expect("the shaders compile");
    let before = setup.drawn();
    // The embedded tone mapping, which would draw other pixels if it replaced the inverted one
    // alone, and a statement without its value on the third line of the bloom's shader.
    let embedded = ShaderSources::embedded();
    let broken = ShaderSources {
        bloom: format!("\n\nfn broken() {{ let x = ; }}\n{}", embedded.bloom),
        ..embedded
    };
    let error = setup
        .renderer
        .set_shaders(&setup.gpu, &broken)
        .expect_err("refused");
    let RenderError::Gfx(GfxError::Shader { label, message }) = &error else {
        panic!("{error:?}");
    };
    assert_eq!(label, "bloom.wgsl");
    assert!(message.contains("wgsl:3:"), "its line: {message}");
    assert!(setup.drawn() == before, "drawn as before, inverted");
}

#[test]
fn a_shader_that_reads_more_values_than_it_is_given_is_refused_with_its_pipelines() {
    let mut setup = Setup::new();
    let before = setup.drawn();
    let embedded = ShaderSources::embedded();
    // Each instance's values, larger than the renderer's. Matched without the line's end, which a
    // checkout on Windows turns into CRLF.
    let instance = "struct Instance {";
    assert!(embedded.scene.contains(instance));
    let larger = ShaderSources {
        scene: embedded.scene.replace(
            instance,
            &format!("{instance}\n    more: array<vec4<f32>, 4>,"),
        ),
        ..embedded
    };
    let error = setup
        .renderer
        .set_shaders(&setup.gpu, &larger)
        .expect_err("refused");
    assert!(
        matches!(error, RenderError::Gfx(GfxError::Pipeline { .. })),
        "{error:?}"
    );
    assert!(setup.drawn() == before, "drawn as before");
}
