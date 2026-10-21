//! Materials, checked by reading pixels back through the tone-mapping curve of the renderer
//! (ADR-0034): a base color texture is read at the texture coordinates of the mesh and multiplied
//! with the base color, and an opaque material ignores the alpha of its base color (masks are in
//! `shading.rs`); meshes, materials and environments are drawn by any renderer of their GPU, not
//! only the one that made them; a material changed in place draws as one made with its new values,
//! and refuses the values that its creation refuses; a material is of a kind, whose parameters
//! reflection describes and changes. Runs on whatever adapter the machine offers.

#[path = "common/tone_curve.rs"]
mod tone_curve;

use std::f32::consts::PI;

use slime_color::LinearColor;
use slime_gfx::{AddressMode, Filter, Gpu, Image, SamplerDescriptor, TextureFormat};
use slime_reflect::{Path, Reflect, Registry, Value};
use slime_render::glam::{DAffine3, DVec3, Vec3};
use slime_render::{
    AlphaMode, Camera, DirectionalLight, Environment, Instance, Light, Material, MaterialData,
    MaterialValues, MeshData, RenderError, Renderer, SampledTexture, Scene,
};

const SIZE: u32 = 16;

/// The red, green, blue and alpha of the pixel at `(x, y)`, `y` counted from the top.
fn pixel(image: &Image, (x, y): (u32, u32)) -> [u8; 4] {
    let start = ((y * image.width + x) * 4) as usize;
    image.pixels[start..start + 4]
        .try_into()
        .expect("four channels")
}

#[test]
fn the_base_color_texture_is_read_at_the_texture_coordinates_and_tints_the_base_color() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let target = gpu
        .create_render_target("materials", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    let mut renderer = Renderer::new(&gpu, target.format()).expect("the renderer is created");
    // A square facing +Z, with the texture's top left at its top left, drawn two units wide.
    let square = renderer
        .create_mesh(&gpu, "square", &MeshData::square())
        .expect("the square is valid");
    // Red and green on the top row, blue and white below.
    let texture = gpu
        .create_texture(
            "quarters",
            2,
            2,
            TextureFormat::Rgba8UnormSrgb,
            &[
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ],
        )
        .expect("the GPU has memory for it");
    let sampler = gpu
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
    let sampled = SampledTexture {
        texture: &texture,
        sampler: &sampler,
    };
    let plain = renderer
        .create_material(
            &gpu,
            "textured",
            &MaterialData {
                base_color_texture: Some(sampled),
                ..MaterialData::default()
            },
        )
        .expect("the GPU has memory for it");
    let dimmed = renderer
        .create_material(
            &gpu,
            "textured and dimmed",
            &MaterialData {
                base_color: LinearColor::rgb(0.5, 0.5, 0.5),
                base_color_texture: Some(sampled),
                ..MaterialData::default()
            },
        )
        .expect("the GPU has memory for it");
    // Lit straight on by a light of π, without an environment: a matte surface seen from the light
    // reflects 0.96 of its color, diffused, and 0.01, reflected by its specular layer (ADR-0035).
    let light = Light::Directional(DirectionalLight {
        direction: Vec3::new(0.0, 0.0, -1.0),
        color: LinearColor::WHITE,
        illuminance: PI,
        ..DirectionalLight::default()
    });
    let camera = Camera {
        eye: DVec3::new(0.0, 0.0, 3.0),
        ..Camera::default()
    };

    // Half transparent, which an opaque surface ignores.
    let translucent = renderer
        .create_material(
            &gpu,
            "textured and translucent",
            &MaterialData {
                base_color: LinearColor::rgba(1.0, 1.0, 1.0, 0.5),
                base_color_texture: Some(sampled),
                ..MaterialData::default()
            },
        )
        .expect("the GPU has memory for it");

    // The pixels at 5 and 10 are inside the square's quarters, away from the texture's edges:
    // red, green, blue and white texels, lit fully, so that what shows is the tone-mapped light
    // that the texel times the base color reflects. With a white base color, about (240, 29, 29)
    // for each primary and 239 for white; with half of it, (184, 2, 2) and 179.
    let quarters: [[f32; 3]; 4] = [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
    ];
    let shown = |base: f32| {
        quarters.map(|texel| {
            let [r, g, b] =
                tone_curve::displayed(texel.map(|channel| channel * base * 0.96 + 0.01));
            [r, g, b, 255]
        })
    };
    for (material, expected, case) in [
        (&plain, shown(1.0), "white base color"),
        (&dimmed, shown(0.5), "half base color"),
        (&translucent, shown(1.0), "half alpha"),
    ] {
        let instances = [Instance::new(
            &square,
            material,
            DAffine3::from_scale(DVec3::splat(2.0)),
        )];
        let lights = [light];
        let mut scene = Scene::new(camera, instances.clone());
        scene.lights = lights.to_vec();
        renderer
            .render(&gpu, &target, &scene)
            .expect("the frame is drawn");
        let image = gpu
            .read_render_target(&target)
            .expect("the image is read back");
        for (position, expected) in [(5, 5), (10, 5), (5, 10), (10, 10)]
            .into_iter()
            .zip(expected)
        {
            let actual = pixel(&image, position);
            let close = actual
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= 1);
            assert!(
                close,
                "{case} at {position:?}: expected {expected:?}, got {actual:?}"
            );
        }
    }
}

