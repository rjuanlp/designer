//! User preferences, stored as versioned JSON in the data directory.
//!
//! The file is small and rewritten whole through an atomic rename. A missing
//! file means defaults. A file that cannot be read, is not valid JSON or comes
//! from a newer version also yields defaults, together with the reason, so
//! the application can tell the user instead of failing to start.

use std::{
    fmt, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::dsg::write_atomic;

/// File name inside the data directory.
pub const FILE_NAME: &str = "preferences.json";
/// Schema version written by this build.
pub const VERSION: u32 = 1;
/// Most recent documents remembered.
pub const MAX_RECENT: usize = 10;

/// Interface theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Dark theme, the default.
    #[default]
    Dark,
    /// Light theme.
    Light,
}

/// Size of the window when it is not maximized, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowSize {
    /// Width in logical pixels.
    pub width: f32,
    /// Height in logical pixels.
    pub height: f32,
}

/// Everything Designer remembers between runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    /// Schema version, [`VERSION`] when written by this build.
    pub version: u32,
    /// Interface theme.
    #[serde(default)]
    pub theme: Theme,
    /// Whether the timeline starts expanded.
    #[serde(default)]
    pub timeline_open: bool,
    /// Last window size, if known.
    #[serde(default)]
    pub window_size: Option<WindowSize>,
    /// Recently opened or saved documents, most recent first.
    #[serde(default)]
    pub recent: Vec<PathBuf>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: VERSION,
            theme: Theme::Dark,
            timeline_open: false,
            window_size: None,
            recent: Vec::new(),
        }
    }
}

/// Why stored preferences were not used.
#[derive(Debug)]
pub enum PreferencesError {
    /// The file exists but could not be read.
    Io(io::Error),
    /// The file is not valid preferences JSON.
    Invalid(serde_json::Error),
    /// The file was written by a newer version of Designer.
    NewerVersion(u32),
}

impl fmt::Display for PreferencesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Invalid(error) => write!(f, "invalid preferences: {error}"),
            Self::NewerVersion(version) => {
                write!(f, "preferences version {version} is newer than {VERSION}")
            }
        }
    }
}

impl std::error::Error for PreferencesError {}

/// Preferences read at start, with the reason defaults were used instead.
#[derive(Debug, Default)]
pub struct Loaded {
    /// The preferences to use.
    pub preferences: Preferences,
    /// Why the stored file was ignored, if it was.
    pub problem: Option<PreferencesError>,
}

impl Preferences {
    /// Path of the preferences file inside `data_dir`.
    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join(FILE_NAME)
    }

    /// Read the preferences stored in `data_dir`. Never fails: problems are
    /// reported next to default values.
    pub fn load(data_dir: &Path) -> Loaded {
        let bytes = match std::fs::read(Self::path(data_dir)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Loaded::default(),
            Err(error) => {
                return Loaded {
                    preferences: Self::default(),
                    problem: Some(PreferencesError::Io(error)),
                };
            }
        };
        match Self::from_json(&bytes) {
            Ok(preferences) => Loaded {
                preferences,
                problem: None,
            },
            Err(problem) => Loaded {
                preferences: Self::default(),
                problem: Some(problem),
            },
        }
    }

    /// Parse and clean stored preferences.
    pub fn from_json(bytes: &[u8]) -> Result<Self, PreferencesError> {
        let mut preferences: Self =
            serde_json::from_slice(bytes).map_err(PreferencesError::Invalid)?;
        if preferences.version > VERSION {
            return Err(PreferencesError::NewerVersion(preferences.version));
        }
        preferences.version = VERSION;
        preferences.window_size = preferences.window_size.filter(|size| {
            size.width.is_finite()
                && size.height.is_finite()
                && size.width > 0.0
                && size.height > 0.0
        });
        let recent = std::mem::take(&mut preferences.recent);
        for path in recent.into_iter().rev() {
            preferences.remember(&path);
        }
        Ok(preferences)
    }

    /// Write the preferences to `data_dir`, creating it if needed. The
    /// previous file is replaced atomically.
    pub fn save(&self, data_dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(data_dir)?;
        let mut bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        bytes.push(b'\n');
        write_atomic(&Self::path(data_dir), &bytes)
    }

    /// Put `path` first in the recent documents, removing older mentions of
    /// the same file and keeping at most [`MAX_RECENT`] entries.
    pub fn remember(&mut self, path: &Path) {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
        self.forget(&path);
        self.recent.insert(0, path);
        self.recent.truncate(MAX_RECENT);
    }

    /// Remove `path` from the recent documents.
    pub fn forget(&mut self, path: &Path) {
        self.recent.retain(|entry| !same_file(entry, path));
    }
}

