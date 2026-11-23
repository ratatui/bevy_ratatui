//! Comprehensive windowed font and render verification showcase using JetBrains Mono
//! (SIL OFL 1.1, see `assets/fonts/jetbrains-mono/OFL.txt`).
//!
//! Demonstrates:
//! - Loading all four font faces (regular, bold, italic, bold-italic) dynamically via
//!   `AssetServer` and from embedded bytes via `include_bytes!` + `Font::from_bytes`.
//! - Live runtime font switching, font-size reflow, and cursor-style switching.
//! - Visual verification across all 512 modifier combinations, ANSI 16, 256-color cube and
//!   grayscale ramp, 128-step true-color RGB gradients, underline colors, every box-drawing
//!   weight and junction, block/quadrant/shade/braille/geometric/legacy-computing symbols,
//!   broad Unicode blocks (Latin, Greek, Cyrillic, arrows, math, technical, currency,
//!   combining marks, wide CJK/fullwidth/emoji/RTL/Indic), blinking cursor, and full ASCII.
//!
//! Run with:
//! ```sh
//! cargo run --example windowed_fonts --features windowed
//! ```
//!
//! Keys:
//! - `Tab` / `F` (`Shift+Tab` back): cycle font presets
//! - `+` / `-`: increase or decrease font size (grid refills the window automatically)
//! - `C`: cycle cursor style (`Block`, `Bar`, `Underline`)
//! - `↑`/`↓` or `j`/`k`, `PageUp`/`PageDown`, `Home`/`End`, mouse wheel: scroll
//! - `Q` / `Esc`: quit

