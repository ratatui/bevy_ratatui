//! Comprehensive windowed font and render verification showcase using bundled monospace fonts
//! (zero `system_fonts` dependency; see licenses under `assets/fonts/`).
//!
//! Demonstrates a distinct monospace family for each font loading method, paired with bundled
//! fallback chains (`FontSource::list`) for symbols, RTL scripts, CJK, Color Emoji, and Nerd icons:
//! - **JetBrains Mono** (4 faces): dynamic loading via `AssetServer` + dynamic fallback list
//! - **Hack** (4 faces): embedded bytes via `include_bytes!` + `Font::from_bytes` + embedded list
//! - **DejaVu Sans Mono** (regular only): single-face fallback with `synthesize: true` vs `false`
//! - **Cascadia Mono** (regular only): single-face monospace with native Arabic/Hebrew coverage
//! - **Bevy `default_font`** (`FiraMono` ASCII subset): built-in baseline with no extra fallbacks
//!
//! For OS system font discovery (`FontSource::monospace()`, named system families, and automatic
//! OS fallback for arbitrary scripts), see `examples/windowed_system_fonts.rs`.
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

use std::time::Duration;

use bevy::{
    app::AppExit,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
        mouse::MouseWheel,
    },
    prelude::*,
    winit::{UpdateMode, WinitSettings},
};
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

// Embedded preset: Hack (4 faces) + embedded fallback chain (zero runtime asset files required).
const EMBEDDED_HACK_REGULAR: &[u8] = include_bytes!("../assets/fonts/hack/Hack-Regular.ttf");
const EMBEDDED_HACK_BOLD: &[u8] = include_bytes!("../assets/fonts/hack/Hack-Bold.ttf");
const EMBEDDED_HACK_ITALIC: &[u8] = include_bytes!("../assets/fonts/hack/Hack-Italic.ttf");
const EMBEDDED_HACK_BOLD_ITALIC: &[u8] = include_bytes!("../assets/fonts/hack/Hack-BoldItalic.ttf");
const EMBEDDED_FALLBACK_DEJAVU: &[u8] =
    include_bytes!("../assets/fonts/dejavu-sans-mono/DejaVuSansMono.ttf");
const EMBEDDED_FALLBACK_CASCADIA: &[u8] =
    include_bytes!("../assets/fonts/cascadia-mono/CascadiaMono-Regular.ttf");
const EMBEDDED_FALLBACK_CJK: &[u8] = include_bytes!("../assets/fonts/fidelity/FidelityCJK.otf");
const EMBEDDED_FALLBACK_EMOJI: &[u8] =
    include_bytes!("../assets/fonts/fidelity/FidelityColorEmoji.ttf");
const EMBEDDED_FALLBACK_NERD: &[u8] =
    include_bytes!("../assets/fonts/fidelity/FidelityNerdSymbols.ttf");

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
    DynamicJetBrains,
    EmbeddedHack,
    DejaVuSynthesized,
    DejaVuExact,
    CascadiaSingle,
    BevyDefault,
}

impl FontPreset {
    fn next(self) -> Self {
        match self {
            Self::DynamicJetBrains => Self::EmbeddedHack,
            Self::EmbeddedHack => Self::DejaVuSynthesized,
            Self::DejaVuSynthesized => Self::DejaVuExact,
            Self::DejaVuExact => Self::CascadiaSingle,
            Self::CascadiaSingle => Self::BevyDefault,
            Self::BevyDefault => Self::DynamicJetBrains,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::DynamicJetBrains => Self::BevyDefault,
            Self::EmbeddedHack => Self::DynamicJetBrains,
            Self::DejaVuSynthesized => Self::EmbeddedHack,
            Self::DejaVuExact => Self::DejaVuSynthesized,
            Self::CascadiaSingle => Self::DejaVuExact,
            Self::BevyDefault => Self::CascadiaSingle,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::DynamicJetBrains => {
                "JetBrains Mono (4 faces, dynamic AssetServer + FontSource::list fallbacks)"
            }
            Self::EmbeddedHack => {
                "Hack (4 faces, embedded include_bytes! + embedded FontSource::list)"
            }
            Self::DejaVuSynthesized => {
                "DejaVu Sans Mono (regular only, synthesize: true + fallbacks)"
            }
            Self::DejaVuExact => "DejaVu Sans Mono (regular only, synthesize: false + fallbacks)",
            Self::CascadiaSingle => "Cascadia Mono (regular only, dynamic AssetServer + fallbacks)",
            Self::BevyDefault => "Bevy default_font (FiraMono ASCII subset, no fallbacks)",
        }
    }
}

