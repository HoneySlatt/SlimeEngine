//! How a viewport's camera moves (`docs/adr/0158-the-viewport.md`): it flies, W, A, S, D, Q and E
//! moving it, orbits and pans while the button that the table of shortcuts binds to each is held,
//! the right button, Alt and the left button and the middle one by default
//! (`docs/adr/0160-two-viewports-and-blenders-way.md`), and frames what is selected; the cursor is
//! held meanwhile, the mouse's raw motion turning it. It needs no GPU and no window: the window
//! gives it the surface's actions, the raw motion and the keys, and draws from its camera.

use std::time::{Duration, Instant};

use slime_platform::{KeyCode, MouseButton};
use slime_render::glam::DVec3;
use slime_render::{Camera, Projection};

use crate::picking::DRAG;

/// How far the camera turns for a unit of the mouse's raw motion, in radians: a turn of the whole
/// way round for some 3 000 units, about a forearm's sweep on a usual mouse.
const TURN: f64 = 0.002;

/// How far the camera looks up or down at most, in radians, short of straight up and down, where
/// its heading would mean nothing.
const PITCH_LIMIT: f64 = 1.55;

/// How far a pan moves the camera for a unit of the raw motion, as a share of its distance from
/// the point that it looks at, so that a pan moves what is under the cursor about as fast near and
/// far.
const PAN: f64 = 0.0015;

/// The speed of flying at first, in metres per second, a brisk walk's thrice.
const FLY_SPEED: f64 = 5.0;

/// The bounds of the flying speed that the wheel changes, in metres per second.
const FLY_SPEEDS: (f64, f64) = (0.1, 500.0);

/// How much a line of the wheel changes the flying speed, or the distance from the point looked at.
const WHEEL_STEP: f64 = 1.2;

/// How much faster Shift flies.
const FAST: f64 = 4.0;

/// The longest step of flying between two frames: a frame that comes late does not throw the
/// camera far.
const LONGEST_STEP: Duration = Duration::from_millis(100);

/// The nearest that the camera comes to the point that it looks at, in metres.
const NEAREST: f64 = 0.05;

/// The gesture that moves the camera, while the button that started it is held, which the table of
/// shortcuts binds (`docs/adr/0160-two-viewports-and-blenders-way.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gesture {
    /// `fly`: the mouse turns the camera where it stands, the keys move it.
    Fly,
    /// `orbit`: the mouse turns the camera around the point that it looks at.
    Orbit,
    /// `pan`: the mouse moves the camera and the point that it looks at, side to side and up and
    /// down.
    Pan,
}

impl Gesture {
    /// The gesture that a button's chord names `name` in the table of shortcuts, if it is one.
    pub(crate) fn named(name: &str) -> Option<Self> {
        match name {
            "fly" => Some(Self::Fly),
            "orbit" => Some(Self::Orbit),
            "pan" => Some(Self::Pan),
            _ => None,
        }
    }
}

/// The keys that fly, by their places on the keyboard, whatever its layout: forward, back, left,
/// right, down and up.
const FLY_KEYS: [KeyCode; 6] = [
    KeyCode::KeyW,
    KeyCode::KeyS,
    KeyCode::KeyA,
    KeyCode::KeyD,
    KeyCode::KeyQ,
    KeyCode::KeyE,
];

/// The viewport's camera and how the user moves it.
#[derive(Clone, Debug)]
pub(crate) struct Navigation {
    /// The point that the camera looks at, which it orbits.
    pivot: DVec3,
    /// The camera's heading about the vertical, in radians, 0 looking along -Z, growing towards +X.
    yaw: f64,
    /// How far it looks up, in radians, down negative.
    pitch: f64,
    /// How far the camera stands from the point that it looks at, in metres.
    distance: f64,
    /// The gesture under way, while the button that started it is held, and that button.
    gesture: Option<(Gesture, MouseButton)>,
    /// The flying keys held, in the order of [`FLY_KEYS`], and whether Shift is.
    keys: [bool; 6],
    fast: bool,
    /// Where the pointer last was on the viewport during a gesture, in logical pixels: where the
    /// cursor cannot be held, its moves turn the camera in place of the raw motion.
    pointer: [f32; 2],
    /// When flying last moved the camera, while a key flies it.
    flown: Option<Instant>,
    /// How far the mouse moved during the gesture under way, in units of its raw motion or logical
    /// pixels.
    moved: f64,
    /// The flying speed, in metres per second.
    speed: f64,
}

