//! Designer application shell.
//!
//! Navigation, panels, themes and a Vello canvas on which frames, rectangles
//! and ellipses can be drawn, moved and deleted, with undo/redo and `.dsg`
//! files. The timeline supports basic keyframe editing and playback. Property
//! editing uses single-line GPUI input fields. Windows MCP clients can attach to the
//! live session over a local named pipe or run a separate locked headless session.
//! AccessKit exposes navigation and screen controls; manual screen-reader
//! validation remains pending. Per-control bounds are not supplied.
//!
//! Launch options, mainly for capturing interface states:
//! `--size 1440x900`, `--screen documents|design|components|preview|import|preferences`,
//! `--page N` (zero-based), `--select NODE_ID`, `--panel layers|canvas|inspector`,
//! `--timeline`, `--light`, `--open FILE.dsg`.

mod icons;
mod inspector;
mod prototype_preview;
mod strings;
pub mod text_field;
mod theme;
#[cfg(any(windows, test))]
mod timeline;

#[cfg(windows)]
mod accessibility;
#[cfg(windows)]
mod bridge;
#[cfg(windows)]
mod canvas;
#[cfg(windows)]
mod shell;

use strings::{NarrowPanel, Screen};
use theme::metrics;

/// Initial interface state requested on the command line.
#[derive(Clone, Debug, PartialEq)]
pub struct LaunchOptions {
    /// Initial window size in logical pixels.
    pub size: (f32, f32),
    /// Section shown first.
    pub screen: Screen,
    /// Zero-based index of the page shown first.
    pub page: usize,
    /// Node selected at start, if it exists on that page.
    pub select: Option<u64>,
    /// Panel shown first in the narrow layout.
    pub panel: NarrowPanel,
    /// Start with the timeline expanded.
    pub timeline: bool,
    /// Start with the light theme.
    pub light: bool,
    /// Document file opened at start instead of the sample document.
    pub open: Option<std::path::PathBuf>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            size: (metrics::WINDOW_WIDTH, metrics::WINDOW_HEIGHT),
            screen: Screen::Design,
            page: 0,
            select: None,
            panel: NarrowPanel::Canvas,
            timeline: false,
            light: false,
            open: None,
        }
    }
}

