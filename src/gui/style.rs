//! Visual style: sizes and a colour palette. Widgets read everything from here, so an
//! application re-themes by replacing the [`Style`] on its context.
//!
//! Colours are **linear** RGBA (the primitive pipeline writes them straight into an sRGB
//! surface, and the text pass converts them back to sRGB for glyphon). Use [`srgb`] to
//! build one from a CSS-style `#rrggbb` value.

/// RGBA colour, linear 0..1.
pub type Color = [f32; 4];

/// One sRGB channel (0..1) to linear.
#[must_use]
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// One linear channel (0..1) to sRGB.
#[must_use]
pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// A colour from a CSS-style `0xRRGGBB` value with `alpha` (0..1).
#[must_use]
pub fn srgb(hex: u32, alpha: f32) -> Color {
    let ch = |shift: u32| srgb_to_linear(((hex >> shift) & 0xff) as f32 / 255.0);
    [ch(16), ch(8), ch(0), alpha]
}

/// Colour palette.
///
/// Three hues by budget: `accent` (orange) for selection / focus / ON, `danger` (red) for
/// stop / destructive, `ok` (green) for healthy; everything else is a neutral grey ladder
/// of five surfaces plus two line values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Window ground (also disabled controls, tab strips).
    pub background: Color,
    /// Panel surface.
    pub panel: Color,
    /// Header bars: title bar, panel headings, status bar.
    pub header: Color,
    /// Raised surface (buttons).
    pub raised: Color,
    /// Raised surface, hovered.
    pub raised_hover: Color,
    /// Raised surface, pressed.
    pub raised_active: Color,
    /// Sunken surface (inputs, consoles, joysticks).
    pub sunken: Color,
    /// Alternate surface — the same as `sunken`; kept for callers that predate the
    /// five-step ladder.
    pub panel_alt: Color,
    /// Row hover.
    pub row_hover: Color,
    /// Popups, menus, modals, tooltips.
    pub popup: Color,
    /// Outer / structural lines (window chrome, dock gaps, input borders).
    pub line_hard: Color,
    /// Separators and button borders.
    pub line: Color,
    /// Table rules.
    pub line_grid: Color,
    /// Primary text.
    pub text: Color,
    /// Secondary text (labels).
    pub text_dim: Color,
    /// Tertiary text (hints, headings, inactive tabs).
    pub text_faint: Color,
    /// Disabled text.
    pub text_disabled: Color,
    /// Text on accent.
    pub text_on_accent: Color,
    /// Console / log body text.
    pub console_text: Color,
    /// Hover overlay (composited over a surface).
    pub hover: Color,
    /// Pressed overlay.
    pub active: Color,
    /// Accent (selection, focus ring, ON state).
    pub accent: Color,
    /// Accent-tinted surface (selected rows, ON toggles).
    pub accent_bg: Color,
    /// Accent-tinted surface, hovered.
    pub accent_bg_hover: Color,
    /// Danger.
    pub danger: Color,
    /// Danger-tinted surface.
    pub danger_bg: Color,
    /// Healthy / ok.
    pub ok: Color,
    /// Modal dimming.
    pub dim: Color,
    /// 3D viewport ground.
    pub viewport_bg: Color,
    /// 3D viewport grid.
    pub viewport_grid: Color,
    /// Overlay text on the viewport.
    pub viewport_label: Color,
}

