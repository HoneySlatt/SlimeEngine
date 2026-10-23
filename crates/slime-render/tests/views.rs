//! Views apart from the renderer (`docs/adr/0126-views-apart-from-the-renderer.md`): each view of a
//! frame draws what it draws alone, the shadows that views share included; an orthographic camera
//! draws what is farther as large; the parts of one image drawn side by side make the whole; a view
//! drawn into a rectangle of its output leaves the rest; four views take one cascades' atlas where
//! four renderers took four; a view resized gives its old targets back (and one whose new size does
//! not fit keeps none, in `oversize.rs`); a multisampled output is refused; a view's image is
//! sampled by another pipeline, as an editor's panel shows it. Runs on whatever adapter the machine
//! offers; what is measured in GPU memory, on those whose memory wgpu counts.

#[path = "common/settled.rs"]
mod settled;

use settled::draw_settled;
use slime_color::LinearColor;
use slime_gfx::{
    AddressMode, BindGroupEntry, BindGroupLayoutEntry, BindingResource, BindingType, CullMode,
    Filter, FloatImage, FragmentState, Gpu, RenderPassDescriptor, RenderPipelineDescriptor,
    RenderTarget, SamplerDescriptor, ShaderStages, TextureFormat,
};
use slime_render::glam::{DAffine3, DQuat, DVec3, Vec3};
use slime_render::{
    Camera, DirectionalLight, Environment, ImagePart, Instance, Light, Material, MaterialData,
    Mesh, MeshData, Projection, Renderer, Scene, Sky, SpotLight, View, ViewDraw, Viewport,
};

const FORMAT: TextureFormat = TextureFormat::Rgba16Float;
const MIB: u64 = 1 << 20;

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

fn output(gpu: &Gpu, width: u32, height: u32) -> RenderTarget {
    gpu.create_render_target("output", width, height, FORMAT)
        .expect("the GPU has memory for it")
}

fn view(gpu: &Gpu, renderer: &Renderer) -> View {
    renderer
        .create_view(gpu, "view")
        .expect("the view is created")
}

/// What the scenes of these tests are made of, made by one renderer, which any renderer of the
/// GPU draws.
struct Things {
    square: Mesh,
    cube: Mesh,
    white: Material,
    glow: Material,
    sky: Environment,
}

impl Things {
    fn new(gpu: &Gpu, renderer: &Renderer) -> Self {
        let glow = MaterialData {
            base_color: LinearColor::BLACK,
            emissive: LinearColor::WHITE,
            emissive_strength: 0.5,
            ..MaterialData::default()
        };
        Self {
            square: renderer
                .create_mesh(gpu, "square", &MeshData::square())
                .expect("the square is valid"),
            cube: renderer
                .create_mesh(gpu, "cube", &MeshData::cube())
                .expect("the cube is valid"),
            white: renderer
                .create_material(gpu, "white", &MaterialData::default())
                .expect("the GPU has memory for it"),
            glow: renderer
                .create_material(gpu, "glow", &glow)
                .expect("the GPU has memory for it"),
            sky: Environment::new(
                gpu,
                &Sky {
                    zenith: LinearColor::rgb(0.2, 0.3, 0.6),
                    horizon: LinearColor::rgb(0.5, 0.5, 0.5),
                    ground: LinearColor::rgb(0.2, 0.15, 0.1),
                    sun: None,
                },
            )
            .expect("the GPU has memory for it"),
        }
    }

