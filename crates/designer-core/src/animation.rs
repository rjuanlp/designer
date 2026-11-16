//! Keyframe animation and its deterministic evaluation.
//!
//! Evaluation is a pure function of the page and an absolute time. It returns
//! a new page and never changes the base values stored in the document.

use serde::{Deserialize, Serialize};

use crate::{NodeId, Page};

/// Longest animation a page may have, in milliseconds.
pub const MAX_DURATION_MS: u32 = 600_000;
/// Most keyframes one track may hold.
pub const MAX_KEYS_PER_TRACK: usize = 4096;

/// A node value that can be animated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Property {
    /// Left edge, relative to the parent.
    X,
    /// Top edge, relative to the parent.
    Y,
    /// Width.
    Width,
    /// Height.
    Height,
    /// Opacity, 0 to 1.
    Opacity,
}

impl Property {
    /// Every animatable property.
    pub const ALL: [Property; 5] = [
        Property::X,
        Property::Y,
        Property::Width,
        Property::Height,
        Property::Opacity,
    ];

    /// Whether `value` is acceptable for this property.
    pub fn accepts(self, value: f64) -> bool {
        value.is_finite()
            && match self {
                Property::X | Property::Y => true,
                Property::Width | Property::Height => value >= 0.0,
                Property::Opacity => (0.0..=1.0).contains(&value),
            }
    }
}

/// How a value moves from one keyframe to the next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    /// Hold this value until the next keyframe.
    Constant,
    /// Move at constant speed to the next keyframe.
    #[default]
    Linear,
}

/// A value at a time.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    /// Time from the start of the animation.
    pub time_ms: u32,
    /// Value of the property at that time.
    pub value: f64,
    /// Interpolation towards the next keyframe.
    pub easing: Easing,
}

/// The keyframes of one property of one node, sorted by time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// The animated node.
    pub node: NodeId,
    /// The animated property.
    pub property: Property,
    /// Keyframes in strictly increasing time order. Never empty.
    pub keys: Vec<Keyframe>,
}

impl Track {
    /// Value at a time. Before the first key and after the last one the
    /// nearest key value holds.
    pub fn value_at(&self, time_ms: u32) -> Option<f64> {
        let first = self.keys.first()?;
        if time_ms <= first.time_ms {
            return Some(first.value);
        }
        let next_index = self.keys.partition_point(|key| key.time_ms <= time_ms);
        let from = self.keys[next_index - 1];
        let Some(to) = self.keys.get(next_index) else {
            return Some(from.value);
        };
        Some(match from.easing {
            Easing::Constant => from.value,
            Easing::Linear => {
                let span = f64::from(to.time_ms - from.time_ms);
                let progress = f64::from(time_ms - from.time_ms) / span;
                from.value + (to.value - from.value) * progress
            }
        })
    }
}

/// The animation of one page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    /// Length of the animation. Playback loops after it.
    pub duration_ms: u32,
    /// At most one track per node and property.
    pub tracks: Vec<Track>,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            duration_ms: 2000,
            tracks: Vec::new(),
        }
    }
}

impl Animation {
    /// The track of a node property, if it is animated.
    pub fn track(&self, node: NodeId, property: Property) -> Option<&Track> {
        self.tracks
            .iter()
            .find(|track| track.node == node && track.property == property)
    }

    /// Insert a keyframe, replacing one at the same time.
    pub(crate) fn set_key(&mut self, node: NodeId, property: Property, key: Keyframe) {
        let index = self
            .tracks
            .iter()
            .position(|track| track.node == node && track.property == property)
            .unwrap_or_else(|| {
                self.tracks.push(Track {
                    node,
                    property,
                    keys: Vec::new(),
                });
                self.tracks.len() - 1
            });
        let keys = &mut self.tracks[index].keys;
        match keys.binary_search_by_key(&key.time_ms, |existing| existing.time_ms) {
            Ok(found) => keys[found] = key,
            Err(insert_at) => keys.insert(insert_at, key),
        }
    }

