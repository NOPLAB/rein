//! Visual style: sizes and a colour palette. Widgets read everything from here, so an
//! application re-themes by replacing the [`Style`] on its context.

/// RGBA colour, linear 0..1 (the GUI pipeline blends in the target's colour space).
pub type Color = [f32; 4];

/// Colour palette. Three hues by budget: `accent` for the active / focused state,
/// `danger` for stop / destructive, `ok` for healthy; everything else is grey.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Window background.
    pub background: Color,
    /// Panel surface.
    pub panel: Color,
    /// Alternate surface (inputs, list rows, tab strips).
    pub panel_alt: Color,
    /// Raised surface (popups, tooltips).
    pub popup: Color,
    /// Borders and separators.
    pub line: Color,
    /// Primary text.
    pub text: Color,
    /// Secondary text.
    pub text_dim: Color,
    /// Text on accent.
    pub text_on_accent: Color,
    /// Hover overlay.
    pub hover: Color,
    /// Pressed overlay.
    pub active: Color,
    /// Accent (selection, focus ring, active tab).
    pub accent: Color,
    /// Danger.
    pub danger: Color,
    /// Healthy / ok.
    pub ok: Color,
    /// Modal dimming.
    pub dim: Color,
}

impl Palette {
    /// Dark palette: neutral greys, orange accent, red danger, green ok.
    pub const DARK: Self = Self {
        background: [0.094, 0.094, 0.106, 1.0],
        panel: [0.125, 0.125, 0.141, 1.0],
        panel_alt: [0.165, 0.165, 0.184, 1.0],
        popup: [0.19, 0.19, 0.21, 1.0],
        line: [0.27, 0.27, 0.30, 1.0],
        text: [0.90, 0.90, 0.91, 1.0],
        text_dim: [0.60, 0.60, 0.63, 1.0],
        text_on_accent: [0.08, 0.06, 0.03, 1.0],
        hover: [1.0, 1.0, 1.0, 0.07],
        active: [1.0, 1.0, 1.0, 0.14],
        accent: [0.95, 0.55, 0.15, 1.0],
        danger: [0.86, 0.22, 0.20, 1.0],
        ok: [0.30, 0.72, 0.38, 1.0],
        dim: [0.0, 0.0, 0.0, 0.55],
    };
}

/// Sizes and palette.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// Base font size in pixels.
    pub font_size: f32,
    /// Small / caption font size.
    pub font_size_small: f32,
    /// Heading font size.
    pub font_size_heading: f32,
    /// Gap between widgets.
    pub spacing: f32,
    /// Inner padding of buttons / inputs.
    pub padding: f32,
    /// Height of one interactive row (buttons, inputs).
    pub row_height: f32,
    /// Indentation of nested tree / collapsing content.
    pub indent: f32,
    /// Scrollbar width.
    pub scrollbar: f32,
    /// Splitter grab width in a dock.
    pub splitter: f32,
    /// Tab strip height in a dock.
    pub tab_height: f32,
    /// Menu bar height.
    pub menu_height: f32,
    /// Seconds before a tooltip appears.
    pub tooltip_delay: f32,
    /// Colours.
    pub palette: Palette,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            font_size: 13.0,
            font_size_small: 11.0,
            font_size_heading: 15.0,
            spacing: 6.0,
            padding: 6.0,
            row_height: 24.0,
            indent: 14.0,
            scrollbar: 8.0,
            splitter: 5.0,
            tab_height: 26.0,
            menu_height: 26.0,
            tooltip_delay: 0.45,
            palette: Palette::DARK,
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
