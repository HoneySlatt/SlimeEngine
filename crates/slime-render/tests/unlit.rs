//! Unlit materials (`docs/adr/0129-drawing-for-the-editor.md`), checked by reading pixels back
//! through the tone-mapping curve of the renderer: an unlit surface shows its color, times its
//! texture, whatever lights the scene and whatever its exposure; a blended one is laid over what
//! lies behind it, the sky included, and hidden by what lies in front of it; a masked one discards
//! what its cutoff discards; a mirrored one shows its normals facing the camera; it casts no shadow; its parameters are reflected, and keep its alpha mode; the
//! physically based kind refuses blending. Runs on whatever adapter the machine offers.

#[path = "common/tone_curve.rs"]
mod tone_curve;

use slime_color::LinearColor;
use slime_gfx::{AddressMode, Filter, Gpu, Image, SamplerDescriptor, TextureFormat};
use slime_reflect::{Reflect, Registry};
use slime_render::glam::{DAffine3, DVec3, Vec3};
use slime_render::{
    AlphaMode, Camera, DebugMode, DirectionalLight, Instance, Light, Material, MaterialData, Mesh,
    MeshData, RenderError, Renderer, SampledTexture, Scene, UnlitData, UnlitValues, ViewDraw,
    ViewSettings,
};

use tone_curve::displayed;

const SIZE: u32 = 16;

/// The red, green and blue of the pixel at `(x, y)`, `y` counted from the top.
fn pixel(image: &Image, (x, y): (u32, u32)) -> [u8; 3] {
    let start = ((y * image.width + x) * 4) as usize;
    image.pixels[start..start + 3]
        .try_into()
        .expect("three channels")
}

/// Whether `shown` is `expected` within one step of each channel's rounding.
fn near(shown: [u8; 3], expected: [u8; 3]) -> bool {
    shown
        .iter()
        .zip(expected)
        .all(|(&shown, expected)| shown.abs_diff(expected) <= 1)
}

/// A renderer of an 8-bit sRGB output, and the square it draws, one unit wide, facing +Z.
struct Drawing {
    gpu: Gpu,
    renderer: Renderer,
    square: Mesh,
}

impl Drawing {
    fn new() -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let renderer =
            Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer is created");
        let square = renderer
            .create_mesh(&gpu, "square", &MeshData::square())
            .expect("the square is valid");
        Self {
            gpu,
            renderer,
            square,
        }
    }

    fn unlit(&self, label: &str, data: &UnlitData<'_>) -> Material {
        self.renderer
            .create_unlit_material(&self.gpu, label, data)
            .expect("the material is valid")
    }

    /// The square of `material`, two units wide, at `z` along the camera's axis.
    fn square(&self, material: &Material, z: f64) -> Instance {
        Instance::new(
            &self.square,
            material,
            DAffine3::from_translation(DVec3::new(0.0, 0.0, z))
                * DAffine3::from_scale(DVec3::splat(2.0)),
        )
    }

    /// `scene` seen from 3 units along +Z, the square filling the image's middle.
    fn draw(&mut self, scene: &Scene) -> Image {
        self.draw_with(scene, ViewSettings::default())
    }

    /// The same, through a view of `settings`.
    fn draw_with(&mut self, scene: &Scene, settings: ViewSettings) -> Image {
        let target = self
            .gpu
            .create_render_target("unlit", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb)
            .expect("the GPU has memory for it");
        let mut view = self
            .renderer
            .create_view(&self.gpu, "view")
            .expect("the view is created");
        view.set_settings(settings);
        let camera = Camera {
            eye: DVec3::new(0.0, 0.0, 3.0),
            ..Camera::default()
        };
        self.renderer
            .render(
                &self.gpu,
                scene,
                &mut [ViewDraw::new(&mut view, camera, &target)],
            )
            .expect("the frame is drawn");
        self.gpu
            .read_render_target(&target)
            .expect("the image is read back")
    }
}

/// The middle of the image, on the square.
const MIDDLE: (u32, u32) = (SIZE / 2, SIZE / 2);

#[test]
fn an_unlit_surface_shows_its_color_whatever_the_light_and_the_exposure() {
    let mut drawing = Drawing::new();
    let color = [0.5, 0.25, 0.1];
    let material = drawing.unlit(
        "orange",
        &UnlitData {
            color: LinearColor::rgb(color[0], color[1], color[2]),
            ..UnlitData::default()
        },
    );
    let expected = displayed(color);
    let sun = Light::Directional(DirectionalLight {
        direction: Vec3::new(0.0, 0.0, -1.0),
        illuminance: 100_000.0,
        ..DirectionalLight::default()
    });
    for (lights, exposure) in [(vec![], 1.0), (vec![sun], 1.0), (vec![sun], 1.0 / 8_000.0)] {
        let mut scene = Scene::new([drawing.square(&material, 0.0)]);
        scene.lights = lights;
        scene.exposure = exposure;
        let shown = pixel(&drawing.draw(&scene), MIDDLE);
        assert!(
            near(shown, expected),
            "{shown:?} where {expected:?}, exposure {exposure}"
        );
    }
}

