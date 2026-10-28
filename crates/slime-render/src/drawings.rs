//! What a client, an editor or a tool, draws into a view besides the scene (ADR-0129): lines a
//! width in pixels wide and instances, at the two points of the view's frame that ADR-0115 opens to
//! it, in the scene and over its image.

use glam::{DAffine3, DMat3, DQuat, DVec3};
use slime_color::LinearColor;
use slime_gfx::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindingResource, GfxError, Gpu, StorageBuffer,
    UniformBuffer,
};

use crate::bytes::push_f32s;
use crate::renderer::Instance;

/// A segment of a line, drawn as a band that faces the camera, a width in pixels wide, its ends
/// squared and stretched by half its width, so that the segments of a box meet at its corners.
/// Its edges and its ends are smoothed over a pixel, whatever the view's anti-aliasing.
///
/// A segment whose ends or width are not finite numbers, whose width is not positive, or whose
/// color has a channel out of 0 to 1, is not drawn; nor is its part behind the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    /// One end, in world space, in metres.
    pub from: DVec3,
    /// The other end.
    pub to: DVec3,
    /// Its color, in linear values, and its alpha, which lays it over what is behind it: each from 0
    /// to 1. In the scene, it is shown as an unlit material's is ([`UnlitData`](crate::UnlitData)),
    /// through the tone curve; over the image, as it is.
    pub color: LinearColor,
    /// Its width, in the pixels of the view's output: at a render scale other than 1, the same on
    /// screen ([`ViewSettings::render_scale`](crate::ViewSettings::render_scale)).
    pub width: f32,
}

impl Line {
    /// The lines of a grid on the horizontal plane at `height`, one each `spacing` metres along each
    /// axis, on the multiples of `spacing`, as far as `reach` metres from the multiple nearest
    /// `center` along each axis, each of `color` and `width`: a grid on the ground, of
    /// 2 × (2 × ⌊`reach` / `spacing`⌋ + 1) lines, which the caller keeps few, a view drawing each.
    /// None for a `spacing` or a `reach` that is not a finite number above 0.
    #[must_use]
    pub fn grid(
        center: DVec3,
        height: f64,
        reach: f64,
        spacing: f64,
        color: LinearColor,
        width: f32,
    ) -> Vec<Line> {
        if !(spacing.is_finite() && spacing > 0.0 && reach.is_finite() && reach > 0.0) {
            return Vec::new();
        }
        let steps = (reach / spacing).floor() as i64;
        let (x, z) = (
            (center.x / spacing).round() * spacing,
            (center.z / spacing).round() * spacing,
        );
        let extent = steps as f64 * spacing;
        let line = |from: DVec3, to: DVec3| Line {
            from,
            to,
            color,
            width,
        };
        (-steps..=steps)
            .flat_map(|step| {
                let step = step as f64 * spacing;
                [
                    line(
                        DVec3::new(x + step, height, z - extent),
                        DVec3::new(x + step, height, z + extent),
                    ),
                    line(
                        DVec3::new(x - extent, height, z + step),
                        DVec3::new(x + extent, height, z + step),
                    ),
                ]
            })
            .collect()
    }
}

/// The transform of a square of `side` metres at `position` that faces a camera at `eye`, upright,
/// as the icon of a light faces the camera: a square that faces +Z, as
/// [`MeshData::square`](crate::MeshData::square) makes it, its +Z turned towards the camera and its
/// +Y kept up as far as it can be.
#[must_use]
pub fn facing(position: DVec3, eye: DVec3, side: f64) -> DAffine3 {
    let back = (eye - position).normalize_or(DVec3::Z);
    let right = DVec3::Y.cross(back).normalize_or(DVec3::X);
    let rotation = DQuat::from_mat3(&DMat3::from_cols(right, back.cross(right), back));
    DAffine3::from_scale_rotation_translation(DVec3::splat(side), rotation, position)
}

/// What a client draws at the first point of a view's frame, in the scene ([`Drawings`]).
#[derive(Clone, Debug, Default)]
pub struct DrawList {
    /// The lines, laid over what the scene and the instances draw, in their order.
    pub lines: Vec<Line>,
    /// The instances, of any kind of material, an unlit one for an icon or a handle: drawn as the
    /// scene's are, with their depth, lit and shadowed as they are, but casting no shadow, and not
    /// counted when the directional light's cascades are fitted to what casts shadows.
    pub instances: Vec<Instance>,
}

/// What a client, an editor or a tool, draws into a view besides the scene, at the two points of
/// its frame ([`ViewDraw::drawings`](crate::ViewDraw::drawings), ADR-0115, ADR-0129).
#[derive(Clone, Debug, Default)]
pub struct Drawings {
    /// Drawn in the scene, after its surfaces and its sky, at the view's anti-aliasing and render
    /// scale, then through the bloom and the tone curve, as the scene is: hidden by the surfaces in
    /// front of them, as a grid or the icons of lights placed in the world are. A line that lies
    /// on a surface may be hidden by it in places: lift it off a little.
    pub in_scene: DrawList,
    /// Drawn over the view's image once tone mapped, over everything, in the output's colors, as
    /// the box around what is selected is: lines alone until the editor's handles need instances
    /// there.
    pub over_image: Vec<Line>,
}

/// The size of a segment as `Segment` in `lines.wgsl` lays it out: one end and the width (16
/// bytes), the other end and padding (16), the color (16).
pub(crate) const SEGMENT_SIZE: usize = 48;