    /// A floor 8 m wide, with two cubes on it, under a sun and a spot light that both cast
    /// shadows, and the sky: every pass runs.
    fn yard(&self, bloom: f32) -> Scene {
        let flat = DQuat::from_rotation_x(-std::f64::consts::FRAC_PI_2);
        let instances = vec![
            Instance::new(
                &self.square,
                &self.white,
                DAffine3::from_scale_rotation_translation(
                    DVec3::new(8.0, 8.0, 1.0),
                    flat,
                    DVec3::ZERO,
                ),
            ),
            Instance::new(
                &self.cube,
                &self.white,
                DAffine3::from_translation(DVec3::new(0.0, 0.5, 0.0)),
            ),
            Instance::new(
                &self.cube,
                &self.white,
                DAffine3::from_translation(DVec3::new(-2.0, 0.5, 1.5)),
            ),
        ];
        let mut scene = Scene::new(instances);
        scene.lights = vec![
            Light::Directional(DirectionalLight {
                direction: Vec3::new(-0.4, -1.0, -0.3),
                illuminance: 2.0,
                shadow_distance: 12.0,
                ..DirectionalLight::default()
            }),
            Light::Spot(SpotLight {
                position: DVec3::new(1.5, 3.0, 1.5),
                direction: Vec3::new(-0.5, -1.0, -0.5),
                intensity: 6.0,
                outer_cone_angle: 0.7,
                ..SpotLight::default()
            }),
        ];
        scene.environment = Some(self.sky.clone());
        scene.bloom = bloom;
        scene
    }
}

/// A camera at `eye`, looking at the yard's middle.
fn looking_from(eye: DVec3) -> Camera {
    Camera {
        eye,
        target: DVec3::new(0.0, 0.5, 0.0),
        ..Camera::default()
    }
}

/// The largest difference between the channels of two images, and the pixels whose channels
/// differ by more than `tolerance`.
fn difference(first: &FloatImage, second: &FloatImage, tolerance: f32) -> (f32, Vec<(u32, u32)>) {
    assert_eq!((first.width, first.height), (second.width, second.height));
    let mut largest = 0.0_f32;
    let mut pixels = Vec::new();
    for (index, (a, b)) in first
        .pixels
        .chunks(4)
        .zip(second.pixels.chunks(4))
        .enumerate()
    {
        let off = a
            .iter()
            .zip(b)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        largest = largest.max(off);
        if off > tolerance {
            let index = index as u32;
            pixels.push((index % first.width, index / first.width));
        }
    }
    (largest, pixels)
}

/// Draws `scene` through `camera` alone, with a renderer and a view of its own, into a new output
/// of `width` × `height`, and reads it back.
fn alone(gpu: &Gpu, scene: &Scene, camera: Camera, (width, height): (u32, u32)) -> FloatImage {
    let mut renderer = Renderer::new(gpu, FORMAT).expect("the renderer is created");
    let mut view = view(gpu, &renderer);
    let target = output(gpu, width, height);
    renderer
        .render(gpu, scene, &mut [ViewDraw::new(&mut view, camera, &target)])
        .expect("the frame is drawn");
    gpu.read_float_render_target(&target)
        .expect("the image is read back")
}

/// Two views of one frame, from two sides of the yard, each into an output of its own: the first
/// draws the spot light's tile for both, and each draws the sun's cascades for its own camera into
/// the atlas they share, in turn. Each image is the one that its view draws alone, frame after
/// frame, up to the rounding of positions relative to another camera's eye in the spot light's
/// tile.
#[test]
fn each_view_of_a_frame_draws_what_it_draws_alone() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let things = Things::new(&gpu, &renderer);
    let scene = things.yard(0.04);
    let cameras = [
        looking_from(DVec3::new(4.0, 3.0, 5.0)),
        looking_from(DVec3::new(-5.0, 2.0, -3.0)),
    ];
    let sizes = [(96, 64), (48, 48)];
    let expected = [
        alone(&gpu, &scene, cameras[0], sizes[0]),
        alone(&gpu, &scene, cameras[1], sizes[1]),
    ];
    let targets = sizes.map(|(width, height)| output(&gpu, width, height));
    let (mut first, mut second) = (view(&gpu, &renderer), view(&gpu, &renderer));
    for frame in 0..2 {
        renderer
            .render(
                &gpu,
                &scene,
                &mut [
                    ViewDraw::new(&mut first, cameras[0], &targets[0]),
                    ViewDraw::new(&mut second, cameras[1], &targets[1]),
                ],
            )
            .expect("the frame is drawn");
        for (index, target) in targets.iter().enumerate() {
            let drawn = gpu
                .read_float_render_target(target)
                .expect("the image is read back");
            let (largest, differ) = difference(&drawn, &expected[index], 2e-3);
            assert!(
                differ.is_empty(),
                "frame {frame}, view {index}: pixels {differ:?} differ, by up to {largest}"
            );
        }
    }
    let stats = [first.stats(), second.stats()];
    assert!(stats[0].spot_shadow_draw_calls > 0, "{stats:?}");
    assert_eq!(
        stats[1].spot_shadow_draw_calls, 0,
        "drawn once, by the first"
    );
    assert!(
        stats.iter().all(|stats| stats.shadow_draw_calls > 0),
        "each view draws its cascades: {stats:?}"
    );
}