impl Default for Navigation {
    /// A camera that looks down at the origin from 12 metres, from the south-east.
    fn default() -> Self {
        Self {
            pivot: DVec3::ZERO,
            yaw: -0.6,
            pitch: -0.45,
            distance: 12.0,
            gesture: None,
            keys: [false; 6],
            fast: false,
            pointer: [0.0; 2],
            flown: None,
            moved: 0.0,
            speed: FLY_SPEED,
        }
    }
}

impl Navigation {
    /// The direction that the camera looks in.
    fn forward(&self) -> DVec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        DVec3::new(sin_yaw * cos_pitch, sin_pitch, -cos_yaw * cos_pitch)
    }

    /// The direction to the camera's right, level.
    fn right(&self) -> DVec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        DVec3::new(cos_yaw, 0.0, sin_yaw)
    }

    /// A navigation whose camera stands and looks as this one's does, flying as fast, no gesture
    /// under way nor key held.
    pub(crate) fn looking_as(&self) -> Self {
        Self {
            pivot: self.pivot,
            yaw: self.yaw,
            pitch: self.pitch,
            distance: self.distance,
            speed: self.speed,
            ..Self::default()
        }
    }

    /// Where the camera stands.
    pub(crate) fn eye(&self) -> DVec3 {
        self.pivot - self.forward() * self.distance
    }

    /// The camera, with the renderer's default lens.
    pub(crate) fn camera(&self) -> Camera {
        Camera {
            eye: self.eye(),
            target: self.pivot,
            ..Camera::default()
        }
    }

    /// Whether a gesture holds the cursor: the window holds it, hidden, while it does.
    pub(crate) fn holds_cursor(&self) -> bool {
        self.gesture.is_some()
    }

    /// Whether it flies: the keys that fly reach it, not the interface.
    pub(crate) fn flying(&self) -> bool {
        self.gesture
            .is_some_and(|(gesture, _)| gesture == Gesture::Fly)
    }

    /// `button` went down over the viewport at `at`, in logical pixels, which starts `gesture`, if
    /// none is under way. Gives whether it started.
    pub(crate) fn press(&mut self, gesture: Gesture, button: MouseButton, at: [f32; 2]) -> bool {
        if self.gesture.is_some() {
            return false;
        }
        self.gesture = Some((gesture, button));
        self.pointer = at;
        self.moved = 0.0;
        true
    }

    /// The pointer moved to `at` on the viewport, in logical pixels, during a gesture: where the
    /// cursor is held, no move comes, and the raw motion moves the camera; where it cannot be held,
    /// the moves do, as [`Navigation::motion`] says. Gives whether it moved the camera.
    pub(crate) fn pointer(&mut self, at: [f32; 2]) -> bool {
        let delta = [
            f64::from(at[0] - self.pointer[0]),
            f64::from(at[1] - self.pointer[1]),
        ];
        self.pointer = at;
        self.motion(delta)
    }

    /// Shift is held or not, which flies four times faster.
    pub(crate) fn set_fast(&mut self, fast: bool) {
        self.fast = fast;
    }

    /// A button that went down over the viewport went up: the gesture that it holds ends, and the
    /// keys stop flying. Gives whether it was an orbit that the mouse moved less than a click may
    /// ([`DRAG`]), its raw motion's units taken for pixels: of the left button, as Alt and the left
    /// button orbit by default, a click that goes through what lies behind what is selected
    /// (`docs/adr/0159-picking-and-the-selection.md`).
    pub(crate) fn release(&mut self, button: MouseButton) -> bool {
        let Some((gesture, _)) = self.gesture.filter(|&(_, held)| held == button) else {
            return false;
        };
        self.gesture = None;
        self.keys = [false; 6];
        self.flown = None;
        gesture == Gesture::Orbit && self.moved < f64::from(DRAG)
    }

    /// The mouse moved by `delta` of its raw motion, right and down: the gesture under way turns or
    /// moves the camera. Gives whether it moved.
    pub(crate) fn motion(&mut self, [x, y]: [f64; 2]) -> bool {
        if self.gesture.is_some() {
            self.moved += x.hypot(y);
        }
        match self.gesture.map(|(gesture, _)| gesture) {
            Some(Gesture::Fly) => {
                // Turned where it stands: the point that it looks at moves.
                let eye = self.eye();
                self.turn(x, y);
                self.pivot = eye + self.forward() * self.distance;
            }
            Some(Gesture::Orbit) => self.turn(x, y),
            Some(Gesture::Pan) => {
                let up = self.right().cross(self.forward());
                let scale = self.distance * PAN;
                self.pivot += (-self.right() * x + up * y) * scale;
            }
            None => return false,
        }
        true
    }

    fn turn(&mut self, x: f64, y: f64) {
        self.yaw = (self.yaw + x * TURN).rem_euclid(std::f64::consts::TAU);
        self.pitch = (self.pitch - y * TURN).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// A key went down or up, at `now`: one that flies, while the camera flies. Gives whether it
    /// took it.
    pub(crate) fn key(&mut self, key: KeyCode, down: bool, now: Instant) -> bool {
        if !self.flying() {
            return false;
        }
        let Some(place) = FLY_KEYS.iter().position(|&fly| fly == key) else {
            return false;
        };
        self.keys[place] = down;
        // Flying from now on, keys held that cancelled out included, once one is let go.
        if self.keys.contains(&true) && self.flown.is_none() {
            self.flown = Some(now);
        }
        true
    }

    /// Moves the camera as the keys held fly it, up to `now`. Gives whether the keys fly it still,
    /// which asks for the next frame.
    pub(crate) fn advance(&mut self, now: Instant) -> bool {
        let Some(since) = self.flown else {
            return false;
        };
        let along = |forward: usize, back: usize| {
            f64::from(u8::from(self.keys[forward])) - f64::from(u8::from(self.keys[back]))
        };
        let direction =
            self.forward() * along(0, 1) + self.right() * along(3, 2) + DVec3::Y * along(5, 4);
        if direction == DVec3::ZERO {
            self.flown = None;
            return false;
        }
        let step = now.saturating_duration_since(since).min(LONGEST_STEP);
        let speed = if self.fast {
            self.speed * FAST
        } else {
            self.speed
        };
        self.pivot += direction.normalize() * speed * step.as_secs_f64();
        self.flown = Some(now);
        true
    }

    /// The wheel turned by `lines`, up positive: flying, it changes the speed; otherwise, it comes
    /// nearer the point that the camera looks at, or goes farther.
    pub(crate) fn wheel(&mut self, lines: f64) {
        let factor = WHEEL_STEP.powf(lines);
        if self.flying() {
            self.speed = (self.speed * factor).clamp(FLY_SPEEDS.0, FLY_SPEEDS.1);
        } else {
            self.distance = (self.distance / factor).max(NEAREST);
        }
    }

    /// Frames a sphere around `center` of `radius` metres in a viewport `aspect` times as wide as
    /// it is tall: the camera looks at its center, as far as the renderer's default lens shows it
    /// whole across the viewport's narrower side, keeping its heading.
    pub(crate) fn frame(&mut self, center: DVec3, radius: f64, aspect: f64) {
        self.pivot = center;
        self.distance = (radius.max(NEAREST) / half_view(aspect).sin()).max(NEAREST);
    }
}