#[test]
fn meshes_materials_and_environments_are_drawn_by_any_renderer_of_their_gpu() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let target = gpu
        .create_render_target("shared", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    let mut first = Renderer::new(&gpu, target.format()).expect("the renderer is created");
    let mut second = Renderer::new(&gpu, target.format()).expect("the renderer is created");
    let triangle = first
        .create_mesh(
            &gpu,
            "triangle",
            &MeshData {
                positions: vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                uvs: vec![[0.0, 1.0], [1.0, 1.0], [0.5, 0.0]],
                tangents: vec![[1.0, 0.0, 0.0, 1.0]; 3],
                indices: vec![0, 1, 2],
            },
        )
        .expect("the triangle is valid");
    let red = first
        .create_material(
            &gpu,
            "red",
            &MaterialData {
                base_color: LinearColor::rgb(1.0, 0.1, 0.1),
                ..MaterialData::default()
            },
        )
        .expect("the GPU has memory for it");
    let sky = Environment::new(
        &gpu,
        &slime_render::Sky {
            zenith: LinearColor::rgb(0.2, 0.3, 0.8),
            horizon: LinearColor::rgb(0.6, 0.6, 0.6),
            ground: LinearColor::rgb(0.1, 0.1, 0.1),
            sun: None,
        },
    )
    .expect("the GPU has memory for it");
    let camera = Camera {
        eye: DVec3::new(0.0, 0.0, 3.0),
        ..Camera::default()
    };
    let instances = [Instance::new(&triangle, &red, DAffine3::IDENTITY)];
    let mut scene = Scene::new(camera, instances.clone());
    scene.environment = Some(sky.clone());

    let [by_first, by_second] = [&mut first, &mut second].map(|renderer| {
        renderer
            .render(&gpu, &target, &scene)
            .expect("the frame is drawn");
        gpu.read_render_target(&target)
            .expect("the image is read back")
    });
    // The triangle's middle shows its material, redder than the grey and blue sky around it.
    let [r, g, _, _] = pixel(&by_first, (8, 9));
    assert!(r > g + 40, "the triangle is drawn red: {r}, {g}");
    assert!(
        by_first.pixels == by_second.pixels,
        "the second renderer draws what the first made as the first does"
    );
}

