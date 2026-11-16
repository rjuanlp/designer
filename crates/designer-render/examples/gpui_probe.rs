//! GPUI window smoke test. Vello composition is deliberately not claimed.
#[cfg(windows)]
mod windows_probe {
    use gpui::{
        App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*,
        px, rgb, size,
    };

    mod tokens {
        pub const WIDTH: f32 = 960.0;
        pub const HEIGHT: f32 = 640.0;
        pub const BACKGROUND: u32 = 0x18191d;
        pub const FOREGROUND: u32 = 0xf2f4f8;
    }

    struct Probe;
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .flex_col()
                .justify_center()
                .items_center()
                .bg(rgb(tokens::BACKGROUND))
                .text_color(rgb(tokens::FOREGROUND))
                .child("Designer · Prueba de ventana nativa")
                .child("La composición de Vello todavía no está conectada.")
        }
    }

    pub fn run() {
        Application::new().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(tokens::WIDTH), px(tokens::HEIGHT)), cx);
            if let Err(error) = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("Designer · GPUI probe".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Probe),
            ) {
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

fn main() {
    #[cfg(windows)]
    windows_probe::run();
    #[cfg(not(windows))]
    eprintln!("The GPUI probe currently targets Windows only.");
}