#[derive(Resource)]
struct FontLibrary {
    dynamic_jetbrains: FontFaces,
    embedded_hack: FontFaces,
    dejavu_synthesized: FontFaces,
    dejavu_exact: FontFaces,
    cascadia_single: FontFaces,
    bevy_default: FontFaces,
}

impl FontLibrary {
    fn faces(&self, preset: FontPreset) -> FontFaces {
        match preset {
            FontPreset::DynamicJetBrains => self.dynamic_jetbrains.clone(),
            FontPreset::EmbeddedHack => self.embedded_hack.clone(),
            FontPreset::DejaVuSynthesized => self.dejavu_synthesized.clone(),
            FontPreset::DejaVuExact => self.dejavu_exact.clone(),
            FontPreset::CascadiaSingle => self.cascadia_single.clone(),
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
            preset: FontPreset::DynamicJetBrains,
            font_size: 16.0,
            cursor_style: CursorStyle::Block,
            scroll: 0,
            page_height: 24,
        }
    }
}

fn with_fallbacks(primary: Handle<Font>, fallbacks: &[Handle<Font>]) -> FontSource {
    let mut chain = Vec::with_capacity(1 + fallbacks.len());
    chain.push(FontSource::from(primary));
    for fallback in fallbacks {
        chain.push(FontSource::from(fallback.clone()));
    }
    FontSource::list(chain)
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
        // Reactive desktop pacing: ticks at 8 Hz while focused for smooth cursor/text blinking
        // and sleeps while unfocused/occluded so Wayland compositors do not report swap-chain
        // acquisition timeouts.
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::reactive(Duration::from_millis(125)),
            unfocused_mode: UpdateMode::reactive_low_power(Duration::from_secs(60)),
        })
        .init_resource::<DemoState>()
        // `RatatuiContext` is inserted during `Startup`, so configure initial fonts in `PostStartup`.
        .add_systems(PostStartup, setup_fonts)
        .add_systems(Update, (handle_input, draw_showcase).chain())
        .run();
}

