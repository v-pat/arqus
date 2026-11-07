//! Semantic colors for the TUI.
//!
//! Every drawing function asks the theme for a color. The theme converts the
//! same palette to truecolor, 256 colors, 16 colors or none.

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    True,
    Indexed256,
    Ansi16,
    None,
}

impl ColorDepth {
    /// Pick a depth from the environment values. Kept pure so it can be tested.
    pub fn detect(
        colorterm: Option<&str>,
        term: Option<&str>,
        no_color: Option<&str>,
    ) -> ColorDepth {
        if no_color.map(|v| !v.is_empty()).unwrap_or(false) {
            return ColorDepth::None;
        }
        let colorterm = colorterm.unwrap_or("").to_ascii_lowercase();
        if colorterm.contains("truecolor") || colorterm.contains("24bit") {
            return ColorDepth::True;
        }
        let term = term.unwrap_or("").to_ascii_lowercase();
        if term.contains("256color") {
            return ColorDepth::Indexed256;
        }
        if term.is_empty() || term == "dumb" {
            return ColorDepth::None;
        }
        ColorDepth::Ansi16
    }

    pub fn from_env() -> ColorDepth {
        let colorterm = std::env::var("COLORTERM").ok();
        let term = std::env::var("TERM").ok();
        let no_color = std::env::var("NO_COLOR").ok();
        ColorDepth::detect(colorterm.as_deref(), term.as_deref(), no_color.as_deref())
    }
}

/// Nearest xterm-256 color: the 6x6x6 cube or the 24-step gray ramp.
pub fn rgb_to_xterm256(r: u8, g: u8, b: u8) -> u8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let mut cube = [0usize; 3];
    for (i, value) in [r, g, b].iter().enumerate() {
        let mut best = 0;
        let mut best_dist = u8::MAX;
        for (j, level) in LEVELS.iter().enumerate() {
            let dist = value.abs_diff(*level);
            if dist < best_dist {
                best_dist = dist;
                best = j;
            }
        }
        cube[i] = best;
    }
    let cube_index = (16 + cube[0] * 36 + cube[1] * 6 + cube[2]) as u8;
    let cube_rgb = (LEVELS[cube[0]], LEVELS[cube[1]], LEVELS[cube[2]]);
    let cube_dist = dist2((r, g, b), cube_rgb);

    let gray_steps = (r as i32 + g as i32 + b as i32) / 3;
    let gray_i = ((gray_steps - 8).max(0) / 10).min(23);
    let gray_value = (8 + gray_i * 10) as u8;
    let gray_index = 232 + gray_i as u8;
    let gray_dist = dist2((r, g, b), (gray_value, gray_value, gray_value));

    if gray_dist < cube_dist {
        gray_index
    } else {
        cube_index
    }
}

/// Nearest of the 16 ANSI colors, returned as an indexed color.
pub fn rgb_to_ansi16(r: u8, g: u8, b: u8) -> u8 {
    const ANSI: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    let mut best = 0;
    let mut best_dist = u32::MAX;
    for (index, (ar, ag, ab)) in ANSI.iter().enumerate() {
        let dist = dist2((r, g, b), (*ar, *ag, *ab));
        if dist < best_dist {
            best_dist = dist;
            best = index as u8;
        }
    }
    best
}

fn dist2(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
    let dr = a.0 as i32 - b.0 as i32;
    let dg = a.1 as i32 - b.1 as i32;
    let db = a.2 as i32 - b.2 as i32;
    (dr * dr + dg * dg + db * db) as u32
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub name: String,
    pub depth: ColorDepth,
    pub fg: Color,
    pub dim: Color,
    pub accent: Color,
    pub user: Color,
    pub assistant: Color,
    pub thinking: Color,
    pub tool: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub border: Color,
    pub border_focus: Color,
    pub selection: Color,
    pub diff_add: Color,
    pub diff_del: Color,
    pub code_bg: Color,
}

impl Theme {
    pub fn from_env() -> Theme {
        Theme::dark(ColorDepth::from_env())
    }

    pub fn dark(depth: ColorDepth) -> Theme {
        Theme::build("dark", depth, &DARK)
    }

    pub fn light(depth: ColorDepth) -> Theme {
        Theme::build("light", depth, &LIGHT)
    }