/// An orthographic camera draws a square as large near and far: a square 1 m wide fills half of an
/// image 2 m high, 32 of its 64 rows, at 2 m and at 20 m, where a camera in perspective draws it ten
/// times smaller. The sky shows as it lies along the camera's axis, the same in every pixel.
#[test]
fn an_orthographic_camera_draws_what_is_farther_as_large() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let things = Things::new(&gpu, &renderer);
    let mut view = view(&gpu, &renderer);
    let target = output(&gpu, 64, 64);
    // The square at `distance`, alone, seen as `projection` sees it, around `sky`: the image.
    let mut draw = |distance: f64, projection: Projection, sky: Option<&Environment>| {
        let mut scene = Scene::new(vec![Instance::new(
            &things.square,
            &things.glow,
            DAffine3::from_translation(DVec3::new(0.0, 0.0, -distance)),
        )]);
        scene.environment = sky.cloned();
        let camera = Camera {
            eye: DVec3::ZERO,
            target: DVec3::NEG_Z,
            projection,
            ..Camera::default()
        };
        renderer
            .render(
                &gpu,
                &scene,
                &mut [ViewDraw::new(&mut view, camera, &target)],
            )
            .expect("the frame is drawn");
        gpu.read_float_render_target(&target)
            .expect("the image is read back")
    };
    // The rows of the image's middle column that show some of the square, on a black sky.
    let mut lit_rows = |distance: f64, projection: Projection| {
        let image = draw(distance, projection, None);
        (0..64_usize)
            .filter(|&y| image.pixels[(y * 64 + 32) * 4] > 0.0)
            .count()
    };
    let orthographic = Projection::Orthographic {
        height: 2.0,
        far: 100.0,
    };
    let perspective = Projection::Perspective {
        fov_y: 2.0 * 0.5_f32.atan(),
    };
    assert_eq!(lit_rows(2.0, orthographic), 32, "at 2 m");
    assert_eq!(lit_rows(20.0, orthographic), 32, "at 20 m");
    assert_eq!(lit_rows(2.0, perspective), 32, "half the image at 2 m");
    let far = lit_rows(20.0, perspective);
    assert!((3..=4).contains(&far), "a tenth of it at 20 m: {far} rows");
    // Beyond its far plane, an orthographic camera sees nothing.
    let near_sighted = Projection::Orthographic {
        height: 2.0,
        far: 10.0,
    };
    assert_eq!(lit_rows(20.0, near_sighted), 0);
    // Seen along one direction, the sky is one color, and so is the square, whose reflection of the
    // sky does not change across it.
    let image = draw(2.0, orthographic, Some(&things.sky));
    let colors: std::collections::HashSet<[u32; 3]> = image
        .pixels
        .chunks(4)
        .map(|pixel| [0, 1, 2].map(|channel| pixel[channel].to_bits()))
        .collect();
    assert_eq!(colors.len(), 2, "{colors:?}");
}

