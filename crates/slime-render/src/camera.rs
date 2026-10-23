//! The camera: where the scene is seen from, and how it is projected.

use glam::{DMat4, DVec3, Mat4, Vec2, Vec3, Vec4};

/// How a camera projects what it sees onto its image (ADR-0111).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    /// In perspective, as an eye sees: what is farther is smaller. Nothing is too far to be seen:
    /// depth is reversed and the far plane is at infinity (ADR-0047).
    Perspective {
        /// The vertical field of view, in radians, of the camera's whole image, from 0 to π
        /// excluded.
        fov_y: f32,
    },
    /// Orthographic, as a map or a plan is drawn: what is farther is as large, the image as high as
    /// `height` metres at every distance, as wide as its aspect makes it. Depth is reversed and
    /// linear, from 1 at the near plane to 0 at `far`, beyond which nothing is seen. The sky is
    /// drawn as it lies along the camera's axis: an orthographic camera sees one direction.
    ///
    /// The sun's shadows reach as far from the eye along the axis as they do in perspective, its
    /// [`shadow_distance`](crate::DirectionalLight::shadow_distance), and no farther than `far`: a
    /// map drawn from higher above the ground than that shows none. Its four cascades are nearly
    /// as large, the image's width and height setting them more than their depths.
    Orthographic {
        /// The height of the camera's whole image, in metres, positive.
        height: f32,
        /// The distance from the eye of the farthest visible point, beyond the camera's near
        /// plane.
        far: f32,
    },
}

/// A part of an image, in fractions of the whole image's width and height, from its top left
/// corner; [`ImagePart::WHOLE`] is the whole image. Of a camera's image, what a view draws of it,
/// as a split screen draws half of an image and a magnifier a small part of it (ADR-0111): the
/// view's output gives the part's aspect, the whole image as many times wider and higher as the
/// part is a fraction of it. Of an image that a canvas draws, what an
/// [`ImageQuad`](crate::ImageQuad) shows of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImagePart {
    /// The part's left edge, from 0 at the image's left edge to 1 at its right.
    pub x: f32,
    /// The part's top edge, from 0 at the image's top edge to 1 at its bottom.
    pub y: f32,
    /// The part's width, positive, a fraction of the image's width.
    pub width: f32,
    /// The part's height, positive, a fraction of the image's height.
    pub height: f32,
}

impl ImagePart {
    /// The whole image.
    pub const WHOLE: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };
}

impl Default for ImagePart {
    /// The whole image.
    fn default() -> Self {
        Self::WHOLE
    }
}

/// A camera, looking from [`eye`](Self::eye) at [`target`](Self::target), in perspective or
/// orthographic ([`Projection`]), drawing its whole image or a part of it ([`ImagePart`]).
///
/// World space is right-handed with +Y up, as in glTF; the camera looks down its own −Z axis.
///
/// A camera that [`view`](Self::view) or [`projection`](Self::projection) says draws nothing can
/// panic inside glam instead when glam's assertions are on: its `glam-assert` feature, or
/// `debug-glam-assert` in a debug build. A game that enables either on the glam it shares with
/// the engine, the version [`crate::glam`] re-exports, enables it in the engine too.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Where the camera is, in world space, in metres: `f64`, as positions in the world are
    /// (ADR-0079).
    pub eye: DVec3,
    /// The point it looks at, which must differ from `eye`.
    pub target: DVec3,
    /// Which way is up, usually [`Vec3::Y`]. Its length does not matter, short of one so small or
    /// so large that its square rounds to zero or overflows in `f32`, but it must not be zero or
    /// parallel to the viewing direction.
    pub up: Vec3,
    /// How it projects what it sees.
    pub projection: Projection,
    /// The distance of the nearest visible point, positive. In perspective, a larger `near` only
    /// hides what is nearer, without making the depth of far surfaces finer (ADR-0047).
    pub near: f32,
    /// The part of its image that it draws.
    pub part: ImagePart,
}