impl Palette {
    /// Dark palette: neutral greys, orange accent, red danger, green ok.
    pub const DARK: Self = Self {
        background: [0.0144, 0.0160, 0.0176, 1.0],
        panel: [0.0212, 0.0232, 0.0262, 1.0],
        header: [0.0262, 0.0284, 0.0331, 1.0],
        raised: [0.0343, 0.0382, 0.0437, 1.0],
        raised_hover: [0.0467, 0.0513, 0.0595, 1.0],
        raised_active: [0.0242, 0.0262, 0.0307, 1.0],
        sunken: [0.0116, 0.0130, 0.0144, 1.0],
        panel_alt: [0.0116, 0.0130, 0.0144, 1.0],
        row_hover: [0.0307, 0.0331, 0.0382, 1.0],
        popup: [0.0212, 0.0232, 0.0262, 1.0],
        line_hard: [0.0086, 0.0091, 0.0103, 1.0],
        line: [0.0423, 0.0467, 0.0545, 1.0],
        line_grid: [0.0331, 0.0369, 0.0423, 1.0],
        text: [0.8070, 0.8148, 0.8228, 1.0],
        text_dim: [0.4564, 0.4793, 0.5029, 1.0],
        text_faint: [0.2159, 0.2346, 0.2582, 1.0],
        text_disabled: [0.1070, 0.1195, 0.1356, 1.0],
        text_on_accent: [0.0103, 0.0060, 0.0018, 1.0],
        console_text: [0.4020, 0.4508, 0.4125, 1.0],
        hover: [1.0, 1.0, 1.0, 0.05],
        active: [0.0, 0.0, 0.0, 0.25],
        accent: [0.8070, 0.2542, 0.0103, 1.0],
        accent_bg: [0.8070, 0.2542, 0.0103, 0.14],
        accent_bg_hover: [0.8070, 0.2542, 0.0103, 0.22],
        danger: [0.6308, 0.0844, 0.0723, 1.0],
        danger_bg: [0.6308, 0.0844, 0.0723, 0.12],
        ok: [0.1022, 0.3813, 0.1441, 1.0],
        dim: [0.0, 0.0, 0.0, 0.45],
        viewport_bg: [0.0103, 0.0123, 0.0152, 1.0],
        viewport_grid: [0.0168, 0.0203, 0.0252, 1.0],
        viewport_label: [0.3419, 0.3813, 0.4233, 1.0],
    };

    /// Light palette: the same ladder on a light ground, accent one step darker.
    pub const LIGHT: Self = Self {
        background: [0.7157, 0.7157, 0.7011, 1.0],
        panel: [0.8388, 0.8388, 0.8308, 1.0],
        header: [0.7758, 0.7758, 0.7605, 1.0],
        raised: [0.9301, 0.9301, 0.9216, 1.0],
        raised_hover: [1.0, 1.0, 1.0, 1.0],
        raised_active: [0.7913, 0.7913, 0.7758, 1.0],
        sunken: [1.0, 1.0, 1.0, 1.0],
        panel_alt: [1.0, 1.0, 1.0, 1.0],
        row_hover: [0.7605, 0.7605, 0.7454, 1.0],
        popup: [0.8388, 0.8388, 0.8308, 1.0],
        line_hard: [0.5089, 0.5089, 0.4910, 1.0],
        line: [0.6514, 0.6514, 0.6308, 1.0],
        line_grid: [0.6867, 0.6867, 0.6654, 1.0],
        text: [0.0123, 0.0130, 0.0144, 1.0],
        text_dim: [0.0685, 0.0723, 0.0802, 1.0],
        text_faint: [0.1946, 0.2051, 0.2232, 1.0],
        text_disabled: [0.4345, 0.4452, 0.4621, 1.0],
        text_on_accent: [0.0103, 0.0060, 0.0018, 1.0],
        console_text: [0.0452, 0.0685, 0.0452, 1.0],
        hover: [0.0, 0.0, 0.0, 0.05],
        active: [0.0, 0.0, 0.0, 0.12],
        accent: [0.6795, 0.1912, 0.0, 1.0],
        accent_bg: [0.6795, 0.1912, 0.0, 0.12],
        accent_bg_hover: [0.6795, 0.1912, 0.0, 0.2],
        danger: [0.6308, 0.0844, 0.0723, 1.0],
        danger_bg: [0.6308, 0.0844, 0.0723, 0.12],
        ok: [0.1022, 0.3813, 0.1441, 1.0],
        dim: [0.0, 0.0, 0.0, 0.35],
        viewport_bg: [0.6939, 0.7231, 0.7529, 1.0],
        viewport_grid: [0.5972, 0.6308, 0.6654, 1.0],
        viewport_label: [0.0452, 0.0612, 0.0782, 1.0],
    };
}