use bevy::{app::AppExit, input::mouse::MouseWheel, prelude::*};
use bevy_ratatui::{
    RatatuiContext, RatatuiPlugins,
    windowed::{
        CursorStyle, FontFaces, FontSource, TerminalRenderConfig, TerminalSizing, TerminalTexture,
    },
};
use ratatui::{
    layout::{Constraint, Layout, Position},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

const EMBEDDED_REGULAR: &[u8] =
    include_bytes!("../assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf");
const EMBEDDED_BOLD: &[u8] =
    include_bytes!("../assets/fonts/jetbrains-mono/JetBrainsMono-Bold.ttf");
const EMBEDDED_ITALIC: &[u8] =
    include_bytes!("../assets/fonts/jetbrains-mono/JetBrainsMono-Italic.ttf");
const EMBEDDED_BOLD_ITALIC: &[u8] =
    include_bytes!("../assets/fonts/jetbrains-mono/JetBrainsMono-BoldItalic.ttf");

const MIN_FONT_SIZE: f32 = 10.0;
const MAX_FONT_SIZE: f32 = 48.0;
const FONT_STEP: f32 = 2.0;

const MODIFIERS: [(Modifier, &str); 9] = [
    (Modifier::BOLD, "B"),
    (Modifier::DIM, "D"),
    (Modifier::ITALIC, "I"),
    (Modifier::UNDERLINED, "U"),
    (Modifier::SLOW_BLINK, "S"),
    (Modifier::RAPID_BLINK, "R"),
    (Modifier::REVERSED, "V"),
    (Modifier::HIDDEN, "H"),
    (Modifier::CROSSED_OUT, "X"),
];

const ANSI: [(Color, &str); 16] = [
    (Color::Black, "Blk"),
    (Color::Red, "Red"),
    (Color::Green, "Grn"),
    (Color::Yellow, "Yel"),
    (Color::Blue, "Blu"),
    (Color::Magenta, "Mag"),
    (Color::Cyan, "Cyn"),
    (Color::Gray, "Gry"),
    (Color::DarkGray, "DGy"),
    (Color::LightRed, "LRd"),
    (Color::LightGreen, "LGn"),
    (Color::LightYellow, "LYl"),
    (Color::LightBlue, "LBl"),
    (Color::LightMagenta, "LMg"),
    (Color::LightCyan, "LCy"),
    (Color::White, "Wht"),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FontPreset {
    #[default]
    DynamicFourFace,
    EmbeddedFourFace,
    RegularSynthesized,
    RegularExact,
    BevyDefault,
}

impl FontPreset {
    fn next(self) -> Self {
        match self {
            Self::DynamicFourFace => Self::EmbeddedFourFace,
            Self::EmbeddedFourFace => Self::RegularSynthesized,
            Self::RegularSynthesized => Self::RegularExact,
            Self::RegularExact => Self::BevyDefault,
            Self::BevyDefault => Self::DynamicFourFace,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::DynamicFourFace => Self::BevyDefault,
            Self::EmbeddedFourFace => Self::DynamicFourFace,
            Self::RegularSynthesized => Self::EmbeddedFourFace,
            Self::RegularExact => Self::RegularSynthesized,
            Self::BevyDefault => Self::RegularExact,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::DynamicFourFace => "JetBrains Mono (4 faces, dynamic AssetServer)",
            Self::EmbeddedFourFace => "JetBrains Mono (4 faces, embedded include_bytes!)",
            Self::RegularSynthesized => "JetBrains Mono (regular only, synthesize: true)",
            Self::RegularExact => "JetBrains Mono (regular only, synthesize: false)",
            Self::BevyDefault => "Bevy default_font (FiraMono ASCII subset)",
        }
    }
}

#[derive(Resource)]
struct FontLibrary {
    dynamic: FontFaces,
    embedded: FontFaces,
    regular_synthesized: FontFaces,
    regular_exact: FontFaces,
    bevy_default: FontFaces,
}

impl FontLibrary {
    fn faces(&self, preset: FontPreset) -> FontFaces {
        match preset {
            FontPreset::DynamicFourFace => self.dynamic.clone(),
            FontPreset::EmbeddedFourFace => self.embedded.clone(),
            FontPreset::RegularSynthesized => self.regular_synthesized.clone(),
            FontPreset::RegularExact => self.regular_exact.clone(),
            FontPreset::BevyDefault => self.bevy_default.clone(),
        }
    }
}

#[derive(Resource)]
struct DemoState {
    preset: FontPreset,
    font_size: f32,
    cursor_style: CursorStyle,
    scroll: u16,
    page_height: u16,
}

impl Default for DemoState {
    fn default() -> Self {
        Self {
            preset: FontPreset::DynamicFourFace,
            font_size: 16.0,
            cursor_style: CursorStyle::Block,
            scroll: 0,
            page_height: 24,
        }
    }
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "bevy_ratatui · windowed fonts & render showcase".into(),
                    resolution: (1440, 960).into(),
                    ..default()
                }),
                ..default()
            }),
            RatatuiPlugins::default(),
        ))
        .init_resource::<DemoState>()
        // `RatatuiContext` is inserted during `Startup`, so configure initial fonts in `PostStartup`.
        .add_systems(PostStartup, setup_fonts)
        .add_systems(Update, (handle_input, draw_showcase).chain())
        .run();
}

