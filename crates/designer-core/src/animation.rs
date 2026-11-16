//! Keyframe animation and its deterministic evaluation.
//!
//! Evaluation is a pure function of the page and an absolute time. It returns
//! a new page and never changes the base values stored in the document.

use serde::{Deserialize, Serialize};

use crate::{NodeId, Page, PathData};

/// Longest animation a page may have, in milliseconds.
pub const MAX_DURATION_MS: u32 = 600_000;
/// Most keyframes one track may hold.
pub const MAX_KEYS_PER_TRACK: usize = 4096;
/// Maximum number of nested sequences in one page animation.
pub const MAX_SEQUENCES: usize = 256;
/// Maximum nesting depth, with top-level sequences at depth one.
pub const MAX_SEQUENCE_DEPTH: usize = 8;

/// Identifier of a sequence, unique within a page animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SequenceId(pub u64);

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

fn set_morph_key(tracks: &mut Vec<MorphTrack>, node: NodeId, key: MorphKeyframe) {
    let index = tracks
        .iter()
        .position(|track| track.node == node)
        .unwrap_or_else(|| {
            tracks.push(MorphTrack {
                node,
                keys: Vec::new(),
            });
            tracks.len() - 1
        });
    let keys = &mut tracks[index].keys;
    match keys.binary_search_by_key(&key.time_ms, |existing| existing.time_ms) {
        Ok(found) => keys[found] = key,
        Err(insert_at) => keys.insert(insert_at, key),
    }
}

fn remove_morph_key(tracks: &mut Vec<MorphTrack>, node: NodeId, time_ms: u32) -> bool {
    let Some(index) = tracks.iter().position(|track| track.node == node) else {
        return false;
    };
    let keys = &mut tracks[index].keys;
    let Ok(found) = keys.binary_search_by_key(&time_ms, |key| key.time_ms) else {
        return false;
    };
    keys.remove(found);
    if keys.is_empty() {
        tracks.remove(index);
    }
    true
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

fn validate_morph_tracks(
    tracks: &[MorphTrack],
    duration_ms: u32,
    path_of: &impl Fn(NodeId) -> Option<PathData>,
    owner: &str,
) -> Result<(), String> {
    for (index, track) in tracks.iter().enumerate() {
        let label = format!("{owner} morph track of node {}", track.node.0);
        let Some(base_path) = path_of(track.node) else {
            return Err(format!("{label} targets a node that is not a path"));
        };
        if tracks[..index].iter().any(|other| other.node == track.node) {
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
        let mut previous = &base_path;
        for key in &track.keys {
            if key.time_ms > duration_ms {
                return Err(format!("{label} has a keyframe after the end"));
            }
            if !key.path.is_valid() || !previous.has_same_topology(&key.path) {
                return Err(format!("{label} has invalid or incompatible path topology"));
            }
            previous = &key.path;
        }
    }
    Ok(())
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
    /// Accelerate from the starting value.
    EaseIn,
    /// Decelerate into the ending value.
    EaseOut,
    /// Accelerate and then decelerate.
    EaseInOut,
}

fn apply_morph_tracks(page: &mut Page, tracks: &[MorphTrack], time_ms: u32) {
    for track in tracks {
        let (Some(path), Some(node)) = (track.path_at(time_ms), page.node_mut(track.node)) else {
            continue;
        };
        if let crate::NodeKind::Path { path: node_path } = &mut node.kind {
            *node_path = path;
        }
    }
}

impl Easing {
    /// Normalized progress for the selected curve.
    pub fn progress(self, progress: f64) -> f64 {
        let progress = progress.clamp(0.0, 1.0);
        match self {
            Self::Constant if progress < 1.0 => 0.0,
            Self::Constant => 1.0,
            Self::Linear => progress,
            Self::EaseIn => cubic_bezier(progress, 0.68, 0.0, 0.77, 0.0),
            Self::EaseOut => cubic_bezier(progress, 0.23, 1.0, 0.32, 1.0),
            Self::EaseInOut => cubic_bezier(progress, 0.77, 0.0, 0.23, 1.0),
        }
    }

    /// The next curve in the timeline's cycle.
    pub fn next(self) -> Self {
        match self {
            Self::Constant => Self::Linear,
            Self::Linear => Self::EaseIn,
            Self::EaseIn => Self::EaseOut,
            Self::EaseOut => Self::EaseInOut,
            Self::EaseInOut => Self::Constant,
        }
    }
}

fn cubic_bezier(progress: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    if progress == 0.0 || progress == 1.0 {
        return progress;
    }
    let axis = |t: f64, control1: f64, control2: f64| {
        let inverse = 1.0 - t;
        3.0 * inverse * inverse * t * control1 + 3.0 * inverse * t * t * control2 + t * t * t
    };
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..24 {
        let middle = (low + high) / 2.0;
        if axis(middle, x1, x2) < progress {
            low = middle;
        } else {
            high = middle;
        }
    }
    axis((low + high) / 2.0, y1, y2)
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
        let span = f64::from(to.time_ms - from.time_ms);
        let progress = f64::from(time_ms - from.time_ms) / span;
        Some(from.value + (to.value - from.value) * from.easing.progress(progress))
    }
}

/// A path geometry at a time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MorphKeyframe {
    /// Time from the start of the animation or sequence.
    pub time_ms: u32,
    /// Path geometry at that time.
    pub path: PathData,
    /// Interpolation towards the next keyframe.
    pub easing: Easing,
}