/// Half the field of view of the renderer's default lens across the narrower side of an image
/// `aspect` times as wide as it is tall, in radians.
fn half_view(aspect: f64) -> f64 {
    let vertical = match Camera::default().projection {
        Projection::Perspective { fov_y } => f64::from(fov_y) / 2.0,
        Projection::Orthographic { .. } => std::f64::consts::FRAC_PI_4,
    };
    let horizontal = (vertical.tan() * aspect.max(f64::EPSILON)).atan();
    vertical.min(horizontal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: DVec3, b: DVec3) -> bool {
        a.distance(b) < 1e-9
    }

    #[test]
    fn the_right_button_flies_turning_where_the_camera_stands() {
        let mut navigation = Navigation::default();
        let eye = navigation.eye();
        assert!(navigation.press(Gesture::Fly, MouseButton::Right, [0.0; 2]));
        assert!(navigation.holds_cursor());
        assert!(navigation.motion([300.0, -100.0]));
        assert!(near(navigation.eye(), eye), "turned where it stands");
        // W held for a tenth of a second moves it forward at its speed.
        let start = Instant::now();
        let forward = navigation.forward();
        assert!(navigation.key(KeyCode::KeyW, true, start));
        assert!(navigation.advance(start + Duration::from_millis(50)));
        assert!(navigation.advance(start + Duration::from_millis(100)));
        let flown = navigation.eye() - eye;
        assert!(near(flown, forward * FLY_SPEED * 0.1), "{flown}");
        // Let go of the button: the keys stop flying, and reach the interface again.
        navigation.release(MouseButton::Right);
        assert!(!navigation.holds_cursor());
        assert!(!navigation.advance(start + Duration::from_millis(150)));
        assert!(!navigation.key(KeyCode::KeyW, true, start));
    }

    #[test]
    fn alt_and_the_left_button_orbit_around_the_point_looked_at() {
        let mut navigation = Navigation::default();
        let (pivot, distance) = (navigation.pivot, navigation.distance);
        assert!(navigation.press(Gesture::Orbit, MouseButton::Left, [0.0; 2]));
        navigation.motion([500.0, 200.0]);
        assert!(near(navigation.pivot, pivot));
        assert!((navigation.eye().distance(pivot) - distance).abs() < 1e-9);
        // It never looks straight down, however far the mouse goes.
        navigation.motion([0.0, 100_000.0]);
        assert_eq!(navigation.pitch, -PITCH_LIMIT);
        // Another button meanwhile starts nothing.
        assert!(!navigation.press(Gesture::Pan, MouseButton::Middle, [0.0; 2]));
        assert!(
            !navigation.release(MouseButton::Left),
            "it moved: not a click"
        );
        assert!(!navigation.holds_cursor());
        // Pressed and let go before the mouse moved, it is Alt and a click.
        navigation.press(Gesture::Orbit, MouseButton::Left, [0.0; 2]);
        navigation.motion([1.0, -1.0]);
        assert!(navigation.release(MouseButton::Left));
        // Not so for the other gestures.
        navigation.press(Gesture::Fly, MouseButton::Right, [0.0; 2]);
        assert!(!navigation.release(MouseButton::Right));
    }

    #[test]
    fn the_middle_button_pans_the_camera_and_what_it_looks_at_together() {
        let mut navigation = Navigation::default();
        let (eye, pivot) = (navigation.eye(), navigation.pivot);
        navigation.press(Gesture::Pan, MouseButton::Middle, [0.0; 2]);
        navigation.motion([100.0, 0.0]);
        let moved = navigation.pivot - pivot;
        assert!(near(navigation.eye() - eye, moved), "both moved alike");
        // Dragged right, the world follows the pointer: the camera moves left.
        assert!(moved.dot(navigation.right()) < 0.0);
        assert!(
            moved.dot(navigation.forward()).abs() < 1e-9,
            "level, not forward"
        );
    }

    #[test]
    fn the_wheel_comes_nearer_or_flies_faster() {
        let mut navigation = Navigation::default();
        let distance = navigation.distance;
        navigation.wheel(1.0);
        assert!((navigation.distance - distance / WHEEL_STEP).abs() < 1e-9);
        navigation.press(Gesture::Fly, MouseButton::Right, [0.0; 2]);
        navigation.wheel(2.0);
        assert!((navigation.speed - FLY_SPEED * WHEEL_STEP * WHEEL_STEP).abs() < 1e-9);
        navigation.wheel(-1000.0);
        assert_eq!(navigation.speed, FLY_SPEEDS.0);
    }

    #[test]
    fn framing_shows_the_sphere_whole_from_the_same_heading() {
        let mut navigation = Navigation::default();
        let forward = navigation.forward();
        let center = DVec3::new(10.0, 2.0, -4.0);
        // In a viewport twice as tall as wide, the sphere fits across its width.
        for aspect in [16.0 / 9.0, 0.5] {
            navigation.frame(center, 3.0, aspect);
            assert!(near(navigation.forward(), forward));
            assert!(near(navigation.camera().target, center));
            let seen = (3.0 / navigation.eye().distance(center)).asin();
            assert!(seen <= half_view(aspect) + 1e-9);
        }
        assert!(half_view(0.5) < half_view(16.0 / 9.0));
    }

    #[test]
    fn where_the_cursor_is_free_its_moves_turn_the_camera() {
        let mut navigation = Navigation::default();
        let yaw = navigation.yaw;
        navigation.press(Gesture::Orbit, MouseButton::Left, [100.0, 100.0]);
        assert!(navigation.pointer([150.0, 100.0]));
        let turned = (yaw + 50.0 * TURN).rem_euclid(std::f64::consts::TAU);
        assert!((navigation.yaw - turned).abs() < 1e-9);
    }

    #[test]
    fn keys_that_cancel_out_fly_again_as_one_is_let_go_and_shift_follows_its_key() {
        let mut navigation = Navigation::default();
        let start = Instant::now();
        let at = |milliseconds| start + Duration::from_millis(milliseconds);
        navigation.press(Gesture::Fly, MouseButton::Right, [0.0; 2]);
        navigation.key(KeyCode::KeyW, true, at(0));
        navigation.key(KeyCode::KeyS, true, at(0));
        assert!(!navigation.advance(at(20)), "W and S cancel out");
        navigation.key(KeyCode::KeyS, false, at(30));
        let eye = navigation.eye();
        assert!(navigation.advance(at(80)), "W alone flies again");
        assert!(navigation.eye().distance(eye) > 0.0);
        // Shift, held or not, as the modifiers say, whatever the gesture.
        navigation.set_fast(true);
        navigation.release(MouseButton::Right);
        navigation.set_fast(false);
        assert!(!navigation.fast);
    }
}