/// Loads all four JetBrains Mono faces both dynamically (via `AssetServer`) and from embedded
/// bytes, then applies the initial four-face configuration to the terminal.
fn setup_fonts(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut fonts: ResMut<Assets<Font>>,
    state: Res<DemoState>,
    mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>,
) {
    let dynamic_regular: Handle<Font> =
        asset_server.load("fonts/jetbrains-mono/JetBrainsMono-Regular.ttf");
    let dynamic = FontFaces {
        regular: FontSource::from(dynamic_regular.clone()),
        bold: Some(FontSource::from(
            asset_server.load("fonts/jetbrains-mono/JetBrainsMono-Bold.ttf"),
        )),
        italic: Some(FontSource::from(
            asset_server.load("fonts/jetbrains-mono/JetBrainsMono-Italic.ttf"),
        )),
        bold_italic: Some(FontSource::from(
            asset_server.load("fonts/jetbrains-mono/JetBrainsMono-BoldItalic.ttf"),
        )),
        synthesize: true,
    };

    let embedded = FontFaces {
        regular: FontSource::from(fonts.add(Font::from_bytes(EMBEDDED_REGULAR.to_vec()))),
        bold: Some(FontSource::from(
            fonts.add(Font::from_bytes(EMBEDDED_BOLD.to_vec())),
        )),
        italic: Some(FontSource::from(
            fonts.add(Font::from_bytes(EMBEDDED_ITALIC.to_vec())),
        )),
        bold_italic: Some(FontSource::from(
            fonts.add(Font::from_bytes(EMBEDDED_BOLD_ITALIC.to_vec())),
        )),
        synthesize: true,
    };

    let regular_synthesized = FontFaces {
        regular: FontSource::from(dynamic_regular.clone()),
        bold: None,
        italic: None,
        bold_italic: None,
        synthesize: true,
    };

    let regular_exact = FontFaces {
        regular: FontSource::from(dynamic_regular),
        bold: None,
        italic: None,
        bold_italic: None,
        synthesize: false,
    };

    let bevy_default = FontFaces::regular(Handle::<Font>::default());

    let library = FontLibrary {
        dynamic,
        embedded,
        regular_synthesized,
        regular_exact,
        bevy_default,
    };

    config.font = library.faces(state.preset);
    config.sizing = TerminalSizing::font(state.font_size);
    config.cursor.style = state.cursor_style;

    commands.insert_resource(library);
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    library: Option<Res<FontLibrary>>,
    mut state: ResMut<DemoState>,
    mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyQ) || keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
        return;
    }

    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    if let Some(library) = library {
        if (keys.just_pressed(KeyCode::Tab) && !shift) || keys.just_pressed(KeyCode::KeyF) {
            state.preset = state.preset.next();
            config.font = library.faces(state.preset);
        } else if keys.just_pressed(KeyCode::Tab) && shift {
            state.preset = state.preset.prev();
            config.font = library.faces(state.preset);
        }
    }

    if keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd) {
        state.font_size = (state.font_size + FONT_STEP).min(MAX_FONT_SIZE);
        config.sizing = TerminalSizing::font(state.font_size);
    }

    if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
        state.font_size = (state.font_size - FONT_STEP).max(MIN_FONT_SIZE);
        config.sizing = TerminalSizing::font(state.font_size);
    }

    if keys.just_pressed(KeyCode::KeyC) {
        state.cursor_style = match state.cursor_style {
            CursorStyle::Block => CursorStyle::Bar,
            CursorStyle::Bar => CursorStyle::Underline,
            CursorStyle::Underline => CursorStyle::Block,
        };
        config.cursor.style = state.cursor_style;
    }

    let page = i32::from(state.page_height.max(1));
    let mut delta: i32 = wheel
        .read()
        .map(|event| (-event.y.round() as i32).clamp(-8, 8))
        .sum();
    for (key, step) in [
        (KeyCode::ArrowDown, 1),
        (KeyCode::KeyJ, 1),
        (KeyCode::ArrowUp, -1),
        (KeyCode::KeyK, -1),
        (KeyCode::PageDown, page),
        (KeyCode::PageUp, -page),
        (KeyCode::End, i32::from(u16::MAX)),
        (KeyCode::Home, -i32::from(u16::MAX)),
    ] {
        if keys.just_pressed(key) {
            delta += step;
        }
    }
    if delta != 0 {
        state.scroll = (i32::from(state.scroll) + delta).clamp(0, i32::from(u16::MAX)) as u16;
    }
}

fn heading(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_owned(),
        Style::new()
            .fg(Color::Black)
            .bg(Color::LightCyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn label(text: &str) -> Span<'static> {
    Span::styled(text.to_owned(), Style::new().fg(Color::DarkGray))
}

fn modifier_combination(bits: u16) -> Modifier {
    MODIFIERS
        .iter()
        .enumerate()
        .filter(|(index, _)| bits & (1 << index) != 0)
        .fold(Modifier::empty(), |set, (_, (modifier, _))| set | *modifier)
}

fn hue(t: f32) -> (f32, f32, f32) {
    let h = t * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    }
}