/// Parse launch options. Every problem is reported instead of ignored.
pub fn parse_options(arguments: impl IntoIterator<Item = String>) -> Result<LaunchOptions, String> {
    let mut options = LaunchOptions::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let mut value = |name: &str| {
            arguments
                .next()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match argument.as_str() {
            "--timeline" => options.timeline = true,
            "--light" => options.light = true,
            "--open" => options.open = Some(value("--open")?.into()),
            "--size" => {
                let text = value("--size")?;
                let parsed = text.split_once('x').and_then(|(width, height)| {
                    Some((width.parse::<f32>().ok()?, height.parse::<f32>().ok()?))
                });
                options.size = match parsed {
                    Some((width, height))
                        if width >= metrics::MIN_WINDOW_WIDTH
                            && height >= metrics::MIN_WINDOW_HEIGHT
                            && width.is_finite()
                            && height.is_finite() =>
                    {
                        (width, height)
                    }
                    _ => {
                        return Err(format!(
                            "--size needs WIDTHxHEIGHT of at least {}x{}, got {text}",
                            metrics::MIN_WINDOW_WIDTH,
                            metrics::MIN_WINDOW_HEIGHT
                        ));
                    }
                };
            }
            "--screen" => {
                let text = value("--screen")?;
                options.screen = match text.as_str() {
                    "documents" => Screen::Documents,
                    "design" => Screen::Design,
                    "components" => Screen::Components,
                    "preview" => Screen::Preview,
                    "import" => Screen::Import,
                    "preferences" => Screen::Preferences,
                    _ => return Err(format!("unknown screen {text}")),
                };
            }
            "--panel" => {
                let text = value("--panel")?;
                options.panel = match text.as_str() {
                    "layers" => NarrowPanel::Layers,
                    "canvas" => NarrowPanel::Canvas,
                    "inspector" => NarrowPanel::Inspector,
                    _ => return Err(format!("unknown panel {text}")),
                };
            }
            "--page" => {
                let text = value("--page")?;
                options.page = text
                    .parse()
                    .map_err(|_| format!("--page needs a zero-based index, got {text}"))?;
            }
            "--select" => {
                let text = value("--select")?;
                options.select = Some(
                    text.parse()
                        .map_err(|_| format!("--select needs a node id, got {text}"))?,
                );
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(options)
}

#[cfg(windows)]
mod app {
    use gpui::{
        App, AppContext as _, Application, Bounds, KeyBinding, TitlebarOptions, WindowBounds,
        WindowOptions, px, size,
    };

    use crate::{
        LaunchOptions,
        shell::{
            self, Cancel, DeleteSelection, FocusNext, FocusPrevious, NewDocument, Open, Redo, Save,
            SaveAs, Shell, ToggleTheme, ToggleTimeline, Undo, ZoomFit, ZoomIn, ZoomOut, ZoomReset,
        },
        strings,
        theme::metrics,
    };

    /// Prints warnings and errors, including those GPUI reports through the
    /// `log` facade, which would otherwise be silent.
    struct StderrLogger;

    impl log::Log for StderrLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Warn
        }

        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                eprintln!("[{}] {}", record.level(), record.args());
            }
        }

        fn flush(&self) {}
    }

    pub fn run(options: LaunchOptions) {
        if log::set_logger(&StderrLogger).is_ok() {
            log::set_max_level(log::LevelFilter::Warn);
        }
        Application::new()
            .with_assets(crate::icons::Assets)
            .run(move |cx: &mut App| {
                let context = Some(shell::KEY_CONTEXT);
                cx.bind_keys([
                    KeyBinding::new("ctrl-t", ToggleTimeline, context),
                    KeyBinding::new("ctrl-shift-l", ToggleTheme, context),
                    KeyBinding::new("tab", FocusNext, context),
                    KeyBinding::new("shift-tab", FocusPrevious, context),
                    KeyBinding::new("ctrl-z", Undo, context),
                    KeyBinding::new("ctrl-shift-z", Redo, context),
                    KeyBinding::new("ctrl-y", Redo, context),
                    KeyBinding::new("ctrl-s", Save, context),
                    KeyBinding::new("ctrl-shift-s", SaveAs, context),
                    KeyBinding::new("ctrl-o", Open, context),
                    KeyBinding::new("ctrl-n", NewDocument, context),
                    KeyBinding::new("delete", DeleteSelection, context),
                    KeyBinding::new("backspace", DeleteSelection, context),
                    KeyBinding::new("escape", Cancel, context),
                    KeyBinding::new("ctrl-=", ZoomIn, context),
                    KeyBinding::new("ctrl-+", ZoomIn, context),
                    KeyBinding::new("ctrl--", ZoomOut, context),
                    KeyBinding::new("ctrl-0", ZoomFit, context),
                    KeyBinding::new("ctrl-1", ZoomReset, context),
                ]);
                cx.bind_keys(
                    ["delete", "backspace", "escape"]
                        .map(|key| KeyBinding::new(key, gpui::NoAction, Some("TextField"))),
                );
                let (width, height) = options.size;
                let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
                let opened = cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(size(
                            px(metrics::MIN_WINDOW_WIDTH),
                            px(metrics::MIN_WINDOW_HEIGHT),
                        )),
                        titlebar: Some(TitlebarOptions {
                            title: Some(strings::APP_NAME.into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    |window, cx| {
                        let shell = cx.new(|cx| Shell::new(&options, window, cx));
                        window.focus(&shell.read(cx).focus_handle());
                        let closing = shell.clone();
                        window.on_window_should_close(cx, move |window, cx| {
                            closing.update(cx, |shell, cx| shell.should_close(window, cx))
                        });
                        shell
                    },
                );
                if let Err(error) = opened {
                    eprintln!("Window creation failed: {error}");
                    cx.quit();
                    return;
                }
                cx.activate(true);
                cx.on_window_closed(|cx| {
                    if cx.windows().is_empty() {
                        cx.quit();
                    }
                })
                .detach();
            });
    }
}

fn main() -> std::process::ExitCode {
    let options = match parse_options(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return std::process::ExitCode::FAILURE;
        }
    };
    #[cfg(windows)]
    app::run(options);
    #[cfg(not(windows))]
    {
        let _ = options;
        eprintln!("Designer currently runs on Windows only.");
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Result<LaunchOptions, String> {
        parse_options(arguments.iter().map(|argument| argument.to_string()))
    }

    #[test]
    fn defaults_open_the_design_workspace() {
        assert_eq!(parse(&[]), Ok(LaunchOptions::default()));
    }

    #[test]
    fn reads_every_option() {
        let options = parse(&[
            "--size",
            "390x844",
            "--screen",
            "preferences",
            "--page",
            "1",
            "--select",
            "7",
            "--panel",
            "inspector",
            "--timeline",
            "--light",
            "--open",
            "a b.dsg",
        ])
        .expect("valid options");
        assert_eq!(options.size, (390.0, 844.0));
        assert_eq!(options.screen, Screen::Preferences);
        assert_eq!(options.page, 1);
        assert_eq!(options.select, Some(7));
        assert_eq!(options.panel, NarrowPanel::Inspector);
        assert!(options.timeline && options.light);
        assert_eq!(options.open, Some("a b.dsg".into()));
    }

    #[test]
    fn rejects_bad_input_instead_of_guessing() {
        for arguments in [
            &["--size"][..],
            &["--size", "100x100"],
            &["--size", "wide"],
            &["--size", "NaNx900"],
            &["--screen", "home"],
            &["--panel", "left"],
            &["--page", "-1"],
            &["--select", "abc"],
            &["--open"],
            &["--fast"],
        ] {
            assert!(parse(arguments).is_err(), "{arguments:?} was accepted");
        }
    }
}