    /// Remove a keyframe. A track left without keys is removed. Returns
    /// whether the keyframe existed.
    pub(crate) fn remove_key(&mut self, node: NodeId, property: Property, time_ms: u32) -> bool {
        let Some(index) = self
            .tracks
            .iter()
            .position(|track| track.node == node && track.property == property)
        else {
            return false;
        };
        let keys = &mut self.tracks[index].keys;
        let Ok(found) = keys.binary_search_by_key(&time_ms, |existing| existing.time_ms) else {
            return false;
        };
        keys.remove(found);
        if keys.is_empty() {
            self.tracks.remove(index);
        }
        true
    }

    /// Remove every track of the given nodes.
    pub(crate) fn remove_nodes(&mut self, removed: &[NodeId]) {
        self.tracks.retain(|track| !removed.contains(&track.node));
    }

    /// Check the invariants. `on_page` tells whether a node belongs to the
    /// page that owns this animation.
    pub(crate) fn validate(&self, on_page: impl Fn(NodeId) -> bool) -> Result<(), String> {
        if self.duration_ms == 0 || self.duration_ms > MAX_DURATION_MS {
            return Err(format!(
                "duration must be 1 to {MAX_DURATION_MS} ms, got {}",
                self.duration_ms
            ));
        }
        for (index, track) in self.tracks.iter().enumerate() {
            let label = format!("track of node {} {:?}", track.node.0, track.property);
            if !on_page(track.node) {
                return Err(format!("{label} targets a node that is not on the page"));
            }
            let duplicate = self.tracks[..index]
                .iter()
                .any(|other| other.node == track.node && other.property == track.property);
            if duplicate {
                return Err(format!("{label} is defined twice"));
            }
            if track.keys.is_empty() || track.keys.len() > MAX_KEYS_PER_TRACK {
                return Err(format!(
                    "{label} must have 1 to {MAX_KEYS_PER_TRACK} keyframes"
                ));
            }
            if !track
                .keys
                .windows(2)
                .all(|pair| pair[0].time_ms < pair[1].time_ms)
            {
                return Err(format!(
                    "{label} keyframes are not in increasing time order"
                ));
            }
            for key in &track.keys {
                if key.time_ms > self.duration_ms {
                    return Err(format!("{label} has a keyframe after the end"));
                }
                if !track.property.accepts(key.value) {
                    return Err(format!("{label} has an invalid value {}", key.value));
                }
            }
        }
        Ok(())
    }
}

impl Page {
    /// The page as it looks at an absolute time of its animation.
    ///
    /// The time is clamped to the duration. Nodes without tracks keep their
    /// base values, and `self` is not modified.
    pub fn at_time(&self, time_ms: u32) -> Page {
        let mut page = self.clone();
        let time_ms = time_ms.min(self.animation.duration_ms);
        for track in &self.animation.tracks {
            let (Some(value), Some(node)) = (track.value_at(time_ms), page.node_mut(track.node))
            else {
                continue;
            };
            match track.property {
                Property::X => node.bounds.x = value,
                Property::Y => node.bounds.y = value,
                Property::Width => node.bounds.width = value,
                Property::Height => node.bounds.height = value,
                Property::Opacity => node.opacity = value,
            }
        }
        page
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, Node, NodeKind, PageId, Rect};

    fn key(time_ms: u32, value: f64, easing: Easing) -> Keyframe {
        Keyframe {
            time_ms,
            value,
            easing,
        }
    }

    fn page() -> Page {
        Page {
            id: PageId(1),
            name: String::new(),
            background: Color::rgb(0, 0, 0),
            nodes: vec![Node {
                id: NodeId(1),
                name: String::new(),
                kind: NodeKind::Ellipse,
                bounds: Rect::new(10.0, 20.0, 30.0, 40.0),
                fill: None,
                opacity: 1.0,
                visible: true,
                locked: false,
                children: Vec::new(),
            }],
            animation: Animation::default(),
        }
    }