/// Builds every visual verification section and returns `(lines, cursor_line_index)`.
fn build_showcase_lines() -> (Vec<Line<'static>>, u16) {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // 1. Font faces and single modifiers.
    lines.push(heading(
        " 1. Faces & single modifiers (regular / bold / italic / bold+italic should be four distinct faces) ",
    ));
    let mut faces = vec![label("faces:  ")];
    for (text, modifier) in [
        ("Regular", Modifier::empty()),
        ("Bold", Modifier::BOLD),
        ("Italic", Modifier::ITALIC),
        ("BoldItalic", Modifier::BOLD | Modifier::ITALIC),
    ] {
        faces.push(Span::styled(text, Style::new().add_modifier(modifier)));
        faces.push(Span::raw("  "));
    }
    faces.push(label("| single: "));
    for (modifier, _) in MODIFIERS {
        let name = format!("{modifier:?}")
            .replace("Modifier(", "")
            .replace(')', "");
        faces.push(Span::styled(
            name,
            Style::new()
                .fg(Color::White)
                .bg(Color::Indexed(236))
                .add_modifier(modifier),
        ));
        faces.push(Span::raw(" "));
    }
    lines.push(Line::from(faces));

    // 2. All 512 modifier combinations (9 modifier bits -> 8 rows x 64 columns).
    let mut legend = String::from("bits: ");
    for (index, (_, code)) in MODIFIERS.iter().enumerate() {
        legend.push_str(&format!("{index}={code} "));
    }
    lines.push(heading(&format!(
        " 2. All 512 modifier combinations ({legend}); 'A' on gold/navy, LightGreen underline; bit 7 (H) shows bg only "
    )));
    for row in 0..8_u16 {
        let mut spans = vec![label(&format!("{:>3} ", row * 64))];
        for column in 0..64_u16 {
            let bits = row * 64 + column;
            let style = Style::new()
                .fg(Color::Rgb(230, 200, 60))
                .bg(Color::Rgb(30, 30, 60))
                .underline_color(Color::LightGreen)
                .add_modifier(modifier_combination(bits));
            spans.push(Span::styled("A", style));
            spans.push(Span::styled(
                " ",
                Style::new().bg(if bits.is_multiple_of(2) {
                    Color::Reset
                } else {
                    Color::Indexed(234)
                }),
            ));
        }
        lines.push(Line::from(spans));
    }

    // 3. ANSI 16 colors.
    lines.push(heading(
        " 3. ANSI 16 colors: A=fg on default, B=default on bg, C=fg on inverse bg, D=REVERSED ",
    ));
    let mut fg = vec![label("A ")];
    let mut bg = vec![label("B ")];
    let mut both = vec![label("C ")];
    let mut rev = vec![label("D ")];
    for (index, (color, name)) in ANSI.iter().enumerate() {
        let inverse = ANSI
            .get(ANSI.len().saturating_sub(1 + index))
            .map_or(Color::Reset, |(c, _)| *c);
        fg.push(Span::styled(format!(" {name} "), Style::new().fg(*color)));
        bg.push(Span::styled(format!(" {name} "), Style::new().bg(*color)));
        both.push(Span::styled(
            format!(" {name} "),
            Style::new().fg(*color).bg(inverse),
        ));
        rev.push(Span::styled(
            format!(" {name} "),
            Style::new().fg(*color).add_modifier(Modifier::REVERSED),
        ));
        for spans in [&mut fg, &mut bg, &mut both, &mut rev] {
            spans.push(Span::raw(" "));
        }
    }
    lines.extend([
        Line::from(fg),
        Line::from(bg),
        Line::from(both),
        Line::from(rev),
    ]);

    // 4. 256-color cube (16..=231) and grayscale ramp (232..=255).
    lines.push(heading(
        " 4. Indexed 256-color cube (16..231, 6x36) and grayscale ramp (232..255); digits show index mod 10 ",
    ));
    for row in 0..6_u8 {
        let mut spans = vec![label(&format!("{:>3} ", 16 + row * 36))];
        for column in 0..36_u8 {
            let index = 16 + row * 36 + column;
            spans.push(Span::styled(
                format!("{}", index % 10),
                Style::new().bg(Color::Indexed(index)).fg(Color::Black),
            ));
            spans.push(Span::styled("  ", Style::new().bg(Color::Indexed(index))));
        }
        lines.push(Line::from(spans));
    }
    let mut gray = vec![label("232 ")];
    for index in 232..=255_u8 {
        let text_fg = if index < 244 {
            Color::White
        } else {
            Color::Black
        };
        gray.push(Span::styled(
            format!(" {} ", index % 10),
            Style::new().bg(Color::Indexed(index)).fg(text_fg),
        ));
    }
    lines.push(Line::from(gray));

    // 5. True-color RGB gradients.
    lines.push(heading(
        " 5. True-color RGB: 128-step R / G / B / Hue backgrounds and foreground blocks + text ",
    ));
    for (name, f) in [
        (
            "R  ",
            (|t: f32| (t, 0.0, 0.0)) as fn(f32) -> (f32, f32, f32),
        ),
        ("G  ", |t: f32| (0.0, t, 0.0)),
        ("B  ", |t: f32| (0.0, 0.0, t)),
        ("H  ", hue),
    ] {
        let mut spans = vec![label(name)];
        for step in 0..128 {
            let (r, g, b) = f(step as f32 / 127.0);
            spans.push(Span::styled(
                " ",
                Style::new().bg(Color::Rgb(
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                )),
            ));
        }
        lines.push(Line::from(spans));
    }
    let mut fg_blocks = vec![label("fg ")];
    let mut fg_text = vec![label("tx ")];
    const RGB_LABELS: [&str; 3] = ["R", "G", "B"];
    for step in 0..128 {
        let (r, g, b) = hue(step as f32 / 127.0);
        let color = Color::Rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
        fg_blocks.push(Span::styled("█", Style::new().fg(color)));
        let ch = RGB_LABELS.get(step % 3).copied().unwrap_or("R");
        fg_text.push(Span::styled(ch, Style::new().fg(color)));
    }
    lines.push(Line::from(fg_blocks));
    lines.push(Line::from(fg_text));

    // 6. Underline colors.
    lines.push(heading(
        " 6. Underline colors: white text underlined in each ANSI color + RGB(255,128,0); last two DIM / REVERSED ",
    ));
    let mut ul = vec![label("   ")];
    for (index, (color, name)) in ANSI.iter().enumerate() {
        let mut style = Style::new()
            .fg(Color::White)
            .underline_color(*color)
            .add_modifier(Modifier::UNDERLINED);
        if index == 14 {
            style = style.add_modifier(Modifier::DIM);
        }
        if index == 15 {
            style = style.add_modifier(Modifier::REVERSED);
        }
        ul.push(Span::styled((*name).to_owned(), style));
        ul.push(Span::raw("  "));
    }
    ul.push(Span::styled(
        "Rgb(255,128,0)",
        Style::new()
            .underline_color(Color::Rgb(255, 128, 0))
            .add_modifier(Modifier::UNDERLINED),
    ));
    lines.push(Line::from(ul));

    // 7. Box drawing: every weight, dashed, rounded, double, diagonals, half-lines, mixed junctions.
    lines.push(heading(
        " 7. Box drawing: light, heavy, double, rounded, dashed, mixed junctions, diagonals, nested frames (must be seamless) ",
    ));
    for text in [
        "┌─┬─┐ ┏━┳━┓ ╔═╦═╗ ╭─┬─╮ ┌╌╌┬╌╌┐ ┍━┯━┑ ┎─┰─┒ ╒═╤═╕ ╓─╥─╖  ╱╲╳  ┄┄┄┄ ┅┅┅┅ ┆┆ ┇┇ ┈┈┈┈ ┉┉┉┉ ┊┊ ┋┋ ╌╌╌╌ ╍╍╍╍ ╎╎ ╏╏",
        "│ │ │ ┃ ┃ ┃ ║ ║ ║ │ │ │ ┆  ┆  ┆ │ │ │ ┃ ┃ ┃ │ │ │ ║ ║ ║   ╲╱   ╴╵╶╷ ╸╹╺╻ ╼╽╾╿  ┝┞┟┠┡┢┣┤┥┦┧┨┩┪┫┬┭┮┯┰┱┲┳┴┵┶┷┸┹┺┻",
        "├─┼─┤ ┣━╋━┫ ╠═╬═╣ ├─┼─┤ ├╌╌┼╌╌┤ ┝━┿━┥ ┠─╂─┨ ╞═╪═╡ ╟─╫─╢        ┼┽┾┿╀╁╂╃╄╅╆╇╈╉╊╋ ╪╫╬ ╭╮╯╰ ┏┓┗┛ ╔╗╚╝",
        "└─┴─┘ ┗━┻━┛ ╚═╩═╝ ╰─┴─╯ └╌╌┴╌╌┘ ┕━┷━┙ ┖─┸─┚ ╘═╧═╛ ╙─╨─╜  ┌──────────────────────────────────┐",
        "──────────────────────────────────────────────────────── │ ┏━━━━━━━━━┓  ╔═════════╗  ╭───╮  │",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │ ┃ ╔═════╗ ┃  ║ ┌─────┐ ║  │ ● │  │",
        "════════════════════════════════════════════════════════ │ ┃ ║  ┼  ║ ┃  ║ │ ─┼─ │ ║  ╰───╯  │",
    ] {
        lines.push(Line::from(text));
    }
    lines.push(Line::from(vec![
        Span::styled(
            "┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄ ┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈ ",
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(
            "│ ┗━━━━━━━━━┛  ╚═════════╝         │",
            Style::new().fg(Color::LightGreen),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw("                                                         "),
        Span::styled(
            "└──────────────────────────────────┘",
            Style::new().fg(Color::LightBlue),
        ),
    ]));

    // 8. Blocks, quadrants, shades, braille, geometric shapes, Powerline, Legacy Computing sextants.
    lines.push(heading(
        " 8. Block elements, quadrants, shades, braille, geometric shapes, Powerline & Legacy Computing sextants ",
    ));
    for text in [
        "Blocks:   █▉▊▋▌▍▎▏  ▁▂▃▄▅▆▇█  ▀▄ ▐▌ ▔▕   Quadrants: ▖▗▘▙▚▛▜▝▞▟   Shades: ░▒▓█",
        "Braille:  ⠁⠃⠇⡇⣇⣧⣷⣿ ⣿⣷⣯⣟⡿⢿⣻⣽⣼⣧ ⠉⠛⠿⣿   Geometric: ◢◣◤◥ ▲▼◀▶ ►◄ ▴▾◂▸ ●○◐◑◒◓ ■□▪▫ ◆◇",
        "Powerline: \u{e0b0} \u{e0b1} \u{e0b2} \u{e0b3} \u{e0b4} \u{e0b5} \u{e0b6} \u{e0b7}   Legacy sextants & wedges: 🬀🬁🬂🬃🬄🬅🬆🬇🬈🬉🬊🬋🬌🬍🬎🬏 🬼🬽🬾🬿🭀 🭭🭮🭯🭰",
    ] {
        lines.push(Line::from(text));
    }
    let mut colored_blocks = vec![label("styled:   ")];
    for (index, (color, _)) in ANSI.iter().enumerate() {
        let pair_bg = ANSI
            .get((index + 8) % ANSI.len())
            .map_or(Color::Reset, |(c, _)| *c);
        colored_blocks.push(Span::styled("█▓▒░ ", Style::new().fg(*color).bg(pair_bg)));
    }
    colored_blocks.push(Span::styled(
        "▀▄▀▄▀▄",
        Style::new()
            .fg(Color::Rgb(255, 0, 128))
            .bg(Color::Rgb(0, 128, 255)),
    ));
    colored_blocks.push(Span::styled(
        " ▛▜▙▟ ",
        Style::new().fg(Color::Yellow).bg(Color::Blue),
    ));
    lines.push(Line::from(colored_blocks));
    lines.push(Line::from(vec![
        label("checker:  "),
        Span::styled(
            "▀▄".repeat(20),
            Style::new().fg(Color::White).bg(Color::Black),
        ),
        Span::raw(" "),
        Span::styled(
            "▚▞".repeat(20),
            Style::new().fg(Color::White).bg(Color::Black),
        ),
        Span::raw(" "),
        Span::styled(
            "░▒▓█".repeat(6),
            Style::new().fg(Color::White).bg(Color::Black),
        ),
    ]));

    // 9. Unicode blocks & wide/combining/script samples with guard columns.
    lines.push(heading(
        " 9. Unicode blocks & scripts (wide cells occupy 2 cols without overlapping '|' guards; coverage depends on active font) ",
    ));
    for text in [
        "|Latin-1  |ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖØÙÚÛÜÝÞß| |café naïve résumé über Ångström Łódź ğşİı|",
        "|Greek    |ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩ| |αβγδεζηθικλμνξοπρστυφχψω|",
        "|Cyrillic |АБВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ| |привет мир ёж|",
        "|Arrows   |← ↑ → ↓ ↔ ↕ ↖ ↗ ↘ ↙| |⇐ ⇑ ⇒ ⇓ ⇔ ↺ ↻ ↵ ⏎|",
        "|Math/Tech|± × ÷ ≠ ≤ ≥ ≈ ≡ ∞ ∑ ∏ √ ∫ ∂ ∆ ∇ ∈ ∉ ∩ ∪ ⊂ ⊃ ∀ ∃ ∧ ∨ ¬| |⌘ ⌥ ⇧ ⌃ ⌫ ⎋|",
        "|Punct/Sym|© ® ™ ° µ ¶ § † ‡ • ◦ … – — « » ‹ › “ ” ‘ ’| |€ £ ¥ ¢ ₿| |☐ ☑ ☒ ♠ ♣ ♥ ♦ ★ ☆ ♪ ♫|",
        "|Combining|e\u{301}|A\u{30a}|n\u{303}|o\u{308}\u{304}|Z\u{330}\u{301}|  precomposed: |é|Å|ñ|  (must stay 1 cell each)",
        "|Wide CJK |汉字|日本語|한글|中文测试|  |ｆｕｌｌｗｉｄｔｈ|  guards: |0123456789|",
        "|Mixed    |a汉b字c日d本e語f|  |narrow|wide|narrow|  the guards | must | stay | aligned |",
        "|Emoji/RTL|🙂|🚀|🎉|❤️|✅|⚠️|  |שלום עולם|مرحبا بالعالم|  |नमस्ते|தமிழ்|ไทย|  (font-dependent)",
    ] {
        lines.push(Line::from(text));
    }
    let mut styled_wide = vec![Span::raw("|Styled   |")];
    for (text, style) in [
        (
            "汉字",
            Style::new()
                .fg(Color::Black)
                .bg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        ),
        (
            "日本",
            Style::new()
                .fg(Color::LightRed)
                .add_modifier(Modifier::UNDERLINED | Modifier::ITALIC),
        ),
        ("🚀🙂", Style::new().bg(Color::Indexed(238))),
        ("한글", Style::new().add_modifier(Modifier::REVERSED)),
        ("中文", Style::new().add_modifier(Modifier::CROSSED_OUT)),
        ("測試", Style::new().add_modifier(Modifier::DIM)),
    ] {
        styled_wide.push(Span::styled(text, style));
        styled_wide.push(Span::raw("|"));
    }
    styled_wide.push(Span::raw(" wide + styles"));
    lines.push(Line::from(styled_wide));

    // 10. Cursor and full printable ASCII in all 4 faces.
    lines.push(heading(
        " 10. Blinking cursor (press C to cycle Block / Bar / Underline) & full printable ASCII across all 4 faces ",
    ));
    let cursor_line_index = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    lines.push(Line::from("cursor here > "));
    const ASCII: &str = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
    for (tag, modifier) in [
        ("Reg ", Modifier::empty()),
        ("Bld ", Modifier::BOLD),
        ("Itl ", Modifier::ITALIC),
        ("B+I ", Modifier::BOLD | Modifier::ITALIC),
    ] {
        lines.push(Line::from(vec![
            label(tag),
            Span::styled(ASCII, Style::new().add_modifier(modifier)),
        ]));
    }

    (lines, cursor_line_index)
}

fn draw_showcase(
    mut state: ResMut<DemoState>,
    texture: Single<&TerminalTexture, With<RatatuiContext>>,
    mut context: ResMut<RatatuiContext>,
) -> Result {
    let metrics = match texture.measured() {
        Some(geometry) => format!(
            "{}×{} cells · cell {:.1}×{:.1} px · scale {:.2}×",
            geometry.grid().width,
            geometry.grid().height,
            geometry.cell_size().x,
            geometry.cell_size().y,
            geometry.raster_scale(),
        ),
        None => format!("status: {:?}", texture.status()),
    };

    let (lines, cursor_line_index) = build_showcase_lines();
    let total_lines = u16::try_from(lines.len()).unwrap_or(u16::MAX);

    context.draw(|frame| {
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .areas(frame.area());

        state.page_height = body.height;
        let max_scroll = total_lines.saturating_sub(body.height);
        state.scroll = state.scroll.min(max_scroll);

        let header_block = Block::bordered()
            .title(" bevy_ratatui · Windowed Fonts & Render Verification ")
            .border_style(Style::new().fg(Color::Cyan));
        let header_text = vec![
            Line::from(vec![
                Span::styled("Font: ", Style::new().fg(Color::DarkGray)),
                Span::styled(
                    state.preset.label(),
                    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Size: ", Style::new().fg(Color::DarkGray)),
                Span::raw(format!("{:.0} px", state.font_size)),
                Span::styled("  ·  Cursor: ", Style::new().fg(Color::DarkGray)),
                Span::raw(format!("{:?}", state.cursor_style)),
                Span::styled("  ·  Scroll: ", Style::new().fg(Color::DarkGray)),
                Span::raw(format!("{}/{}", state.scroll, max_scroll)),
                Span::styled("  ·  ", Style::new().fg(Color::DarkGray)),
                Span::styled(metrics, Style::new().fg(Color::Green)),
            ]),
        ];
        frame.render_widget(Paragraph::new(header_text).block(header_block), header);

        frame.render_widget(Paragraph::new(lines).scroll((state.scroll, 0)), body);

        if let Some(relative_row) = cursor_line_index.checked_sub(state.scroll)
            && relative_row < body.height
            && body.width > 14
        {
            frame.set_cursor_position(Position::new(body.x + 14, body.y + relative_row));
        }

        let footer_text = Line::from(vec![
            Span::styled("[Tab/F]", Style::new().fg(Color::Yellow)),
            Span::raw(" Font (Shift+Tab back)   "),
            Span::styled("[+/-]", Style::new().fg(Color::Yellow)),
            Span::raw(" Size   "),
            Span::styled("[C]", Style::new().fg(Color::Yellow)),
            Span::raw(" Cursor   "),
            Span::styled("[↑↓/jk/PgUp/PgDn/Wheel]", Style::new().fg(Color::Yellow)),
            Span::raw(" Scroll   "),
            Span::styled("[Q/Esc]", Style::new().fg(Color::Yellow)),
            Span::raw(" Quit"),
        ]);
        frame.render_widget(Paragraph::new(footer_text).block(Block::bordered()), footer);
    })?;

    Ok(())
}