/// A camera's image drawn in two halves, each by a view of its own into its half of the output,
/// gives the image drawn whole, its shadows included: the halves share the cascades of the whole
/// image. Without bloom, which spreads light across the halves' seam only in the whole. A half's
/// projection, the whole's moved and scaled, rounds otherwise: a sample that lies on an edge can
/// fall on its other side, which changes its pixel by a quarter of the edge's contrast at most, at
/// a few pixels.
#[test]
fn the_parts_of_an_image_drawn_side_by_side_make_the_whole() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let things = Things::new(&gpu, &renderer);
    let scene = things.yard(0.0);
    for projection in [
        Projection::Perspective { fov_y: 1.0 },
        Projection::Orthographic {
            height: 6.0,
            far: 30.0,
        },
    ] {
        let camera = Camera {
            projection,
            ..looking_from(DVec3::new(4.0, 3.0, 5.0))
        };
        let whole = alone(&gpu, &scene, camera, (96, 48));
        let target = output(&gpu, 96, 48);
        let (mut left, mut right) = (view(&gpu, &renderer), view(&gpu, &renderer));
        let half = |x: f32| Camera {
            part: ImagePart {
                x,
                y: 0.0,
                width: 0.5,
                height: 1.0,
            },
            ..camera
        };
        let mut draws = [
            ViewDraw::new(&mut left, half(0.0), &target),
            ViewDraw::new(&mut right, half(0.5), &target),
        ];
        for (draw, x) in draws.iter_mut().zip([0, 48]) {
            draw.viewport = Some(Viewport {
                x,
                y: 0,
                width: 48,
                height: 48,
            });
        }
        renderer
            .render(&gpu, &scene, &mut draws)
            .expect("the frame is drawn");
        let halves = gpu
            .read_float_render_target(&target)
            .expect("the image is read back");
        let (largest, differ) = difference(&halves, &whole, 2e-3);
        assert!(
            differ.len() <= 4 && largest < 0.1,
            "{projection:?}: pixels {differ:?} differ, by up to {largest}"
        );
    }
}

/// A view drawn into a rectangle of an output that another view covers draws there what it draws
/// alone into an output of the rectangle's size, and leaves the rest as the other drew it.
#[test]
fn a_view_drawn_into_a_rectangle_leaves_the_rest_of_its_output() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let things = Things::new(&gpu, &renderer);
    let scene = things.yard(0.04);
    let (around, above) = (
        looking_from(DVec3::new(4.0, 3.0, 5.0)),
        Camera {
            eye: DVec3::new(0.0, 10.0, 0.0),
            target: DVec3::ZERO,
            up: Vec3::NEG_Z,
            projection: Projection::Orthographic {
                height: 9.0,
                far: 20.0,
            },
            ..Camera::default()
        },
    );
    let rectangle = Viewport {
        x: 60,
        y: 4,
        width: 32,
        height: 32,
    };
    let behind = alone(&gpu, &scene, around, (96, 64));
    let corner = alone(&gpu, &scene, above, (32, 32));
    let target = output(&gpu, 96, 64);
    let (mut first, mut map) = (view(&gpu, &renderer), view(&gpu, &renderer));
    let mut in_corner = ViewDraw::new(&mut map, above, &target);
    in_corner.viewport = Some(rectangle);
    renderer
        .render(
            &gpu,
            &scene,
            &mut [ViewDraw::new(&mut first, around, &target), in_corner],
        )
        .expect("the frame is drawn");
    let drawn = gpu
        .read_float_render_target(&target)
        .expect("the image is read back");
    let inside = |x: u32, y: u32| {
        (rectangle.x..rectangle.x + rectangle.width).contains(&x)
            && (rectangle.y..rectangle.y + rectangle.height).contains(&y)
    };
    for y in 0..64 {
        for x in 0..96 {
            let at = ((y * 96 + x) * 4) as usize;
            let expected = if inside(x, y) {
                let at = (((y - rectangle.y) * 32 + x - rectangle.x) * 4) as usize;
                &corner.pixels[at..at + 4]
            } else {
                &behind.pixels[at..at + 4]
            };
            let largest = drawn.pixels[at..at + 4]
                .iter()
                .zip(expected)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f32::max);
            assert!(largest <= 2e-3, "({x}, {y}): off by {largest}");
        }
    }
}