/// Data made of `values` and of `textures`' base color and normal textures.
fn with_values<'a>(
    values: MaterialValues,
    textures: (SampledTexture<'a>, SampledTexture<'a>),
) -> MaterialData<'a> {
    MaterialData {
        base_color: values.base_color,
        base_color_texture: Some(textures.0),
        metallic: values.metallic,
        roughness: values.roughness,
        normal_texture: Some(textures.1),
        normal_scale: values.normal_scale,
        occlusion_strength: values.occlusion_strength,
        emissive: values.emissive,
        emissive_strength: values.emissive_strength,
        alpha_mode: values.alpha_mode,
        ..MaterialData::default()
    }
}

#[test]
fn a_material_changed_in_place_draws_as_one_made_with_the_new_values() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let target = gpu
        .create_render_target("changed", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    let mut renderer = Renderer::new(&gpu, target.format()).expect("the renderer is created");
    let square = renderer
        .create_mesh(&gpu, "square", &MeshData::square())
        .expect("the square is valid");
    // A grey base color whose alpha fades from left to right, which the masks cut, and a normal
    // tilted towards the tangent, which the normal scales bend by different angles.
    let fading = gpu
        .create_texture(
            "fading",
            2,
            1,
            TextureFormat::Rgba8UnormSrgb,
            &[200, 200, 200, 255, 200, 200, 200, 60],
        )
        .expect("the GPU has memory for it");
    let tilted = gpu
        .create_texture(
            "tilted",
            1,
            1,
            TextureFormat::Rgba8Unorm,
            &[191, 128, 238, 255],
        )
        .expect("the GPU has memory for it");
    let sampler = gpu
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
    let sampled = |texture| SampledTexture {
        texture,
        sampler: &sampler,
    };
    let textures = (sampled(&fading), sampled(&tilted));
    let made = MaterialValues {
        base_color: LinearColor::rgb(1.0, 0.8, 0.6),
        metallic: 0.0,
        roughness: 1.0,
        normal_scale: 1.0,
        occlusion_strength: 1.0,
        emissive: LinearColor::BLACK,
        emissive_strength: 1.0,
        alpha_mode: AlphaMode::Mask { cutoff: 0.1 },
    };
    let changed_to = MaterialValues {
        base_color: LinearColor::rgba(0.3, 0.6, 0.9, 1.0),
        metallic: 0.5,
        roughness: 0.4,
        normal_scale: 0.5,
        occlusion_strength: 0.5,
        emissive: LinearColor::rgb(0.2, 0.1, 0.0),
        emissive_strength: 2.0,
        alpha_mode: AlphaMode::Mask { cutoff: 0.5 },
    };
    let changed = renderer
        .create_material(&gpu, "changed", &with_values(made, textures))
        .expect("the GPU has memory for it");
    let fresh = renderer
        .create_material(&gpu, "made so", &with_values(changed_to, textures))
        .expect("the GPU has memory for it");
    let light = [Light::Directional(DirectionalLight {
        direction: Vec3::new(0.3, -0.2, -1.0),
        color: LinearColor::WHITE,
        illuminance: PI,
        ..DirectionalLight::default()
    })];
    let camera = Camera {
        eye: DVec3::new(0.0, 0.0, 3.0),
        ..Camera::default()
    };
    let mut drawn = |material: &Material| {
        let instances = [Instance::new(
            &square,
            material,
            DAffine3::from_scale(DVec3::splat(2.0)),
        )];
        let mut scene = Scene::new(camera, instances.clone());
        scene.lights = light.to_vec();
        renderer
            .render(&gpu, &target, &scene)
            .expect("the frame is drawn");
        gpu.read_render_target(&target)
            .expect("the image is read back")
            .pixels
    };
    let before = drawn(&changed);
    assert_eq!(changed.values(), made);
    // Through a clone, which shares the material.
    changed
        .clone()
        .set_values(&gpu, changed_to)
        .expect("valid values");
    assert_eq!(changed.values(), changed_to);
    let after = drawn(&changed);
    assert!(before != after, "the change shows");
    assert!(
        after == drawn(&fresh),
        "the material changed draws as the one made with its values"
    );
}

