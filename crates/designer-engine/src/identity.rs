//! Central application identity for forks and local storage.

use std::path::PathBuf;

/// Application display name.
pub const DISPLAY_NAME: &str = "Designer";
/// Per-user storage directory name.
pub const DATA_DIRECTORY: &str = "Designer";
/// Named pipe prefix, including the Windows pipe namespace.
pub const PIPE_PREFIX: &str = r"\\.\pipe\designer-session";
/// Recovery subdirectory name.
pub const RECOVERY_DIRECTORY: &str = "recovery";

/// Live session pipe name.
pub fn session_pipe() -> String {
    format!("{PIPE_PREFIX}-v1")
}

/// Per-user storage, overridden by `DESIGNER_DATA_DIR`.
pub fn data_dir() -> PathBuf {
    resolve_data_dir(
        std::env::var_os("DESIGNER_DATA_DIR"),
        std::env::var_os("APPDATA"),
    )
}

fn resolve_data_dir(
    override_dir: Option<std::ffi::OsString>,
    appdata: Option<std::ffi::OsString>,
) -> PathBuf {
    override_dir
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            appdata
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(DATA_DIRECTORY)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_resolution_does_not_require_mutating_process_environment() {
        assert_eq!(
            resolve_data_dir(Some("custom".into()), Some("user".into())),
            PathBuf::from("custom")
        );
        assert_eq!(
            resolve_data_dir(None, Some("user".into())),
            PathBuf::from("user").join(DATA_DIRECTORY)
        );
        assert_eq!(
            resolve_data_dir(None, None),
            std::env::temp_dir().join(DATA_DIRECTORY)
        );
    }
}