impl Default for Camera {
    /// Five units in front of the origin, looking at it, in perspective with a field of view of
    /// 60°, seeing from 0.1 away, drawing its whole image.
    fn default() -> Self {
        Self {
            eye: DVec3::new(0.0, 0.0, 5.0),
            target: DVec3::ZERO,
            up: Vec3::Y,
            projection: Projection::Perspective {
                fov_y: 60f32.to_radians(),
            },
            near: 0.1,
            part: ImagePart::WHOLE,
        }
    }
}

impl Camera {
    /// The view matrix: from world space to the camera's space, in `f64`, as world positions are.
    /// A `target` at `eye`, or so near it that the square of their distance rounds to zero, or an
    /// `up` that is zero, parallel to the viewing direction or of an extreme length (see
    /// [`up`](Self::up)), gives a matrix that is not finite and draws nothing.
    ///
    /// # Panics
    ///
    /// Inside glam, for those cameras, when glam's assertions are on (see [`Camera`]).
    pub fn view(&self) -> DMat4 {
        // glam asks for an `up` of length one, and asserts it when its assertions are on.
        glam::dcamera::rh::view::look_at_mat4(self.eye, self.target, self.up.as_dvec3().normalize())
    }

    /// The view matrix from world space moved by −[`eye`](Self::eye), the space in which the
    /// renderer draws (ADR-0048): the rotation of [`view`](Self::view) alone. The direction from
    /// the eye to the target is computed in `f64`, so that a camera far from the origin keeps it.
    pub(crate) fn view_from_eye(&self) -> Mat4 {
        let direction = (self.target - self.eye).as_vec3();
        glam::camera::rh::view::look_at_mat4(Vec3::ZERO, direction, self.up.normalize())
    }

    /// The aspect of the camera's whole image when its [`part`](Self::part) is drawn into an output
    /// `aspect` times as wide as it is high.
    pub(crate) fn whole_aspect(&self, aspect: f32) -> f32 {
        aspect * self.part.height / self.part.width
    }

    /// The projection matrix of the camera's [`part`](Self::part) drawn into an output `aspect`
    /// times as wide as it is high: from the camera's space to clip space, +Y up, the convention of
    /// WebGPU, which wgpu follows on every graphics API, with depth reversed (ADR-0047). In
    /// perspective, depth is 1 at [`near`](Self::near) and falls towards 0 as the distance d grows,
    /// as `near` / d, without reaching it: the far plane is at infinity, and 32-bit float depths
    /// keep about the same relative precision at every distance. Orthographic, it falls linearly
    /// from 1 at `near` to 0 at the projection's `far`. The whole image's projection is moved and
    /// scaled so that the part fills clip space: parts drawn side by side make the whole image. A
    /// field of view outside (0, π), a height or a part of no size, a `near` that is not positive
    /// or a `far` not beyond it gives a matrix that draws nothing usable.
    ///
    /// # Panics
    ///
    /// Inside glam, when glam's assertions are on (see [`Camera`]): in perspective, for a field of
    /// view outside (0, π), a `near` that is not positive, or an `aspect` of zero; orthographic,
    /// for a height or an `aspect` of zero, or a `far` equal to `near`.
    pub fn projection(&self, aspect: f32) -> Mat4 {
        let aspect = self.whole_aspect(aspect);
        let whole = match self.projection {
            Projection::Perspective { fov_y } => {
                glam::camera::rh::proj::directx::perspective_infinite_reverse(
                    fov_y, aspect, self.near,
                )
            }
            Projection::Orthographic { height, far } => {
                let (half_height, half_width) = (height / 2.0, height / 2.0 * aspect);
                // Its far and near planes given in each other's place: depth reversed.
                glam::camera::rh::proj::directx::orthographic(
                    -half_width,
                    half_width,
                    -half_height,
                    half_height,
                    far,
                    self.near,
                )
            }
        };
        // The part's center and size in clip space, +Y up where the part's y runs down.
        let ImagePart {
            x,
            y,
            width,
            height,
        } = self.part;
        let center_x = -1.0 + 2.0 * x + width;
        let center_y = 1.0 - 2.0 * y - height;
        let part = Mat4::from_cols(
            Vec4::new(1.0 / width, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 1.0 / height, 0.0, 0.0),
            Vec4::Z,
            Vec4::new(-center_x / width, -center_y / height, 0.0, 1.0),
        );
        part * whole
    }

