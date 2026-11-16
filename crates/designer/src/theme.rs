//! Interface design tokens. Components must take every color and dimension
//! from here; values authored by users are document data, not tokens.

/// Semantic colors of one theme, as `0xRRGGBB`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Window background behind the panels.
    pub app: u32,
    /// Panel and bar background.
    pub panel: u32,
    /// Lines between panels and around controls.
    pub border: u32,
    /// Primary text.
    pub text: u32,
    /// Secondary text and inactive labels.
    pub text_muted: u32,
    /// Background of a hovered control.
    pub hover: u32,
    /// Background of the selected row or active control.
    pub selected: u32,
    /// Text on a selected background.
    pub selected_text: u32,
    /// Keyboard focus ring.
    pub focus_ring: u32,
}

/// Dark theme.
pub const DARK: Palette = Palette {
    app: 0x141519,
    panel: 0x1e1f24,
    border: 0x34363d,
    text: 0xf2f4f8,
    text_muted: 0xa4aab6,
    hover: 0x2a2c33,
    selected: 0x4a3fc4,
    selected_text: 0xffffff,
    focus_ring: 0x7fb4ff,
};

/// Light theme.
pub const LIGHT: Palette = Palette {
    app: 0xe6e8ee,
    panel: 0xf8f9fb,
    border: 0xcdd1da,
    text: 0x18191d,
    text_muted: 0x565c69,
    hover: 0xe9ebf1,
    selected: 0x5646d6,
    selected_text: 0xffffff,
    focus_ring: 0x0b5fd1,
};

/// Dimensions in logical pixels.
pub mod metrics {
    /// Initial window size.
    pub const WINDOW_WIDTH: f32 = 1440.0;
    /// Initial window height.
    pub const WINDOW_HEIGHT: f32 = 900.0;
    /// Smallest supported window width.
    pub const MIN_WINDOW_WIDTH: f32 = 390.0;
    /// Smallest supported window height.
    pub const MIN_WINDOW_HEIGHT: f32 = 560.0;
    /// Below this width the side panels and canvas are shown one at a time.
    pub const NARROW_BREAKPOINT: f32 = 760.0;

    /// Height of the navigation bar and the tool bar.
    pub const BAR_HEIGHT: f32 = 40.0;
    /// Width of the layers panel.
    pub const LAYERS_WIDTH: f32 = 248.0;
    /// Width of the inspector panel.
    pub const INSPECTOR_WIDTH: f32 = 264.0;
    /// Height of the expanded timeline, including its header.
    pub const TIMELINE_HEIGHT: f32 = 200.0;
    /// Height of a list row and of a control.
    pub const ROW_HEIGHT: f32 = 28.0;
    /// Indentation per tree level.
    pub const INDENT: f32 = 14.0;
    /// Side of a color swatch.
    pub const SWATCH: f32 = 16.0;
    /// Width of a property label column.
    pub const LABEL_WIDTH: f32 = 84.0;

    /// Spacing scale.
    pub const SPACE_XS: f32 = 4.0;
    /// Spacing scale.
    pub const SPACE_S: f32 = 8.0;
    /// Spacing scale.
    pub const SPACE_M: f32 = 12.0;
    /// Spacing scale.
    pub const SPACE_L: f32 = 24.0;

    /// Corner radius of controls.
    pub const RADIUS: f32 = 6.0;

    /// Type scale.
    pub const TEXT_S: f32 = 12.0;
    /// Type scale.
    pub const TEXT_M: f32 = 13.0;
    /// Type scale.
    pub const TEXT_L: f32 = 18.0;

    /// Free space kept around the page content when it is fitted to the
    /// canvas, in physical pixels.
    pub const CANVAS_MARGIN: f64 = 48.0;
}

/// How the workspace arranges its panels for a window width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    /// Layers, canvas and inspector side by side.
    Wide,
    /// One of them at a time, chosen with a switcher.
    Narrow,
}

/// Choose the workspace layout for a window width in logical pixels.
pub fn layout_mode(window_width: f32) -> LayoutMode {
    if window_width < metrics::NARROW_BREAKPOINT {
        LayoutMode::Narrow
    } else {
        LayoutMode::Wide
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG 2 relative luminance of an sRGB color.
    fn luminance(color: u32) -> f64 {
        let channel = |shift: u32| {
            let value = f64::from((color >> shift) & 0xff) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }

    fn contrast(a: u32, b: u32) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn text_meets_wcag_aa_on_every_surface_in_both_themes() {
        for (name, palette) in [("dark", DARK), ("light", LIGHT)] {
            for (label, foreground, background) in [
                ("text on panel", palette.text, palette.panel),
                ("text on app", palette.text, palette.app),
                ("text on hover", palette.text, palette.hover),
                ("muted on panel", palette.text_muted, palette.panel),
                ("muted on app", palette.text_muted, palette.app),
                ("selected", palette.selected_text, palette.selected),
            ] {
                let ratio = contrast(foreground, background);
                assert!(ratio >= 4.5, "{name}: {label} contrast is {ratio:.2}");
            }
            // Non-text indicators need 3:1 against what surrounds them.
            for background in [palette.panel, palette.app] {
                let ratio = contrast(palette.focus_ring, background);
                assert!(ratio >= 3.0, "{name}: focus ring contrast is {ratio:.2}");
            }
        }
    }

    #[test]
    fn narrow_layout_applies_at_phone_width_only() {
        assert_eq!(layout_mode(metrics::MIN_WINDOW_WIDTH), LayoutMode::Narrow);
        assert_eq!(layout_mode(metrics::NARROW_BREAKPOINT), LayoutMode::Wide);
        assert_eq!(layout_mode(metrics::WINDOW_WIDTH), LayoutMode::Wide);
    }

    #[test]
    fn side_panels_fit_beside_a_usable_canvas_in_wide_layout() {
        let canvas = metrics::NARROW_BREAKPOINT - metrics::LAYERS_WIDTH - metrics::INSPECTOR_WIDTH;
        assert!(canvas >= 240.0, "canvas would be {canvas} wide");
    }
}