/// Keyframes that morph one path node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MorphTrack {
    /// The animated path node.
    pub node: NodeId,
    /// Keyframes in strictly increasing time order. Never empty.
    pub keys: Vec<MorphKeyframe>,
}

impl MorphTrack {
    /// Path at a time. Before the first key and after the last one the nearest
    /// key geometry holds.
    pub fn path_at(&self, time_ms: u32) -> Option<PathData> {
        let first = self.keys.first()?;
        if time_ms <= first.time_ms {
            return Some(first.path.clone());
        }
        let next_index = self.keys.partition_point(|key| key.time_ms <= time_ms);
        let from = &self.keys[next_index - 1];
        let Some(to) = self.keys.get(next_index) else {
            return Some(from.path.clone());
        };
        let span = f64::from(to.time_ms - from.time_ms);
        let progress = f64::from(time_ms - from.time_ms) / span;
        from.path
            .interpolate(&to.path, from.easing.progress(progress))
    }
}

/// A nested animation with its own local clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sequence {
    /// Identity within the page animation.
    pub id: SequenceId,
    /// Name shown in the timeline.
    pub name: String,
    /// Start time relative to the parent sequence, or the page for a root.
    pub offset_ms: u32,
    /// Local duration. Once it ends, its last values are held.
    pub duration_ms: u32,
    /// Animated properties in this sequence.
    pub tracks: Vec<Track>,
    /// Path morphs in this sequence.
    #[serde(default)]
    pub morph_tracks: Vec<MorphTrack>,
    /// Child sequences with offsets relative to this sequence.
    pub children: Vec<Sequence>,
}

impl Sequence {
    /// The track of a node property in this sequence, if present.
    pub fn track(&self, node: NodeId, property: Property) -> Option<&Track> {
        self.tracks
            .iter()
            .find(|track| track.node == node && track.property == property)
    }

    fn find(&self, id: SequenceId) -> Option<&Sequence> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.find(id))
    }

    fn find_mut(&mut self, id: SequenceId) -> Option<&mut Sequence> {
        if self.id == id {
            return Some(self);
        }
        self.children
            .iter_mut()
            .find_map(|child| child.find_mut(id))
    }
}