/// Four views of 1280 × 720 with the sun share one cascades' atlas, where four renderers held one
/// each (ADR-0111 measured 508 MiB for them on an RX 6900 XT).
#[test]
fn four_views_take_one_cascades_atlas_where_four_renderers_took_four() {
    let gpu = gpu();
    if gpu.memory_in_use().is_none() {
        eprintln!("skipped: wgpu does not count the memory of this adapter's objects");
        return;
    }
    let made = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let things = Things::new(&gpu, &made);
    // The sun alone, as ADR-0111 measured.
    let mut scene = things.yard(0.04);
    scene.lights.truncate(1);
    let targets: Vec<RenderTarget> = (0..4).map(|_| output(&gpu, 1280, 720)).collect();
    let cameras = [
        DVec3::new(4.0, 3.0, 5.0),
        DVec3::new(-5.0, 2.0, -3.0),
        DVec3::new(0.0, 8.0, 1.0),
        DVec3::new(6.0, 1.0, 0.0),
    ]
    .map(looking_from);
    let in_use = || {
        gpu.wait_idle().expect("the GPU finishes");
        gpu.memory_in_use().expect("counted")
    };

    let before = in_use();
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let mut views: Vec<View> = (0..4).map(|_| view(&gpu, &renderer)).collect();
    let mut draws: Vec<ViewDraw<'_>> = views
        .iter_mut()
        .zip(&targets)
        .zip(cameras)
        .map(|((view, target), camera)| ViewDraw::new(view, camera, target))
        .collect();
    renderer
        .render(&gpu, &scene, &mut draws)
        .expect("the frame is drawn");
    drop(draws);
    let four_views = in_use() - before;
    drop((renderer, views));

    let before = in_use();
    let mut renderers = Vec::new();
    for (target, camera) in targets.iter().zip(cameras) {
        let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
        let mut view = view(&gpu, &renderer);
        renderer
            .render(
                &gpu,
                &scene,
                &mut [ViewDraw::new(&mut view, camera, target)],
            )
            .expect("the frame is drawn");
        renderers.push((renderer, view));
    }
    let four_renderers = in_use() - before;
    eprintln!(
        "on {}: four views of 1280 × 720 with the sun take {:.1} MiB, four \
         renderers {:.1} MiB",
        gpu.adapter_name(),
        four_views as f64 / MIB as f64,
        four_renderers as f64 / MIB as f64
    );
    // Three cascades' atlases of 64 MiB each, but a few MiB that the allocators keep differently in
    // each case: lavapipe shows exactly three atlases more, Direct3D 12 on WARP 0.4 MiB less.
    assert!(
        four_renderers + 4 * MIB >= four_views + 3 * 64 * MIB,
        "{four_views} bytes against {four_renderers}"
    );
}

/// Reads the texel of an image under each pixel of a target of its size, as a panel of an editor's
/// interface shows a view's image.
const PANEL: &str = "
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var nearest: sampler;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureSample(image, nearest, position.xy / vec2<f32>(textureDimensions(image)));
}
";