    /// The projection times the view: from world space to clip space, in `f64` as
    /// [`view`](Self::view) is.
    ///
    /// # Panics
    ///
    /// As [`view`](Self::view) and [`projection`](Self::projection) do.
    pub fn view_projection(&self, aspect: f32) -> DMat4 {
        self.projection(aspect).as_dmat4() * self.view()
    }

    /// The ray through `point` of the image that the camera draws into an output `aspect` times as
    /// wide as it is high, its [`part`](Self::part) if it draws one: `point` runs from (0, 0), the
    /// image's top left corner, to (1, 1), its bottom right, a pixel's centre at ((x + 0.5) /
    /// width, (y + 0.5) / height). Returns where the ray starts, on the near plane, and its unit
    /// direction, in world space: what picking by the CPU casts (ADR-0130). In perspective, every
    /// ray starts near the eye; orthographic, they run side by side along the camera's axis. A
    /// camera whose matrices draw nothing usable gives a ray that is not finite.
    ///
    /// # Panics
    ///
    /// As [`projection`](Self::projection) does.
    #[must_use]
    pub fn ray(&self, aspect: f32, point: Vec2) -> (DVec3, DVec3) {
        // Relative to the eye, as the renderer draws (ADR-0048); depth reversed, the near plane at
        // 1.
        let inverse = (self.projection(aspect) * self.view_from_eye()).inverse();
        let on_screen = Vec2::new(point.x * 2.0 - 1.0, 1.0 - point.y * 2.0);
        let near = inverse.project_point3(on_screen.extend(1.0));
        let farther = inverse.project_point3(on_screen.extend(0.5));
        (
            self.eye + near.as_dvec3(),
            (farther - near).as_dvec3().normalize(),
        )
    }

    /// The unit direction from what the camera sees towards it, the same everywhere, for an
    /// orthographic camera, in the frame's space; `None` in perspective, where it goes towards the
    /// eye.
    pub(crate) fn orthographic_backward(&self) -> Option<Vec3> {
        match self.projection {
            Projection::Orthographic { .. } => {
                Some((self.eye - self.target).as_vec3().normalize_or_zero())
            }
            Projection::Perspective { .. } => None,
        }
    }
}

/// What a camera sees, for frustum culling: the space between the four planes of the sides of the
/// image, beyond the near plane and before the far plane, in the frame's space (ADR-0048). In
/// perspective, the far plane lies at infinity (ADR-0047): nothing is too far to be seen.
pub(crate) struct Frustum {
    /// Each plane as (n, d), n pointing inside: a point p is on its inner side when n · p + d is
    /// not negative. Left, right, bottom, top, near, far.
    planes: [Vec4; 6],
}