/// The animation of one page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    /// Length of the animation. Playback loops after it.
    pub duration_ms: u32,
    /// At most one track per node and property.
    pub tracks: Vec<Track>,
    /// Path morph tracks.
    #[serde(default)]
    pub morph_tracks: Vec<MorphTrack>,
    /// Nested sequences, each with a local clock.
    #[serde(default)]
    pub sequences: Vec<Sequence>,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            duration_ms: 2000,
            tracks: Vec::new(),
            morph_tracks: Vec::new(),
            sequences: Vec::new(),
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

    /// Find a nested sequence by its identifier.
    pub fn sequence(&self, id: SequenceId) -> Option<&Sequence> {
        self.sequences.iter().find_map(|sequence| sequence.find(id))
    }

    /// Return the sequence's absolute start, local duration, and parent duration.
    pub fn sequence_timing(&self, id: SequenceId) -> Option<(u32, u32, u32)> {
        fn find(
            sequences: &[Sequence],
            id: SequenceId,
            parent_offset: u32,
            parent_duration: u32,
        ) -> Option<(u32, u32, u32)> {
            for sequence in sequences {
                let absolute_offset = parent_offset.checked_add(sequence.offset_ms)?;
                if sequence.id == id {
                    return Some((absolute_offset, sequence.duration_ms, parent_duration));
                }
                if let Some(timing) = find(
                    &sequence.children,
                    id,
                    absolute_offset,
                    sequence.duration_ms,
                ) {
                    return Some(timing);
                }
            }
            None
        }
        find(&self.sequences, id, 0, self.duration_ms)
    }

    /// Find a sequence by its identifier, for modification.
    pub(crate) fn sequence_mut(&mut self, id: SequenceId) -> Option<&mut Sequence> {
        self.sequences
            .iter_mut()
            .find_map(|sequence| sequence.find_mut(id))
    }

    /// Return the next available identifier, if identifiers are not exhausted.
    pub fn next_sequence_id(&self) -> Option<SequenceId> {
        fn maximum(sequences: &[Sequence]) -> u64 {
            sequences.iter().fold(0, |max, sequence| {
                max.max(sequence.id.0).max(maximum(&sequence.children))
            })
        }
        maximum(&self.sequences).checked_add(1).map(SequenceId)
    }

    /// Insert a sequence at the page level or under an existing sequence.
    pub(crate) fn add_sequence(
        &mut self,
        parent: Option<SequenceId>,
        sequence: Sequence,
    ) -> Result<(), SequenceId> {
        match parent {
            Some(parent) => {
                let parent = self.sequence_mut(parent).ok_or(parent)?;
                parent.children.push(sequence);
            }
            None => self.sequences.push(sequence),
        }
        Ok(())
    }

    /// Remove a sequence and all of its descendants.
    pub(crate) fn remove_sequence(&mut self, id: SequenceId) -> bool {
        fn remove(sequences: &mut Vec<Sequence>, id: SequenceId) -> bool {
            if let Some(index) = sequences.iter().position(|sequence| sequence.id == id) {
                sequences.remove(index);
                return true;
            }
            sequences
                .iter_mut()
                .any(|sequence| remove(&mut sequence.children, id))
        }
        remove(&mut self.sequences, id)
    }

    /// Insert or replace a keyframe on a nested sequence.
    pub(crate) fn set_sequence_key(
        &mut self,
        sequence: SequenceId,
        node: NodeId,
        property: Property,
        key: Keyframe,
    ) -> bool {
        let Some(sequence) = self.sequence_mut(sequence) else {
            return false;
        };
        set_key(&mut sequence.tracks, node, property, key);
        true
    }

    /// Remove a keyframe from a nested sequence.
    pub(crate) fn remove_sequence_key(
        &mut self,
        sequence: SequenceId,
        node: NodeId,
        property: Property,
        time_ms: u32,
    ) -> bool {
        self.sequence_mut(sequence)
            .is_some_and(|sequence| remove_key(&mut sequence.tracks, node, property, time_ms))
    }

    /// Insert a keyframe, replacing one at the same time.
    pub(crate) fn set_key(&mut self, node: NodeId, property: Property, key: Keyframe) {
        set_key(&mut self.tracks, node, property, key);
    }

    /// Insert or replace a path morph keyframe.
    pub(crate) fn set_morph_key(&mut self, node: NodeId, key: MorphKeyframe) {
        set_morph_key(&mut self.morph_tracks, node, key);
    }

    /// Insert or replace a path morph keyframe in a nested sequence.
    pub(crate) fn set_sequence_morph_key(
        &mut self,
        sequence: SequenceId,
        node: NodeId,
        key: MorphKeyframe,
    ) -> bool {
        let Some(sequence) = self.sequence_mut(sequence) else {
            return false;
        };
        set_morph_key(&mut sequence.morph_tracks, node, key);
        true
    }

    /// Remove a path morph keyframe.
    pub(crate) fn remove_morph_key(&mut self, node: NodeId, time_ms: u32) -> bool {
        remove_morph_key(&mut self.morph_tracks, node, time_ms)
    }

    /// Remove a path morph keyframe from a nested sequence.
    pub(crate) fn remove_sequence_morph_key(
        &mut self,
        sequence: SequenceId,
        node: NodeId,
        time_ms: u32,
    ) -> bool {
        self.sequence_mut(sequence)
            .is_some_and(|sequence| remove_morph_key(&mut sequence.morph_tracks, node, time_ms))
    }

    /// Remove a keyframe. A track left without keys is removed. Returns
    /// whether the keyframe existed.
    pub(crate) fn remove_key(&mut self, node: NodeId, property: Property, time_ms: u32) -> bool {
        remove_key(&mut self.tracks, node, property, time_ms)
    }

    /// Remove every track of the given nodes.
    pub(crate) fn remove_nodes(&mut self, removed: &[NodeId]) {
        self.tracks.retain(|track| !removed.contains(&track.node));
        self.morph_tracks
            .retain(|track| !removed.contains(&track.node));
        fn remove_nodes_from(sequences: &mut [Sequence], removed: &[NodeId]) {
            for sequence in sequences {
                sequence
                    .tracks
                    .retain(|track| !removed.contains(&track.node));
                sequence
                    .morph_tracks
                    .retain(|track| !removed.contains(&track.node));
                remove_nodes_from(&mut sequence.children, removed);
            }
        }
        remove_nodes_from(&mut self.sequences, removed);
    }

    /// Check the invariants. `on_page` tells whether a node belongs to the
    /// page that owns this animation.
    pub(crate) fn validate(
        &self,
        on_page: impl Fn(NodeId) -> bool,
        path_of: impl Fn(NodeId) -> Option<PathData>,
    ) -> Result<(), String> {
        if self.duration_ms == 0 || self.duration_ms > MAX_DURATION_MS {
            return Err(format!(
                "duration must be 1 to {MAX_DURATION_MS} ms, got {}",
                self.duration_ms
            ));
        }
        validate_tracks(&self.tracks, self.duration_ms, &on_page, "page")?;
        validate_morph_tracks(&self.morph_tracks, self.duration_ms, &path_of, "page")?;
        let mut ids = std::collections::HashSet::new();
        let mut count = 0;
        validate_sequences(
            &self.sequences,
            self.duration_ms,
            1,
            &on_page,
            &path_of,
            &mut ids,
            &mut count,
        )?;
        Ok(())
    }
}

