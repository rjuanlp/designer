//! Vello scene construction for Designer.
//!
//! [`document`] turns a document page into a scene. The items at the crate
//! root are the deterministic probe scene used by the graphics gate. Nothing
//! here touches a GPU; building a scene is not evidence of presentation.

pub mod document;

use vello::{
    Scene,
    kurbo::{Affine, Circle, Rect},
    peniko::{Color, Fill},
};

/// Visual constants shared by the feasibility scene.
pub mod tokens {
    /// Width of the reference rendering target in physical pixels.
    pub const WIDTH: u32 = 960;
    /// Height of the reference rendering target in physical pixels.
    pub const HEIGHT: u32 = 640;
    /// Canvas background, in sRGB.
    pub const BACKGROUND: [u8; 3] = [24, 25, 29];
    /// Artboard background, in sRGB.
    pub const PAPER: [u8; 3] = [242, 244, 248];
    /// Animated shape accent, in sRGB.
    pub const ACCENT: [u8; 3] = [105, 87, 235];
    /// Artboard inset in physical pixels.
    pub const INSET: f64 = 64.0;
    /// Circle radius in physical pixels.
    pub const RADIUS: f64 = 40.0;
    /// Horizontal animation amplitude in physical pixels.
    pub const AMPLITUDE: f64 = 180.0;
    /// Period of the probe animation in seconds.
    pub const PERIOD: f64 = 2.0;
}

/// Return the animated center for a finite, nonnegative time.
///
/// Invalid input is rejected instead of allowing NaNs into GPU geometry.
pub fn center_at(seconds: f64) -> Option<(f64, f64)> {
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    let phase = (seconds % tokens::PERIOD) / tokens::PERIOD * std::f64::consts::TAU;
    Some((
        f64::from(tokens::WIDTH) / 2.0 + phase.sin() * tokens::AMPLITUDE,
        f64::from(tokens::HEIGHT) / 2.0,
    ))
}

/// Build a fresh scene at an absolute time; returns `None` for invalid time.
pub fn scene_at(seconds: f64) -> Option<Scene> {
    let center = center_at(seconds)?;
    let mut scene = Scene::new();
    let [r, g, b] = tokens::PAPER;
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgb8(r, g, b),
        None,
        &Rect::new(
            tokens::INSET,
            tokens::INSET,
            f64::from(tokens::WIDTH) - tokens::INSET,
            f64::from(tokens::HEIGHT) - tokens::INSET,
        ),
    );
    let [r, g, b] = tokens::ACCENT;
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgb8(r, g, b),
        None,
        &Circle::new(center, tokens::RADIUS),
    );
    Some(scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_times_never_reach_scene_encoding() {
        for time in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(scene_at(time).is_none());
        }
    }

    #[test]
    fn evaluation_is_absolute_periodic_and_bounded() {
        for step in 0..1000 {
            let time = f64::from(step) / 60.0;
            let (x, y) = center_at(time).expect("valid time");
            let (next_x, next_y) = center_at(time + tokens::PERIOD).expect("valid time");
            assert!((x - next_x).abs() < 1e-9);
            assert_eq!(y, next_y);
            assert!(x >= tokens::INSET + tokens::RADIUS);
            assert!(x <= f64::from(tokens::WIDTH) - tokens::INSET - tokens::RADIUS);
        }
    }
}