    #[test]
    fn interpolates_holds_and_clamps() {
        let track = Track {
            node: NodeId(1),
            property: Property::X,
            keys: vec![
                key(100, 0.0, Easing::Linear),
                key(300, 50.0, Easing::Constant),
                key(500, 80.0, Easing::Linear),
            ],
        };
        assert_eq!(track.value_at(0), Some(0.0));
        assert_eq!(track.value_at(100), Some(0.0));
        assert_eq!(track.value_at(200), Some(25.0));
        assert_eq!(track.value_at(300), Some(50.0));
        assert_eq!(track.value_at(499), Some(50.0));
        assert_eq!(track.value_at(500), Some(80.0));
        assert_eq!(track.value_at(9000), Some(80.0));
    }

    #[test]
    fn evaluation_is_deterministic_and_leaves_base_values_alone() {
        let mut page = page();
        page.animation
            .set_key(NodeId(1), Property::X, key(1000, 110.0, Easing::Linear));
        page.animation
            .set_key(NodeId(1), Property::X, key(0, 10.0, Easing::Linear));
        page.animation
            .set_key(NodeId(1), Property::Opacity, key(0, 0.5, Easing::Linear));
        let base = page.clone();

        let middle = page.at_time(500);
        assert_eq!(middle.nodes[0].bounds.x, 60.0);
        assert_eq!(middle.nodes[0].bounds.y, 20.0);
        assert_eq!(middle.nodes[0].opacity, 0.5);
        assert_eq!(page.at_time(500), middle);
        assert_eq!(page, base);
        // Past the end the time is clamped to the duration.
        assert_eq!(page.at_time(u32::MAX), page.at_time(2000));
    }

    #[test]
    fn keys_stay_sorted_and_replace_at_the_same_time() {
        let mut animation = Animation::default();
        for (time, value) in [(500, 1.0), (0, 2.0), (500, 3.0), (250, 4.0)] {
            animation.set_key(NodeId(1), Property::Y, key(time, value, Easing::Linear));
        }
        let track = animation.track(NodeId(1), Property::Y).expect("track");
        let pairs: Vec<_> = track.keys.iter().map(|k| (k.time_ms, k.value)).collect();
        assert_eq!(pairs, [(0, 2.0), (250, 4.0), (500, 3.0)]);
        assert_eq!(animation.validate(|_| true), Ok(()));

        assert!(!animation.remove_key(NodeId(1), Property::Y, 1));
        for time in [0, 250, 500] {
            assert!(animation.remove_key(NodeId(1), Property::Y, time));
        }
        assert!(animation.tracks.is_empty());
    }

    #[test]
    fn validation_rejects_inconsistent_tracks() {
        let valid = |animation: &Animation| animation.validate(|id| id == NodeId(1));
        let with = |keys: Vec<Keyframe>, property| Animation {
            duration_ms: 1000,
            tracks: vec![Track {
                node: NodeId(1),
                property,
                keys,
            }],
        };
        let linear = |time, value| key(time, value, Easing::Linear);

        assert!(valid(&with(vec![linear(0, 1.0)], Property::X)).is_ok());
        assert!(valid(&with(vec![], Property::X)).is_err());
        assert!(valid(&with(vec![linear(5, 1.0), linear(5, 2.0)], Property::X)).is_err());
        assert!(valid(&with(vec![linear(9, 1.0), linear(5, 2.0)], Property::X)).is_err());
        assert!(valid(&with(vec![linear(1001, 1.0)], Property::X)).is_err());
        assert!(valid(&with(vec![linear(0, f64::NAN)], Property::X)).is_err());
        assert!(valid(&with(vec![linear(0, -1.0)], Property::Width)).is_err());
        assert!(valid(&with(vec![linear(0, 1.5)], Property::Opacity)).is_err());

        let mut orphan = with(vec![linear(0, 1.0)], Property::X);
        orphan.tracks[0].node = NodeId(2);
        assert!(valid(&orphan).is_err());

        let mut twice = with(vec![linear(0, 1.0)], Property::X);
        twice.tracks.push(twice.tracks[0].clone());
        assert!(valid(&twice).is_err());

        let mut endless = Animation::default();
        endless.duration_ms = 0;
        assert!(valid(&endless).is_err());
    }
}