/// A view's image, drawn into an output of its own, is sampled by a pipeline of the GPU layer, as
/// the editor's interface shows a viewport in a panel (ADR-0115): the panel shows what the view
/// drew.
#[test]
fn a_view_s_image_is_sampled_by_another_pipeline() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let things = Things::new(&gpu, &renderer);
    let mut view = view(&gpu, &renderer);
    let image = output(&gpu, 64, 48);
    renderer
        .render(
            &gpu,
            &things.yard(0.04),
            &mut [ViewDraw::new(
                &mut view,
                looking_from(DVec3::new(4.0, 3.0, 6.0)),
                &image,
            )],
        )
        .expect("the frame is drawn");
    let layout = gpu
        .create_bind_group_layout(
            "panel",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::Texture,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::Sampler,
                },
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
    let group = gpu
        .create_bind_group(
            "panel",
            &layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::RenderTarget(&image),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        )
        .expect("the GPU has memory for it");
    let shader = gpu
        .create_shader_module("panel", PANEL)
        .expect("the shader compiles");
    let pipeline = gpu
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: "panel",
            shader: &shader,
            vertex_entry_point: "vs_main",
            vertex_layout: None,
            fragment: Some(FragmentState {
                entry_point: "fs_main",
                target_format: Some(FORMAT),
                blend: None,
            }),
            bind_group_layouts: &[&layout],
            depth: None,
            cull: CullMode::None,
            multisampled: false,
        })
        .expect("the pipeline is valid");
    let panel = output(&gpu, 64, 48);
    gpu.render(
        &RenderPassDescriptor {
            target: &panel,
            clear: Some(LinearColor::BLACK),
            depth: None,
            timer: None,
            resolve: None,
        },
        |pass| {
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3);
        },
    );
    let drawn = gpu
        .read_float_render_target(&image)
        .expect("the image is read back");
    let shown = gpu
        .read_float_render_target(&panel)
        .expect("the image is read back");
    assert!(
        drawn.pixels.iter().any(|&value| value > 0.1),
        "a yard drawn"
    );
    assert_eq!(shown.pixels, drawn.pixels);
}

/// A view resized gives its old targets back: drawn again at its first size, it holds what it held
/// then.
#[test]
fn a_view_resized_gives_its_old_targets_back() {
    let gpu = gpu();
    if gpu.memory_in_use().is_none() {
        eprintln!("skipped: wgpu does not count the memory of this adapter's objects");
        return;
    }
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let things = Things::new(&gpu, &renderer);
    let scene = things.yard(0.04);
    let mut view = view(&gpu, &renderer);
    let (small, large) = (output(&gpu, 256, 256), output(&gpu, 512, 512));
    let camera = looking_from(DVec3::new(4.0, 3.0, 5.0));
    let mut held_after = |target: &RenderTarget| {
        draw_settled(&gpu, &mut renderer, &scene, &mut view, camera, target);
        gpu.wait_idle().expect("the GPU finishes");
        gpu.memory_in_use().expect("counted")
    };
    let first = held_after(&small);
    let resized = held_after(&large);
    assert!(resized > first, "larger targets: {first} then {resized}");
    // Less than the smallest of the view's images at the larger size, its high-dynamic-range one of
    // 2 MiB: the allocators keep a little differently from one frame to the next, a few kilobytes
    // on Vulkan, and 1.3 MiB more on Direct3D 12 on WARP once the canvas's objects joined the
    // renderer, where Vulkan's figures did not change.
    let again = held_after(&small);
    assert!(
        again < first + 2 * MIB,
        "nothing of the larger size kept: {first} bytes, then {again}"
    );
}

/// G47: a multisampled output is refused by the renderer, which resolves its own samples, not
/// inside wgpu.
#[test]
#[should_panic(expected = "the renderer draws into targets of one sample per pixel")]
fn a_multisampled_output_is_a_bug_of_the_caller() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let mut view = view(&gpu, &renderer);
    let target = gpu
        .create_multisampled_render_target("multisampled", 16, 16, FORMAT)
        .expect("the GPU has memory for it");
    let _ = renderer.render(
        &gpu,
        &Scene::new(Vec::new()),
        &mut [ViewDraw::new(&mut view, Camera::default(), &target)],
    );
}

/// A rectangle that passes its output's edge is a bug of the caller.
#[test]
#[should_panic(expected = "which is not a rectangle of `output`")]
fn a_rectangle_beyond_its_output_is_a_bug_of_the_caller() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let mut view = view(&gpu, &renderer);
    let target = output(&gpu, 16, 16);
    let mut draw = ViewDraw::new(&mut view, Camera::default(), &target);
    draw.viewport = Some(Viewport {
        x: 8,
        y: 0,
        width: 9,
        height: 16,
    });
    let _ = renderer.render(&gpu, &Scene::new(Vec::new()), &mut [draw]);
}