/// The size of the settings of the lines at one point, as `Settings` in `lines.wgsl` lays them out:
/// where clip space lies in the target drawn into, its corner and its size in the target's texels,
/// the size of the view's rectangle in the output's pixels, what the colors are multiplied by, and
/// padding.
pub(crate) const LINE_SETTINGS_SIZE: usize = 32;

/// Packs the segments of `lines` that can be drawn into `bytes`, relative to `origin`, the
/// camera's position (ADR-0048), as `lines.wgsl` reads them, at most `room` of them; returns how
/// many it packed, and how many that could be drawn it left out for want of room.
pub(crate) fn pack_lines(
    lines: &[Line],
    origin: DVec3,
    bytes: &mut Vec<u8>,
    room: u32,
) -> (u32, usize) {
    let (mut packed, mut left_out) = (0, 0);
    for line in lines {
        let LinearColor { r, g, b, a } = line.color;
        if !(line.from.is_finite()
            && line.to.is_finite()
            && line.width.is_finite()
            && line.width > 0.0
            && [r, g, b, a]
                .iter()
                .all(|channel| (0.0..=1.0).contains(channel)))
        {
            continue;
        }
        if packed == room {
            left_out += 1;
            continue;
        }
        let (from, to) = ((line.from - origin).as_vec3(), (line.to - origin).as_vec3());
        push_f32s(bytes, &[from.x, from.y, from.z, line.width]);
        push_f32s(bytes, &[to.x, to.y, to.z, 0.0]);
        push_f32s(bytes, &[r, g, b, a]);
        packed += 1;
    }
    (packed, left_out)
}

/// The settings of the lines at one point: drawn into the texels of a target from `origin`, `extent`
/// of them, that a view's rectangle of `rectangle` pixels of its output spans, their colors
/// multiplied by `scale`.
pub(crate) fn line_settings(
    origin: (u32, u32),
    extent: (u32, u32),
    rectangle: (u32, u32),
    scale: f32,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(LINE_SETTINGS_SIZE);
    push_f32s(
        &mut bytes,
        &[
            origin.0 as f32,
            origin.1 as f32,
            extent.0 as f32,
            extent.1 as f32,
            rectangle.0 as f32,
            rectangle.1 as f32,
            scale,
            0.0,
        ],
    );
    bytes
}

/// A view's lines on the GPU: the segments of both points, those in the scene first, and the
/// settings of each point, with the group through which each point's pipeline reads them.
#[derive(Debug)]
pub(crate) struct LineBuffers {
    pub(crate) segments: StorageBuffer,
    /// In the scene, then over the image.
    pub(crate) settings: [UniformBuffer; 2],
    pub(crate) groups: [BindGroup; 2],
    /// How many segments `segments` holds.
    pub(crate) capacity: usize,
}

impl LineBuffers {
    /// Buffers for `capacity` segments, read through groups of `layout`.
    pub(crate) fn new(
        gpu: &Gpu,
        layout: &BindGroupLayout,
        capacity: usize,
    ) -> Result<Self, GfxError> {
        let buffer = gpu.create_storage_buffer("lines", (capacity * SEGMENT_SIZE) as u64)?;
        let settings = [
            gpu.create_uniform_buffer("lines in the scene", &[0; LINE_SETTINGS_SIZE])?,
            gpu.create_uniform_buffer("lines over the image", &[0; LINE_SETTINGS_SIZE])?,
        ];
        let group = |settings: &UniformBuffer| {
            gpu.create_bind_group(
                "lines",
                layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::StorageBuffer(&buffer),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::UniformBuffer(settings),
                    },
                ],
            )
        };
        let groups = [group(&settings[0])?, group(&settings[1])?];
        Ok(Self {
            segments: buffer,
            settings,
            groups,
            capacity,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floats(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks(4)
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
            .collect()
    }

    #[test]
    fn segments_are_packed_relative_to_the_camera_and_those_that_cannot_be_drawn_left_out() {
        let line = Line {
            from: DVec3::new(1001.0, 2.0, 3.0),
            to: DVec3::new(1004.0, 5.0, 6.0),
            color: LinearColor::rgba(0.1, 0.2, 0.3, 0.4),
            width: 2.0,
        };
        let mut bytes = Vec::new();
        let packed = pack_lines(
            &[
                line,
                Line {
                    color: LinearColor::rgba(0.1, 0.2, f32::NAN, 0.4),
                    ..line
                },
                Line {
                    color: LinearColor::rgb(2.0, 0.0, 0.0),
                    ..line
                },
                line,
                Line { width: 0.0, ..line },
                Line {
                    to: DVec3::new(f64::NAN, 0.0, 0.0),
                    ..line
                },
                Line {
                    width: f32::INFINITY,
                    ..line
                },
            ],
            DVec3::new(1000.0, 0.0, 0.0),
            &mut bytes,
            1,
        );
        // The first two that can be drawn, one packed and one left out for want of room.
        assert_eq!(packed, (1, 1));
        assert_eq!(bytes.len(), SEGMENT_SIZE);
        assert_eq!(
            floats(&bytes),
            [1.0, 2.0, 3.0, 2.0, 4.0, 5.0, 6.0, 0.0, 0.1, 0.2, 0.3, 0.4]
        );
    }
}