#[test]
fn an_unlit_surface_s_texture_multiplies_its_color_and_its_mask_cuts_it_out() {
    let mut drawing = Drawing::new();
    // White on the left, cut out (alpha 0) on the right.
    let texture = drawing
        .gpu
        .create_texture(
            "half",
            2,
            1,
            TextureFormat::Rgba8UnormSrgb,
            &[255, 255, 255, 255, 255, 255, 255, 0],
        )
        .expect("the GPU has memory for it");
    let sampler = drawing
        .gpu
        .create_sampler(
            "nearest",
            &SamplerDescriptor {
                mag_filter: Filter::Nearest,
                min_filter: Filter::Nearest,
                address_u: AddressMode::ClampToEdge,
                address_v: AddressMode::ClampToEdge,
            },
        )
        .expect("the GPU has memory for it");
    let behind = drawing.unlit(
        "blue",
        &UnlitData {
            color: LinearColor::rgb(0.0, 0.0, 0.5),
            ..UnlitData::default()
        },
    );
    let masked = drawing.unlit(
        "masked",
        &UnlitData {
            color: LinearColor::rgb(0.5, 0.0, 0.0),
            texture: Some(SampledTexture {
                texture: &texture,
                sampler: &sampler,
            }),
            alpha_mode: AlphaMode::Mask { cutoff: 0.5 },
            ..UnlitData::default()
        },
    );
    let image = drawing.draw(&Scene::new([
        drawing.square(&behind, -1.0),
        drawing.square(&masked, 0.0),
    ]));
    // On the square's left half, and on its right half, in front of the blue square, smaller.
    let (left, right) = ((SIZE / 2 - 2, SIZE / 2), (SIZE / 2 + 1, SIZE / 2));
    for (at, color) in [(left, [0.5, 0.0, 0.0]), (right, [0.0, 0.0, 0.5])] {
        let (shown, expected) = (pixel(&image, at), displayed(color));
        assert!(
            near(shown, expected),
            "{shown:?} where {expected:?} at {at:?}"
        );
    }
}

#[test]
fn a_blended_unlit_surface_lies_over_what_is_behind_it_and_under_what_is_in_front() {
    let mut drawing = Drawing::new();
    let red = drawing.unlit(
        "red",
        &UnlitData {
            color: LinearColor::rgb(0.6, 0.0, 0.0),
            ..UnlitData::default()
        },
    );
    let glass = drawing.unlit(
        "half blue",
        &UnlitData {
            color: LinearColor::rgba(0.0, 0.0, 0.6, 0.5),
            alpha_mode: AlphaMode::Blend,
            ..UnlitData::default()
        },
    );
    // In front of the red square, half of each; behind it, the red alone, whichever is listed
    // first: the blended surfaces are drawn last.
    let in_front = drawing.draw(&Scene::new([
        drawing.square(&glass, 0.0),
        drawing.square(&red, -1.0),
    ]));
    let shown = pixel(&in_front, MIDDLE);
    let expected = displayed([0.3, 0.0, 0.3]);
    assert!(near(shown, expected), "{shown:?} where {expected:?}");
    let behind = drawing.draw(&Scene::new([
        drawing.square(&glass, -1.0),
        drawing.square(&red, 0.0),
    ]));
    let shown = pixel(&behind, MIDDLE);
    let expected = displayed([0.6, 0.0, 0.0]);
    assert!(near(shown, expected), "{shown:?} where {expected:?}");
}

/// Drawn after the sky, which would otherwise cover it where nothing lies behind it: it writes no
/// depth.
#[test]
fn a_blended_unlit_surface_lies_over_the_sky() {
    let mut drawing = Drawing::new();
    let glass = drawing.unlit(
        "half blue",
        &UnlitData {
            color: LinearColor::rgba(0.0, 0.0, 0.6, 0.5),
            alpha_mode: AlphaMode::Blend,
            ..UnlitData::default()
        },
    );
    // Without an environment, the sky is black.
    let shown = pixel(
        &drawing.draw(&Scene::new([drawing.square(&glass, 0.0)])),
        MIDDLE,
    );
    let expected = displayed([0.0, 0.0, 0.3]);
    assert!(near(shown, expected), "{shown:?} where {expected:?}");
}