/// Loads a distinct monospace family for each import method (dynamic `AssetServer`, embedded
/// `include_bytes!`, single-face fallback, and Bevy's built-in default font), pairing each
/// custom preset with bundled fallback chains via `FontSource::list`.
fn setup_fonts(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut fonts: ResMut<Assets<Font>>,
    state: Res<DemoState>,
    mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>,
) {
    let dejavu_dyn: Handle<Font> = asset_server.load("fonts/dejavu-sans-mono/DejaVuSansMono.ttf");
    let cascadia_dyn: Handle<Font> =
        asset_server.load("fonts/cascadia-mono/CascadiaMono-Regular.ttf");
    let cjk_dyn: Handle<Font> = asset_server.load("fonts/fidelity/FidelityCJK.otf");
    let emoji_dyn: Handle<Font> = asset_server.load("fonts/fidelity/FidelityColorEmoji.ttf");
    let nerd_dyn: Handle<Font> = asset_server.load("fonts/fidelity/FidelityNerdSymbols.ttf");

    let dynamic_fallbacks = [
        dejavu_dyn.clone(),
        cascadia_dyn.clone(),
        cjk_dyn.clone(),
        emoji_dyn.clone(),
        nerd_dyn.clone(),
    ];

    let dynamic_jetbrains = FontFaces {
        regular: with_fallbacks(
            asset_server.load("fonts/jetbrains-mono/JetBrainsMono-Regular.ttf"),
            &dynamic_fallbacks,
        ),
        bold: Some(with_fallbacks(
            asset_server.load("fonts/jetbrains-mono/JetBrainsMono-Bold.ttf"),
            &dynamic_fallbacks,
        )),
        italic: Some(with_fallbacks(
            asset_server.load("fonts/jetbrains-mono/JetBrainsMono-Italic.ttf"),
            &dynamic_fallbacks,
        )),
        bold_italic: Some(with_fallbacks(
            asset_server.load("fonts/jetbrains-mono/JetBrainsMono-BoldItalic.ttf"),
            &dynamic_fallbacks,
        )),
        synthesize: true,
    };

    let embedded_fallbacks = [
        fonts.add(Font::from_bytes(EMBEDDED_FALLBACK_DEJAVU.to_vec())),
        fonts.add(Font::from_bytes(EMBEDDED_FALLBACK_CASCADIA.to_vec())),
        fonts.add(Font::from_bytes(EMBEDDED_FALLBACK_CJK.to_vec())),
        fonts.add(Font::from_bytes(EMBEDDED_FALLBACK_EMOJI.to_vec())),
        fonts.add(Font::from_bytes(EMBEDDED_FALLBACK_NERD.to_vec())),
    ];

    let embedded_hack = FontFaces {
        regular: with_fallbacks(
            fonts.add(Font::from_bytes(EMBEDDED_HACK_REGULAR.to_vec())),
            &embedded_fallbacks,
        ),
        bold: Some(with_fallbacks(
            fonts.add(Font::from_bytes(EMBEDDED_HACK_BOLD.to_vec())),
            &embedded_fallbacks,
        )),
        italic: Some(with_fallbacks(
            fonts.add(Font::from_bytes(EMBEDDED_HACK_ITALIC.to_vec())),
            &embedded_fallbacks,
        )),
        bold_italic: Some(with_fallbacks(
            fonts.add(Font::from_bytes(EMBEDDED_HACK_BOLD_ITALIC.to_vec())),
            &embedded_fallbacks,
        )),
        synthesize: true,
    };

    let dejavu_fallbacks = [
        cascadia_dyn.clone(),
        cjk_dyn.clone(),
        emoji_dyn.clone(),
        nerd_dyn.clone(),
    ];
    let dejavu_source = with_fallbacks(dejavu_dyn.clone(), &dejavu_fallbacks);
    let dejavu_synthesized = FontFaces {
        regular: dejavu_source.clone(),
        bold: None,
        italic: None,
        bold_italic: None,
        synthesize: true,
    };
    let dejavu_exact = FontFaces {
        regular: dejavu_source,
        bold: None,
        italic: None,
        bold_italic: None,
        synthesize: false,
    };

    let cascadia_fallbacks = [dejavu_dyn, cjk_dyn, emoji_dyn, nerd_dyn];
    let cascadia_single = FontFaces {
        regular: with_fallbacks(cascadia_dyn, &cascadia_fallbacks),
        bold: None,
        italic: None,
        bold_italic: None,
        synthesize: true,
    };

    let bevy_default = FontFaces::regular(Handle::<Font>::default());

    let library = FontLibrary {
        dynamic_jetbrains,
        embedded_hack,
        dejavu_synthesized,
        dejavu_exact,
        cascadia_single,
        bevy_default,
    };

    config.font = library.faces(state.preset);
    config.sizing = TerminalSizing::font(state.font_size);
    config.cursor.style = state.cursor_style;

    commands.insert_resource(library);
}

