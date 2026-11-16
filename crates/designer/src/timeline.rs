//! Transient playback state; it never mutates the document or its history.

use std::time::Instant;

use designer_core::{Easing, Node, Property};

pub(crate) struct Timeline {
    pub time_ms: u32,
    pub preview: bool,
    pub property: Property,
    pub easing: Easing,
    playing: Option<(Instant, u32)>,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            time_ms: 0,
            preview: false,
            property: Property::X,
            easing: Easing::Linear,
            playing: None,
        }
    }
}

impl Timeline {
    pub fn is_playing(&self) -> bool {
        self.playing.is_some()
    }

    pub fn seek(&mut self, time_ms: u32, duration: u32) {
        self.playing = None;
        self.preview = true;
        self.time_ms = time_ms.min(duration);
    }

    pub fn pause(&mut self) {
        self.playing = None;
    }

    pub fn toggle_play(&mut self, now: Instant, duration: u32) {
        if self.playing.take().is_none() {
            if self.time_ms >= duration {
                self.time_ms = 0;
            }
            self.preview = true;
            self.playing = Some((now, self.time_ms));
        }
    }

    pub fn tick(&mut self, now: Instant, duration: u32) {
        if let Some((start, offset)) = self.playing {
            self.time_ms = playback_time(offset, now.duration_since(start).as_millis(), duration);
        } else {
            self.time_ms = self.time_ms.min(duration);
        }
    }
}

fn playback_time(offset: u32, elapsed_ms: u128, duration: u32) -> u32 {
    ((u128::from(offset) + elapsed_ms) % u128::from(duration.max(1))) as u32
}

pub(crate) fn value(node: &Node, property: Property) -> f64 {
    match property {
        Property::X => node.bounds.x,
        Property::Y => node.bounds.y,
        Property::Width => node.bounds.width,
        Property::Height => node.bounds.height,
        Property::Opacity => node.opacity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn playback_uses_elapsed_time_loops_and_pauses() {
        let start = Instant::now();
        let mut timeline = Timeline::default();
        timeline.seek(500, 2000);
        timeline.toggle_play(start, 2000);
        timeline.tick(start + Duration::from_millis(2250), 2000);
        assert_eq!(timeline.time_ms, 750);
        timeline.pause();
        timeline.tick(start + Duration::from_secs(10), 2000);
        assert_eq!(timeline.time_ms, 750);
        assert!(timeline.preview);
    }

    #[test]
    fn seeking_stops_playback_and_end_restarts_from_zero() {
        let start = Instant::now();
        let mut timeline = Timeline::default();
        timeline.toggle_play(start, 2000);
        timeline.seek(5000, 2000);
        assert!(!timeline.is_playing());
        assert_eq!(timeline.time_ms, 2000);
        timeline.toggle_play(start, 2000);
        assert_eq!(timeline.time_ms, 0);
        timeline.tick(start + Duration::from_millis(100), 2000);
        assert_eq!(timeline.time_ms, 100);
    }
}