fn set_key(tracks: &mut Vec<Track>, node: NodeId, property: Property, key: Keyframe) {
    let index = tracks
        .iter()
        .position(|track| track.node == node && track.property == property)
        .unwrap_or_else(|| {
            tracks.push(Track {
                node,
                property,
                keys: Vec::new(),
            });
            tracks.len() - 1
        });
    let keys = &mut tracks[index].keys;
    match keys.binary_search_by_key(&key.time_ms, |existing| existing.time_ms) {
        Ok(found) => keys[found] = key,
        Err(insert_at) => keys.insert(insert_at, key),
    }
}

fn remove_key(tracks: &mut Vec<Track>, node: NodeId, property: Property, time_ms: u32) -> bool {
    let Some(index) = tracks
        .iter()
        .position(|track| track.node == node && track.property == property)
    else {
        return false;
    };
    let keys = &mut tracks[index].keys;
    let Ok(found) = keys.binary_search_by_key(&time_ms, |existing| existing.time_ms) else {
        return false;
    };
    keys.remove(found);
    if keys.is_empty() {
        tracks.remove(index);
    }
    true
}

fn validate_tracks(
    tracks: &[Track],
    duration_ms: u32,
    on_page: &impl Fn(NodeId) -> bool,
    owner: &str,
) -> Result<(), String> {
    for (index, track) in tracks.iter().enumerate() {
        let label = format!(
            "{owner} track of node {} {:?}",
            track.node.0, track.property
        );
        if !on_page(track.node) {
            return Err(format!("{label} targets a node that is not on the page"));
        }
        if tracks[..index]
            .iter()
            .any(|other| other.node == track.node && other.property == track.property)
        {
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
            if key.time_ms > duration_ms {
                return Err(format!("{label} has a keyframe after the end"));
            }
            if !track.property.accepts(key.value) {
                return Err(format!("{label} has an invalid value {}", key.value));
            }
        }
    }
    Ok(())
}