fn handle_input(
    mut key_events: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    library: Option<Res<FontLibrary>>,
    mut state: ResMut<DemoState>,
    mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>,
    mut exit: MessageWriter<AppExit>,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let page = i32::from(state.page_height.max(1));
    let mut delta: i32 = wheel
        .read()
        .map(|event| (-event.y.round() as i32).clamp(-8, 8))
        .sum();

    for event in key_events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        match &event.logical_key {
            Key::Escape => {
                exit.write(AppExit::Success);
                return;
            }
            Key::Character(ch) if ch.eq_ignore_ascii_case("q") => {
                exit.write(AppExit::Success);
                return;
            }
            Key::Tab => {
                if let Some(library) = library.as_ref() {
                    state.preset = if shift {
                        state.preset.prev()
                    } else {
                        state.preset.next()
                    };
                    config.font = library.faces(state.preset);
                }
            }
            Key::Character(ch) if ch.eq_ignore_ascii_case("f") => {
                if let Some(library) = library.as_ref() {
                    state.preset = state.preset.next();
                    config.font = library.faces(state.preset);
                }
            }
            Key::Character(ch) if ch.as_str() == "+" || ch.as_str() == "=" => {
                state.font_size = (state.font_size + FONT_STEP).min(MAX_FONT_SIZE);
                config.sizing = TerminalSizing::font(state.font_size);
            }
            Key::Character(ch) if ch.as_str() == "-" || ch.as_str() == "_" => {
                state.font_size = (state.font_size - FONT_STEP).max(MIN_FONT_SIZE);
                config.sizing = TerminalSizing::font(state.font_size);
            }
            Key::Character(ch) if ch.eq_ignore_ascii_case("c") => {
                state.cursor_style = match state.cursor_style {
                    CursorStyle::Block => CursorStyle::Bar,
                    CursorStyle::Bar => CursorStyle::Underline,
                    CursorStyle::Underline => CursorStyle::Block,
                };
                config.cursor.style = state.cursor_style;
            }
            Key::ArrowDown => delta += 1,
            Key::Character(ch) if ch.eq_ignore_ascii_case("j") => delta += 1,
            Key::ArrowUp => delta -= 1,
            Key::Character(ch) if ch.eq_ignore_ascii_case("k") => delta -= 1,
            Key::PageDown => delta += page,
            Key::PageUp => delta -= page,
            Key::End => delta = i32::from(u16::MAX),
            Key::Home => delta = -i32::from(u16::MAX),
            _ => {}
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
        " 9. Unicode blocks & scripts (wide cells occupy 2 cols without overlapping '|' guards; bundled fallbacks via FontSource::list) ",
    ));
    for text in [
        "|Latin-1  |ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖØÙÚÛÜÝÞß| |café naïve résumé über Ångström Łódź ğşİı|",
        "|Greek    |ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩ| |αβγδεζηθικλμνξοπρστυφχψω|",
        "|Cyrillic |АБВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ| |привет мир ёж|",
        "|Arrows   |← ↑ → ↓ ↔ ↕ ↖ ↗ ↘ ↙| |⇐ ⇑ ⇒ ⇓ ⇔ ↺ ↻ ↵ ↩|",
        "|Math/Tech|± × ÷ ≠ ≤ ≥ ≈ ≡ ∞ ∑ ∏ √ ∫ ∂ ∆ ∇ ∈ ∉ ∩ ∪ ⊂ ⊃ ∀ ∃ ∧ ∨ ¬| |⌈ ⌉ ⌊ ⌋ ⊕ ⊗ ⊥ ⊢|",
        "|Punct/Sym|© ® ™ ° µ ¶ § † ‡ • ◦ … – — « » ‹ › “ ” ‘ ’| |€ £ ¥ ¢ ₿| |☐ ☑ ☒ ♠ ♣ ♥ ♦ ★ ☆ ♪ ♫|",
        "|Combining|e\u{301}|A\u{30a}|n\u{303}|o\u{308}\u{304}|Z\u{330}\u{301}|  precomposed: |é|Å|ñ|  (must stay 1 cell each)",
        "|Wide CJK |汉字|日本語|한글|  |ｆｕｌｌｗｉｄｔｈ|  guards: |0123456789|",
        "|Mixed    |a汉b字c日d本e語f|  |narrow|wide|narrow|  the guards | must | stay | aligned |",
        "|Emoji/RTL|🙂|🚀|🎉|❤️|☕️|👍🏽|🇺🇸|👩‍💻|🏳️‍🌈|  |שלום עולם|مرحبا بالعالم|  Nerd: |☰||||",
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
        ("日本語", Style::new().add_modifier(Modifier::CROSSED_OUT)),
        ("🎉☕️", Style::new().add_modifier(Modifier::DIM)),
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