/// Sizes and palette. Sizes are in logical pixels on a 4 px grid; the 20 px row is the
/// density reference (see [`Style::scaled`] for HiDPI).
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// Base font size in pixels.
    pub font_size: f32,
    /// Small / caption font size (tabs, status, hints).
    pub font_size_small: f32,
    /// Section heading font size (small caps style: bold, letter-spaced, faint).
    pub font_size_heading: f32,
    /// Gap between widgets.
    pub spacing: f32,
    /// Inner padding of panels / inputs.
    pub padding: f32,
    /// Horizontal padding of a button.
    pub button_padding: f32,
    /// Height of one text row (labels, tree / table rows, checkboxes, sliders).
    pub row_height: f32,
    /// Height of one control (buttons, inputs, combos, headings).
    pub control_height: f32,
    /// Indentation of nested tree / collapsing content.
    pub indent: f32,
    /// Scrollbar width.
    pub scrollbar: f32,
    /// Splitter grab width in a dock (the drawn line is 1 px).
    pub splitter: f32,
    /// Tab strip height in a dock.
    pub tab_height: f32,
    /// Menu bar height.
    pub menu_height: f32,
    /// Window title bar height (client-side decorations).
    pub title_height: f32,
    /// Toolbar strip height.
    pub toolbar_height: f32,
    /// Status bar height.
    pub status_height: f32,
    /// Seconds before a tooltip appears.
    pub tooltip_delay: f32,
    /// Colours.
    pub palette: Palette,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            font_size: 11.5,
            font_size_small: 11.0,
            font_size_heading: 10.5,
            spacing: 4.0,
            padding: 6.0,
            button_padding: 8.0,
            row_height: 20.0,
            control_height: 22.0,
            indent: 12.0,
            scrollbar: 8.0,
            splitter: 5.0,
            tab_height: 22.0,
            menu_height: 24.0,
            title_height: 26.0,
            toolbar_height: 28.0,
            status_height: 22.0,
            tooltip_delay: 0.45,
            palette: Palette::DARK,
        }
    }
}

impl Style {
    /// Every size multiplied by `k` (the window's scale factor); colours unchanged.
    #[must_use]
    pub fn scaled(&self, k: f32) -> Self {
        Self {
            font_size: self.font_size * k,
            font_size_small: self.font_size_small * k,
            font_size_heading: self.font_size_heading * k,
            spacing: self.spacing * k,
            padding: self.padding * k,
            button_padding: self.button_padding * k,
            row_height: self.row_height * k,
            control_height: self.control_height * k,
            indent: self.indent * k,
            scrollbar: self.scrollbar * k,
            splitter: self.splitter * k,
            tab_height: self.tab_height * k,
            menu_height: self.menu_height * k,
            title_height: self.title_height * k,
            toolbar_height: self.toolbar_height * k,
            status_height: self.status_height * k,
            tooltip_delay: self.tooltip_delay,
            palette: self.palette,
        }
    }
}

/// `c` with its alpha replaced.
pub fn with_alpha(c: Color, a: f32) -> Color {
    [c[0], c[1], c[2], a]
}

/// `a` composited over `b` (straight alpha).
pub fn over(a: Color, b: Color) -> Color {
    let t = a[3];
    [
        a[0] * t + b[0] * (1.0 - t),
        a[1] * t + b[1] * (1.0 - t),
        a[2] * t + b[2] * (1.0 - t),
        (t + b[3] * (1.0 - t)).min(1.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_round_trip() {
        for v in [0.0_f32, 0.02, 0.2, 0.5, 0.9, 1.0] {
            let back = linear_to_srgb(srgb_to_linear(v));
            assert!((back - v).abs() < 1e-5, "{v} -> {back}");
        }
        let c = srgb(0x00e8_8a1a, 1.0);
        assert!((c[0] - 0.807).abs() < 1e-3 && (c[1] - 0.2542).abs() < 1e-3);
        assert!((Palette::DARK.accent[0] - c[0]).abs() < 1e-3);
    }

    #[test]
    fn scaled_multiplies_sizes_only() {
        let s = Style::default().scaled(2.0);
        assert!((s.row_height - 40.0).abs() < 1e-6);
        assert!((s.font_size - 23.0).abs() < 1e-6);
        assert_eq!(s.palette, Palette::DARK);
    }
}