/// Mirrored, its front faces wind clockwise on screen: its normals, shown as colors, still face the
/// camera.
#[test]
fn a_mirrored_unlit_surface_shows_its_normals_facing_the_camera() {
    let mut drawing = Drawing::new();
    let material = drawing.unlit("mirrored", &UnlitData::default());
    let mirrored = Instance::new(
        &drawing.square,
        &material,
        DAffine3::from_scale(DVec3::new(-2.0, 2.0, 2.0)),
    );
    let mut settings = ViewSettings::default();
    settings.debug = DebugMode::Normals;
    let shown = pixel(
        &drawing.draw_with(&Scene::new([mirrored]), settings),
        MIDDLE,
    );
    // (0, 0, 1), as (0.5, 0.5, 1), sRGB-encoded.
    assert!(near(shown, [188, 188, 255]), "{shown:?}");
}

#[test]
fn an_unlit_surface_casts_no_shadow() {
    let mut drawing = Drawing::new();
    let floor = drawing
        .renderer
        .create_material(&drawing.gpu, "floor", &MaterialData::default())
        .expect("the material is valid");
    let unlit = drawing.unlit("between", &UnlitData::default());
    let lit_square = drawing
        .renderer
        .create_material(&drawing.gpu, "lit", &MaterialData::default())
        .expect("the material is valid");
    // From the left, slanting: a square that casts a shadow casts it on the floor, half a unit
    // behind it and half a unit to its right.
    let sun = Light::Directional(DirectionalLight {
        direction: Vec3::new(1.0, 0.0, -1.0).normalize(),
        illuminance: 3.0,
        casts_shadows: true,
        ..DirectionalLight::default()
    });
    let lit = |instances: Vec<Instance>| {
        let mut scene = Scene::new(instances);
        scene.lights = vec![sun];
        scene
    };
    let wide =
        DAffine3::from_translation(DVec3::Z * -0.5) * DAffine3::from_scale(DVec3::splat(8.0));
    let floor = Instance::new(&drawing.square, &floor, wide);
    // Just right of the square, on the floor: 1.25 units right, 3.5 units away.
    let beside = (13, SIZE / 2);
    let alone = pixel(&drawing.draw(&lit(vec![floor.clone()])), beside);
    let behind_lit = drawing.draw(&lit(vec![floor.clone(), drawing.square(&lit_square, 0.0)]));
    assert!(
        pixel(&behind_lit, beside)[0] < alone[0] / 2,
        "a lit square's shadow shows there"
    );
    let behind_unlit = drawing.draw(&lit(vec![floor, drawing.square(&unlit, 0.0)]));
    assert_eq!(pixel(&behind_unlit, beside), alone);
}

#[test]
fn an_unlit_material_s_parameters_are_reflected_and_keep_its_alpha_mode() {
    let drawing = Drawing::new();
    let mut registry = Registry::new();
    assert_eq!(
        drawing.renderer.describe_materials(&mut registry),
        [
            ("physically based", "MaterialValues".to_owned()),
            ("unlit", "UnlitValues".to_owned()),
        ]
    );
    let material = drawing.unlit("reflected", &UnlitData::default());
    assert_eq!(material.kind(), "unlit");
    let green = UnlitValues {
        color: LinearColor::rgb(0.0, 1.0, 0.0),
        alpha_mode: AlphaMode::Opaque,
    };
    material
        .set_parameters(&drawing.gpu, &green.to_value())
        .expect("the material takes them");
    assert_eq!(material.parameters(), green.to_value());
    for (values, reason) in [
        (
            UnlitValues {
                alpha_mode: AlphaMode::Blend,
                ..green
            },
            "its alpha mode stays the one it was made with",
        ),
        (
            UnlitValues {
                color: LinearColor::rgb(0.0, 2.0, 0.0),
                ..green
            },
            "green 2",
        ),
    ] {
        let Err(error) = material.set_parameters(&drawing.gpu, &values.to_value()) else {
            panic!("{values:?} taken");
        };
        assert!(error.to_string().contains(reason), "{error}");
    }
    // A physically based material is not blended until Stage 9a.
    let Err(error) = drawing.renderer.create_material(
        &drawing.gpu,
        "blended",
        &MaterialData {
            alpha_mode: AlphaMode::Blend,
            ..MaterialData::default()
        },
    ) else {
        panic!("a blended physically based material made");
    };
    assert!(
        matches!(error, RenderError::InvalidMaterial { .. }),
        "{error}"
    );
}