/// Whether two paths name the same file, ignoring letter case on Windows.
fn same_file(a: &Path, b: &Path) -> bool {
    let absolute = |path: &Path| std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    let (a, b) = (absolute(a), absolute(b));
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "designer-preferences-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_missing_file_gives_defaults_without_a_problem() {
        let dir = scratch("missing");
        let loaded = Preferences::load(&dir);
        assert_eq!(loaded.preferences, Preferences::default());
        assert!(loaded.problem.is_none());
    }

    #[test]
    fn preferences_round_trip_through_the_data_directory() {
        let dir = scratch("round-trip");
        let mut preferences = Preferences {
            theme: Theme::Light,
            timeline_open: true,
            window_size: Some(WindowSize {
                width: 1280.0,
                height: 800.0,
            }),
            ..Preferences::default()
        };
        preferences.remember(Path::new("a.dsg"));
        preferences.save(&dir).expect("saved");
        let loaded = Preferences::load(&dir);
        assert!(loaded.problem.is_none());
        assert_eq!(loaded.preferences, preferences);
        // No temporary file is left behind.
        let names: Vec<_> = std::fs::read_dir(&dir)
            .expect("listed")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        assert_eq!(names, [FILE_NAME]);
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn corrupt_or_newer_files_fall_back_to_defaults_and_say_why() {
        let dir = scratch("corrupt");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(Preferences::path(&dir), b"{ not json").expect("written");
        let loaded = Preferences::load(&dir);
        assert_eq!(loaded.preferences, Preferences::default());
        assert!(matches!(loaded.problem, Some(PreferencesError::Invalid(_))));

        std::fs::write(Preferences::path(&dir), br#"{"version": 99}"#).expect("written");
        let loaded = Preferences::load(&dir);
        assert_eq!(loaded.preferences, Preferences::default());
        assert!(matches!(
            loaded.problem,
            Some(PreferencesError::NewerVersion(99))
        ));
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn missing_fields_take_defaults_and_bad_sizes_are_dropped() {
        let preferences = Preferences::from_json(
            br#"{"version": 1, "window_size": {"width": -5, "height": 900}}"#,
        )
        .expect("valid");
        assert_eq!(preferences, Preferences::default());
    }

    #[test]
    fn recent_documents_are_unique_newest_first_and_bounded() {
        let mut preferences = Preferences::default();
        for index in 0..15 {
            preferences.remember(Path::new(&format!("doc{index}.dsg")));
        }
        preferences.remember(Path::new("doc12.dsg"));
        assert_eq!(preferences.recent.len(), MAX_RECENT);
        assert!(preferences.recent[0].ends_with("doc12.dsg"));
        assert!(preferences.recent[1].ends_with("doc14.dsg"));
        assert_eq!(
            preferences
                .recent
                .iter()
                .filter(|path| path.ends_with("doc12.dsg"))
                .count(),
            1
        );
        if cfg!(windows) {
            preferences.remember(Path::new("DOC14.DSG"));
            assert_eq!(preferences.recent.len(), MAX_RECENT);
        }
        preferences.forget(Path::new("doc12.dsg"));
        assert!(!preferences.recent.iter().any(|p| p.ends_with("doc12.dsg")));
    }

    #[test]
    fn stored_duplicates_are_collapsed_when_loading() {
        let preferences =
            Preferences::from_json(br#"{"version": 1, "recent": ["x.dsg", "y.dsg", "x.dsg"]}"#)
                .expect("valid");
        assert_eq!(preferences.recent.len(), 2);
        assert!(preferences.recent[0].ends_with("x.dsg"));
        assert!(preferences.recent[1].ends_with("y.dsg"));
    }
}