    pub fn mono() -> Theme {
        Theme::build("mono", ColorDepth::None, &MONO)
    }

    /// The one place that turns an RGB palette into a terminal color.
    pub fn color(&self, rgb: (u8, u8, u8)) -> Color {
        match self.depth {
            ColorDepth::True => Color::Rgb(rgb.0, rgb.1, rgb.2),
            ColorDepth::Indexed256 => Color::Indexed(rgb_to_xterm256(rgb.0, rgb.1, rgb.2)),
            ColorDepth::Ansi16 => Color::Indexed(rgb_to_ansi16(rgb.0, rgb.1, rgb.2)),
            ColorDepth::None => Color::Reset,
        }
    }

    fn build(name: &str, depth: ColorDepth, p: &Palette) -> Theme {
        let c = |rgb: (u8, u8, u8)| match depth {
            ColorDepth::True => Color::Rgb(rgb.0, rgb.1, rgb.2),
            ColorDepth::Indexed256 => Color::Indexed(rgb_to_xterm256(rgb.0, rgb.1, rgb.2)),
            ColorDepth::Ansi16 => Color::Indexed(rgb_to_ansi16(rgb.0, rgb.1, rgb.2)),
            ColorDepth::None => Color::Reset,
        };
        Theme {
            name: name.to_string(),
            depth,
            fg: c(p.fg),
            dim: c(p.dim),
            accent: c(p.accent),
            user: c(p.user),
            assistant: c(p.assistant),
            thinking: c(p.thinking),
            tool: c(p.tool),
            success: c(p.success),
            warning: c(p.warning),
            error: c(p.error),
            border: c(p.border),
            border_focus: c(p.border_focus),
            selection: c(p.selection),
            diff_add: c(p.diff_add),
            diff_del: c(p.diff_del),
            code_bg: c(p.code_bg),
        }
    }
}

struct Palette {
    fg: (u8, u8, u8),
    dim: (u8, u8, u8),
    accent: (u8, u8, u8),
    user: (u8, u8, u8),
    assistant: (u8, u8, u8),
    thinking: (u8, u8, u8),
    tool: (u8, u8, u8),
    success: (u8, u8, u8),
    warning: (u8, u8, u8),
    error: (u8, u8, u8),
    border: (u8, u8, u8),
    border_focus: (u8, u8, u8),
    selection: (u8, u8, u8),
    diff_add: (u8, u8, u8),
    diff_del: (u8, u8, u8),
    code_bg: (u8, u8, u8),
}

static DARK: Palette = Palette {
    fg: (205, 214, 244),
    dim: (110, 118, 129),
    accent: (122, 162, 247),
    user: (86, 182, 194),
    assistant: (205, 214, 244),
    thinking: (110, 118, 129),
    tool: (224, 175, 104),
    success: (158, 206, 106),
    warning: (224, 175, 104),
    error: (224, 108, 117),
    border: (68, 71, 90),
    border_focus: (122, 162, 247),
    selection: (50, 70, 110),
    diff_add: (80, 160, 90),
    diff_del: (200, 80, 90),
    code_bg: (35, 38, 50),
};

static LIGHT: Palette = Palette {
    fg: (30, 30, 30),
    dim: (120, 120, 120),
    accent: (30, 90, 180),
    user: (20, 120, 130),
    assistant: (30, 30, 30),
    thinking: (120, 120, 120),
    tool: (150, 100, 20),
    success: (40, 130, 60),
    warning: (150, 100, 20),
    error: (180, 40, 50),
    border: (170, 170, 170),
    border_focus: (30, 90, 180),
    selection: (200, 220, 250),
    diff_add: (40, 130, 60),
    diff_del: (180, 40, 50),
    code_bg: (235, 235, 235),
};

static MONO: Palette = Palette {
    fg: (255, 255, 255),
    dim: (255, 255, 255),
    accent: (255, 255, 255),
    user: (255, 255, 255),
    assistant: (255, 255, 255),
    thinking: (255, 255, 255),
    tool: (255, 255, 255),
    success: (255, 255, 255),
    warning: (255, 255, 255),
    error: (255, 255, 255),
    border: (255, 255, 255),
    border_focus: (255, 255, 255),
    selection: (255, 255, 255),
    diff_add: (255, 255, 255),
    diff_del: (255, 255, 255),
    code_bg: (255, 255, 255),
};

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