#[test]
fn values_refused_at_creation_are_refused_in_place_too() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let renderer =
        Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer is created");
    let made = MaterialData::default();
    let opaque = renderer
        .create_material(&gpu, "refused", &made)
        .expect("the GPU has memory for it");
    let valid = opaque.values();
    for values in [
        MaterialValues {
            roughness: f32::NAN,
            ..valid
        },
        MaterialValues {
            base_color: LinearColor::rgb(2.0, 1.0, 1.0),
            ..valid
        },
        MaterialValues {
            emissive_strength: -1.0,
            ..valid
        },
    ] {
        let data = MaterialData {
            base_color: values.base_color,
            roughness: values.roughness,
            emissive_strength: values.emissive_strength,
            ..made
        };
        let Err(at_creation) = renderer.create_material(&gpu, "refused", &data) else {
            panic!("created with {values:?}");
        };
        let Err(in_place) = opaque.set_values(&gpu, values) else {
            panic!("changed to {values:?}");
        };
        assert!(
            matches!(at_creation, RenderError::InvalidMaterial { .. }),
            "{at_creation}"
        );
        assert_eq!(at_creation.to_string(), in_place.to_string());
        assert_eq!(opaque.values(), valid, "the material did not change");
    }
    // The alpha mode stays as made: an opaque material takes no cutoff, a mask keeps one.
    let masked = renderer
        .create_material(
            &gpu,
            "masked",
            &MaterialData {
                alpha_mode: AlphaMode::Mask { cutoff: 0.5 },
                ..made
            },
        )
        .expect("the GPU has memory for it");
    for (material, alpha_mode) in [
        (&opaque, AlphaMode::Mask { cutoff: 0.5 }),
        (&masked, AlphaMode::Opaque),
    ] {
        let values = MaterialValues {
            alpha_mode,
            ..material.values()
        };
        let Err(error) = material.set_values(&gpu, values) else {
            panic!("changed to {alpha_mode:?}");
        };
        assert!(error.to_string().contains("alpha mode stays"), "{error}");
    }
    masked
        .set_values(
            &gpu,
            MaterialValues {
                alpha_mode: AlphaMode::Mask { cutoff: 0.8 },
                ..masked.values()
            },
        )
        .expect("a mask's cutoff changes");
}

#[test]
fn a_material_is_of_a_kind_whose_parameters_reflection_describes_and_changes() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let renderer =
        Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer is created");
    let material = renderer
        .create_material(&gpu, "reflected", &MaterialData::default())
        .expect("the GPU has memory for it");
    assert_eq!(material.kind(), "physically based");
    let mut registry = Registry::new();
    assert_eq!(
        renderer.describe_materials(&mut registry),
        [("physically based", "MaterialValues".to_owned())]
    );
    // A field changed by its name, within its range, as an editor does.
    let roughness: Path = "roughness".parse().expect("a path");
    let mut parameters = material.parameters();
    registry
        .set(
            "MaterialValues",
            &mut parameters,
            &roughness,
            Value::F32(0.25),
        )
        .expect("within its range");
    material
        .set_parameters(&gpu, &parameters)
        .expect("the material takes them");
    assert_eq!(material.values().roughness, 0.25);
    assert!(
        registry
            .set(
                "MaterialValues",
                &mut parameters,
                &roughness,
                Value::F32(1.5)
            )
            .is_err(),
        "out of its range"
    );
    let Err(error) = material.set_parameters(&gpu, &Value::Bool(true)) else {
        panic!("a value of another type taken");
    };
    assert!(
        matches!(error, RenderError::InvalidMaterial { .. }),
        "{error}"
    );
    assert_eq!(
        material.values().roughness,
        0.25,
        "the material did not change"
    );
    // A value of the type, whose color its check refuses.
    let red = MaterialValues {
        base_color: LinearColor::rgb(1.5, 0.0, 0.0),
        ..material.values()
    };
    let Err(error) = material.set_parameters(&gpu, &red.to_value()) else {
        panic!("a red above 1 taken");
    };
    assert!(error.to_string().contains("red 1.5"), "{error}");
}