impl Frustum {
    /// The frustum of `view_projection`, the matrix from the frame's space to clip space, whose
    /// rows give its planes (Gribb and Hartmann): a point is seen where −w ≤ x ≤ w, −w ≤
    /// y ≤ w and 0 ≤ z ≤ w, reversed depth putting the near plane at z = w and the far one at
    /// z = 0, which a perspective's infinite far plane never reaches.
    pub(crate) fn new(view_projection: Mat4) -> Self {
        let [x, y, z, w] = [0, 1, 2, 3].map(|row| view_projection.row(row));
        Self {
            planes: [w + x, w - x, w + y, w - y, w - z, z],
        }
    }
    /// Whether the box of an instance may be seen: `transform` takes it from its own space, where
    /// it has its center and half its size along each axis, to the frame's space. A box that some
    /// plane leaves wholly outside is not seen; one that crosses each plane, or that lies inside,
    /// is, even when it lies outside two planes near a corner, which costs a draw and keeps the
    /// test cheap. A box at no finite place is not seen.
    pub(crate) fn holds(&self, (transform, center, half_size): (Mat4, Vec3, Vec3)) -> bool {
        let center = transform.transform_point3(center);
        // The box's half axes in the frame's space: how far it reaches from its center along a
        // plane's normal is the sum of their lengths along it.
        let axes = [
            transform.x_axis.truncate() * half_size.x,
            transform.y_axis.truncate() * half_size.y,
            transform.z_axis.truncate() * half_size.z,
        ];
        self.planes.iter().all(|plane| {
            let normal = plane.truncate();
            let reach: f32 = axes.iter().map(|axis| normal.dot(*axis).abs()).sum();
            // Not a number, for a box at no finite place, compares as false: not seen.
            normal.dot(center) + plane.w >= -reach
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Projects `point` and divides by w: normalized device coordinates.
    fn project(camera: &Camera, point: DVec3) -> DVec3 {
        let clip = camera.view_projection(1.0) * point.extend(1.0);
        clip.truncate() / clip.w
    }

    #[test]
    fn the_target_is_at_the_center_and_depth_falls_from_1_at_near_towards_0_at_infinity() {
        let camera = Camera {
            eye: DVec3::new(0.0, 0.0, 5.0),
            near: 1.0,
            ..Camera::default()
        };
        let center = project(&camera, camera.target);
        assert!(center.x.abs() < 1e-6 && center.y.abs() < 1e-6, "{center}");
        let depth = |distance: f64| project(&camera, DVec3::new(0.0, 0.0, 5.0 - distance)).z;
        assert!((depth(1.0) - 1.0).abs() < 1e-6, "near plane at depth 1");
        assert!((depth(4.0) - 0.25).abs() < 1e-6, "near / distance");
        // Far beyond any far plane the camera had, depth still orders distances: a float keeps
        // its relative precision down to 1e-38.
        let (far, farther) = (depth(1e7), depth(1.1e7));
        assert!(
            0.0 < farther && farther < far && far < 1e-6,
            "{far}, {farther}"
        );
        // A direction, a point at infinity, is at depth 0.
        let clip = camera.view_projection(1.0) * DVec3::NEG_Z.extend(0.0);
        assert!(clip.z == 0.0 && clip.w > 0.0, "{clip}");
    }

    #[test]
    fn an_orthographic_camera_s_depth_falls_linearly_from_1_at_near_to_0_at_far() {
        let camera = Camera {
            eye: DVec3::new(0.0, 0.0, 5.0),
            near: 1.0,
            projection: Projection::Orthographic {
                height: 4.0,
                far: 9.0,
            },
            ..Camera::default()
        };
        let at = |x: f64, y: f64, distance: f64| project(&camera, DVec3::new(x, y, 5.0 - distance));
        for (distance, depth) in [(1.0, 1.0), (5.0, 0.5), (9.0, 0.0)] {
            let point = at(0.0, 0.0, distance);
            assert!((point.z - depth).abs() < 1e-6, "{distance} m: {point}");
        }
        // As large at any distance: 2 m from the axis is the image's edge, near and far.
        for distance in [1.0, 9.0] {
            let corner = at(2.0, 2.0, distance);
            assert!(
                (corner.x - 1.0).abs() < 1e-6 && (corner.y - 1.0).abs() < 1e-6,
                "{corner}"
            );
        }
        // What lies beyond the far plane is outside the frustum, and nearer inside it.
        let frustum = Frustum::new(camera.view_projection(1.0).as_mat4());
        let unit = |distance: f32| {
            (
                Mat4::from_translation(Vec3::new(0.0, 0.0, 5.0 - distance)),
                Vec3::ZERO,
                Vec3::splat(0.5),
            )
        };
        assert!(frustum.holds(unit(8.0)), "inside");
        assert!(!frustum.holds(unit(10.0)), "beyond the far plane");
        assert_eq!(
            camera.orthographic_backward(),
            Some(Vec3::Z),
            "seen from +Z, as its eye is"
        );
    }

    /// A part of the image is the whole image's projection moved and scaled: what the whole image
    /// shows at the part's corners, the part shows at its own.
    #[test]
    fn a_part_of_the_image_shows_what_the_whole_shows_there() {
        let whole = Camera::default();
        // The top right quarter of the image, drawn into a square view: the whole image is square.
        let part = Camera {
            part: ImagePart {
                x: 0.5,
                y: 0.0,
                width: 0.5,
                height: 0.5,
            },
            ..whole
        };
        let in_clip = |camera: &Camera, point: Vec3| {
            let clip = camera.projection(1.0) * point.extend(1.0);
            clip.truncate() / clip.w
        };
        // A point that the whole image shows at its middle, at the top right quarter's bottom left
        // corner, and one at the whole image's top right corner, which the quarter shows at its own.
        let middle = Vec3::new(0.0, 0.0, -3.0);
        let corner = {
            let clip = Vec3::new(1.0, 1.0, in_clip(&whole, middle).z);
            let point = whole.projection(1.0).inverse() * clip.extend(1.0);
            point.truncate() / point.w
        };
        for (point, expected) in [(middle, (-1.0, -1.0)), (corner, (1.0, 1.0))] {
            let shown = in_clip(&part, point);
            assert!(
                (shown.x - expected.0).abs() < 1e-5 && (shown.y - expected.1).abs() < 1e-5,
                "{point}: {shown}"
            );
            assert!(
                (shown.z - in_clip(&whole, point).z).abs() < 1e-6,
                "depth unchanged"
            );
        }
        assert_eq!(part.whole_aspect(1.0), 1.0);
        let half = Camera {
            part: ImagePart {
                width: 0.5,
                ..ImagePart::WHOLE
            },
            ..whole
        };
        assert_eq!(
            half.whole_aspect(1.0),
            2.0,
            "a half as wide as high is half a 2:1 image"
        );
    }

    #[test]
    fn the_right_of_the_camera_is_on_the_right_of_the_image() {
        let camera = Camera::default();
        let right = project(&camera, DVec3::new(1.0, 0.0, 0.0));
        let up = project(&camera, DVec3::new(0.0, 1.0, 0.0));
        assert!(right.x > 0.0 && right.y.abs() < 1e-6, "{right}");
        assert!(up.y > 0.0 && up.x.abs() < 1e-6, "{up}");
    }

    /// glam's `look_at_mat4` asks for an `up` of length one and asserts it when glam's assertions
    /// are on; with them off, it ignores the length. This test can therefore fail only with them
    /// on, as in `cargo test -p slime-render --features glam/debug-glam-assert`, which
    /// `scripts/check.sh` runs.
    #[test]
    fn the_length_of_up_does_not_change_the_view() {
        let unit = Camera::default().view();
        for length in [2.0, 1e-3] {
            let camera = Camera {
                up: Vec3::Y * length,
                ..Camera::default()
            };
            assert!(camera.view().abs_diff_eq(unit, 1e-6), "length {length}");
        }
    }

    /// The frustum of the default camera, looking down −Z from 5 m in front of the origin with a
    /// field of view of 60°, on a square image: in the frame's space, the eye is at the origin.
    fn frustum() -> Frustum {
        let camera = Camera::default();
        Frustum::new(camera.projection(1.0) * camera.view_from_eye())
    }

    /// A box of half size `half` at `center`, in the frame's space.
    fn boxed(center: Vec3, half: f32) -> (Mat4, Vec3, Vec3) {
        (
            Mat4::from_translation(center),
            Vec3::ZERO,
            Vec3::splat(half),
        )
    }

    #[test]
    fn a_box_in_view_or_across_its_edges_is_seen_and_one_beyond_them_is_not() {
        let frustum = frustum();
        // 10 m ahead, the image spans tan(30°) × 10 = 5.77 m on each side of the axis.
        for (center, half, seen, case) in [
            (Vec3::new(0.0, 0.0, -10.0), 0.5, true, "ahead"),
            (
                Vec3::new(6.0, 0.0, -10.0),
                0.5,
                true,
                "across the right edge",
            ),
            (
                Vec3::new(7.0, 0.0, -10.0),
                0.5,
                false,
                "beyond the right edge",
            ),
            (
                Vec3::new(0.0, -7.0, -10.0),
                0.5,
                false,
                "beyond the bottom edge",
            ),
            (Vec3::new(0.0, 0.0, 10.0), 0.5, false, "behind the eye"),
            (Vec3::new(0.0, 0.0, 0.0), 0.5, true, "around the eye"),
            (
                Vec3::new(0.0, 0.0, -0.05),
                0.01,
                false,
                "nearer than the near plane",
            ),
            (
                Vec3::new(0.0, 0.0, -1e9),
                1.0,
                true,
                "farther than any far plane",
            ),
        ] {
            assert_eq!(frustum.holds(boxed(center, half)), seen, "{case}");
        }
    }

    #[test]
    fn a_box_turned_and_scaled_reaches_as_far_as_its_corners() {
        let frustum = frustum();
        // A rod 4 m long along X, its center 7.5 m right of the axis 10 m ahead: lying along X it
        // reaches 5.5 m right of the axis, in view; turned along Y, it does not.
        let at = Mat4::from_translation(Vec3::new(7.5, 0.0, -10.0));
        let rod = Mat4::from_scale(Vec3::new(4.0, 0.1, 0.1));
        let along_x = at * rod;
        let along_y = at * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2) * rod;
        let unit = (Vec3::ZERO, Vec3::splat(0.5));
        assert!(frustum.holds((along_x, unit.0, unit.1)), "along X");
        assert!(!frustum.holds((along_y, unit.0, unit.1)), "along Y");
    }

    #[test]
    fn the_frustum_follows_where_the_camera_looks_and_the_image_s_aspect() {
        // Looking along +X from the origin, at an image twice as wide as it is high: 10 m ahead, it
        // spans tan(30°) × 10 = 5.77 m up and down, twice that, 11.5 m, to each side.
        let camera = Camera {
            eye: DVec3::ZERO,
            target: DVec3::X,
            ..Camera::default()
        };
        let frustum = Frustum::new(camera.projection(2.0) * camera.view_from_eye());
        for (center, seen, case) in [
            (Vec3::new(10.0, 0.0, 0.0), true, "ahead"),
            (Vec3::new(-10.0, 0.0, 0.0), false, "behind"),
            (Vec3::new(10.0, 0.0, 11.0), true, "11 m to the side"),
            (Vec3::new(10.0, 0.0, 12.5), false, "12.5 m to the side"),
            (Vec3::new(10.0, 7.0, 0.0), false, "7 m up"),
        ] {
            assert_eq!(frustum.holds(boxed(center, 0.25)), seen, "{case}");
        }
    }

    #[test]
    fn a_box_at_no_finite_place_is_not_seen() {
        let frustum = frustum();
        for value in [f32::NAN, f32::INFINITY] {
            let transform = Mat4::from_translation(Vec3::new(0.0, 0.0, value));
            assert!(
                !frustum.holds((transform, Vec3::ZERO, Vec3::ONE)),
                "{value}"
            );
        }
    }

    /// The ray through the point of the image where a point of the world is drawn passes through
    /// it, in perspective from near the eye, orthographic along the camera's axis.
    #[test]
    fn the_ray_through_a_point_of_the_image_passes_through_what_is_drawn_there() {
        let perspective = Camera {
            eye: DVec3::new(1000.0, 2.0, 5.0),
            target: DVec3::new(1000.0, 0.0, 0.0),
            ..Camera::default()
        };
        let orthographic = Camera {
            projection: Projection::Orthographic {
                height: 4.0,
                far: 50.0,
            },
            ..perspective
        };
        for camera in [perspective, orthographic] {
            for point in [
                DVec3::new(1000.0, 0.0, 0.0),
                DVec3::new(1001.0, 0.5, -0.5),
                DVec3::new(999.2, -0.3, 1.0),
            ] {
                let drawn = project(&camera, point);
                let on_image = Vec2::new(drawn.x as f32 + 1.0, 1.0 - drawn.y as f32) / 2.0;
                let (origin, direction) = camera.ray(1.0, on_image);
                assert!((direction.length() - 1.0).abs() < 1e-6);
                let along = (point - origin).dot(direction);
                let off = (origin + direction * along - point).length();
                assert!(
                    along > 0.0 && off < 1e-3,
                    "{camera:?}: {point} is {off} off the ray"
                );
            }
        }
        let (_, axis) = orthographic.ray(1.0, Vec2::new(0.1, 0.9));
        assert!(axis.abs_diff_eq((orthographic.target - orthographic.eye).normalize(), 1e-6));
    }
}