fn validate_sequences(
    sequences: &[Sequence],
    parent_duration_ms: u32,
    depth: usize,
    on_page: &impl Fn(NodeId) -> bool,
    path_of: &impl Fn(NodeId) -> Option<PathData>,
    ids: &mut std::collections::HashSet<SequenceId>,
    count: &mut usize,
) -> Result<(), String> {
    for sequence in sequences {
        *count += 1;
        if *count > MAX_SEQUENCES {
            return Err(format!("animation has more than {MAX_SEQUENCES} sequences"));
        }
        if depth > MAX_SEQUENCE_DEPTH {
            return Err(format!(
                "sequence nesting exceeds depth {MAX_SEQUENCE_DEPTH}"
            ));
        }
        if sequence.id.0 == 0 || !ids.insert(sequence.id) {
            return Err(format!(
                "sequence id {} is zero or used more than once",
                sequence.id.0
            ));
        }
        if sequence.name.is_empty() || sequence.name.chars().count() > 256 {
            return Err(format!("sequence {} has an invalid name", sequence.id.0));
        }
        if sequence.duration_ms == 0 || sequence.duration_ms > MAX_DURATION_MS {
            return Err(format!(
                "sequence {} has an invalid duration",
                sequence.id.0
            ));
        }
        let Some(end_ms) = sequence.offset_ms.checked_add(sequence.duration_ms) else {
            return Err(format!("sequence {} timing overflows", sequence.id.0));
        };
        if end_ms > parent_duration_ms {
            return Err(format!(
                "sequence {} extends beyond its parent",
                sequence.id.0
            ));
        }
        validate_tracks(
            &sequence.tracks,
            sequence.duration_ms,
            on_page,
            &format!("sequence {}", sequence.id.0),
        )?;
        validate_morph_tracks(
            &sequence.morph_tracks,
            sequence.duration_ms,
            path_of,
            &format!("sequence {}", sequence.id.0),
        )?;
        validate_sequences(
            &sequence.children,
            sequence.duration_ms,
            depth + 1,
            on_page,
            path_of,
            ids,
            count,
        )?;
    }
    Ok(())
}

impl Page {
    /// The page as it looks at an absolute time of its animation.
    ///
    /// The time is clamped to the duration. Nodes without tracks keep their
    /// base values, and `self` is not modified. Page tracks are applied first,
    /// then sequences in stored order; child sequences are applied after their
    /// parent. Later tracks win when they animate the same property.
    pub fn at_time(&self, time_ms: u32) -> Page {
        let mut page = self.clone();
        let time_ms = time_ms.min(self.animation.duration_ms);
        apply_tracks(&mut page, &self.animation.tracks, time_ms);
        apply_morph_tracks(&mut page, &self.animation.morph_tracks, time_ms);
        for sequence in &self.animation.sequences {
            apply_sequence(&mut page, sequence, time_ms);
        }
        page
    }
}

fn apply_tracks(page: &mut Page, tracks: &[Track], time_ms: u32) {
    for track in tracks {
        let (Some(value), Some(node)) = (track.value_at(time_ms), page.node_mut(track.node)) else {
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
}

fn apply_sequence(page: &mut Page, sequence: &Sequence, parent_time_ms: u32) {
    if parent_time_ms < sequence.offset_ms {
        return;
    }
    let local_time_ms = parent_time_ms
        .saturating_sub(sequence.offset_ms)
        .min(sequence.duration_ms);
    apply_tracks(page, &sequence.tracks, local_time_ms);
    apply_morph_tracks(page, &sequence.morph_tracks, local_time_ms);
    for child in &sequence.children {
        apply_sequence(page, child, local_time_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, Node, NodeKind, PageId, PathPoint, PathSegment, Rect};

    fn key(time_ms: u32, value: f64, easing: Easing) -> Keyframe {
        Keyframe {
            time_ms,
            value,
            easing,
        }
    }

    fn triangle(offset: f64) -> PathData {
        PathData {
            start: PathPoint {
                x: 0.5 + offset,
                y: 0.0,
            },
            segments: vec![
                PathSegment::LineTo {
                    end: PathPoint { x: 1.0, y: 1.0 },
                },
                PathSegment::LineTo {
                    end: PathPoint { x: 0.0, y: 1.0 },
                },
            ],
            closed: true,
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
    fn easing_curves_shape_interpolation_and_stay_within_their_endpoints() {
        let track = |easing| Track {
            node: NodeId(1),
            property: Property::X,
            keys: vec![key(0, 0.0, easing), key(1000, 100.0, Easing::Linear)],
        };
        assert_eq!(track(Easing::Linear).value_at(500), Some(50.0));
        assert_eq!(track(Easing::Constant).value_at(500), Some(0.0));
        assert!(track(Easing::EaseIn).value_at(500).unwrap() < 50.0);
        assert!(track(Easing::EaseOut).value_at(500).unwrap() > 50.0);
        let middle = track(Easing::EaseInOut).value_at(500).unwrap();
        assert!((45.0..55.0).contains(&middle));
        assert_eq!(Easing::Constant.progress(1.0), 1.0);
        assert_eq!(Easing::EaseIn.progress(-1.0), 0.0);
        assert_eq!(Easing::EaseOut.progress(2.0), 1.0);
    }

    #[test]
    fn nested_sequences_use_local_time_and_hold_their_final_values() {
        let mut page = page();
        page.animation.sequences.push(Sequence {
            id: SequenceId(1),
            name: "Parent".into(),
            offset_ms: 200,
            duration_ms: 600,
            tracks: vec![Track {
                node: NodeId(1),
                property: Property::X,
                keys: vec![key(0, 0.0, Easing::Linear), key(400, 100.0, Easing::Linear)],
            }],
            morph_tracks: Vec::new(),
            children: vec![Sequence {
                id: SequenceId(2),
                name: "Child".into(),
                offset_ms: 100,
                duration_ms: 200,
                tracks: vec![Track {
                    node: NodeId(1),
                    property: Property::Y,
                    keys: vec![key(0, 20.0, Easing::Linear), key(200, 60.0, Easing::Linear)],
                }],
                morph_tracks: Vec::new(),
                children: Vec::new(),
            }],
        });
        assert_eq!(
            page.animation.validate(|id| id == NodeId(1), |_| None),
            Ok(())
        );
        assert_eq!(page.at_time(0).nodes[0].bounds.x, 10.0);
        assert_eq!(page.at_time(200).nodes[0].bounds.x, 0.0);
        assert_eq!(page.at_time(300).nodes[0].bounds.x, 25.0);
        assert_eq!(page.at_time(400).nodes[0].bounds.x, 50.0);
        assert_eq!(page.at_time(400).nodes[0].bounds.y, 40.0);
        assert_eq!(page.at_time(500).nodes[0].bounds.x, 75.0);
        assert_eq!(page.at_time(500).nodes[0].bounds.y, 60.0);
        assert_eq!(page.at_time(900).nodes[0].bounds.x, 100.0);
        assert_eq!(page.at_time(900).nodes[0].bounds.y, 60.0);
    }

    #[test]
    fn morph_tracks_interpolate_matching_paths_and_hold_the_last_key() {
        let mut page = page();
        page.nodes[0].kind = NodeKind::Path {
            path: triangle(0.0),
        };
        page.animation.set_morph_key(
            NodeId(1),
            MorphKeyframe {
                time_ms: 0,
                path: triangle(0.0),
                easing: Easing::Linear,
            },
        );
        page.animation.set_morph_key(
            NodeId(1),
            MorphKeyframe {
                time_ms: 1000,
                path: triangle(0.2),
                easing: Easing::Linear,
            },
        );

        assert_eq!(
            page.animation.validate(
                |id| id == NodeId(1),
                |id| match &page.node(id)?.kind {
                    NodeKind::Path { path } => Some(path.clone()),
                    _ => None,
                },
            ),
            Ok(())
        );
        let middle = page.at_time(500);
        let NodeKind::Path { path } = &middle.nodes[0].kind else {
            panic!("morph target remains a path");
        };
        assert!((path.start.x - 0.6).abs() < 1e-12);
        assert_eq!(page.at_time(1500), page.at_time(1000));

        let mut incompatible = page.animation.clone();
        incompatible.morph_tracks[0].keys[1].path.segments.pop();
        assert!(
            incompatible
                .validate(
                    |id| id == NodeId(1),
                    |id| match &page.node(id)?.kind {
                        NodeKind::Path { path } => Some(path.clone()),
                        _ => None,
                    },
                )
                .is_err()
        );
        let mut incompatible_segment = page.animation.clone();
        incompatible_segment.morph_tracks[0].keys[1].path.segments[0] = PathSegment::CubicTo {
            control1: PathPoint { x: 0.4, y: 0.2 },
            control2: PathPoint { x: 0.6, y: 0.2 },
            end: PathPoint { x: 1.0, y: 1.0 },
        };
        assert!(
            incompatible_segment
                .validate(
                    |id| id == NodeId(1),
                    |id| match &page.node(id)?.kind {
                        NodeKind::Path { path } => Some(path.clone()),
                        _ => None,
                    },
                )
                .is_err()
        );
    }

    #[test]
    fn older_animation_json_defaults_to_no_nested_sequences() {
        let animation: Animation =
            serde_json::from_str(r#"{"duration_ms":1000,"tracks":[]}"#).expect("old format");
        assert_eq!(animation.duration_ms, 1000);
        assert!(animation.sequences.is_empty());
        let nested: Animation = serde_json::from_str(
            r#"{"duration_ms":1000,"tracks":[],"sequences":[{"id":1,"name":"old","offset_ms":0,"duration_ms":1000,"tracks":[],"children":[]}]}"#,
        )
        .expect("old nested sequence format");
        assert!(nested.sequences[0].morph_tracks.is_empty());
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
        assert_eq!(animation.validate(|_| true, |_| None), Ok(()));

        assert!(!animation.remove_key(NodeId(1), Property::Y, 1));
        for time in [0, 250, 500] {
            assert!(animation.remove_key(NodeId(1), Property::Y, time));
        }
        assert!(animation.tracks.is_empty());
    }

    #[test]
    fn validation_rejects_inconsistent_tracks() {
        let valid = |animation: &Animation| animation.validate(|id| id == NodeId(1), |_| None);
        let with = |keys: Vec<Keyframe>, property| Animation {
            duration_ms: 1000,
            tracks: vec![Track {
                node: NodeId(1),
                property,
                keys,
            }],
            morph_tracks: Vec::new(),
            sequences: Vec::new(),
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

        let endless = Animation {
            duration_ms: 0,
            ..Animation::default()
        };
        assert!(valid(&endless).is_err());
    }
}
