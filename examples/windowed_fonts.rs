//! Ways to give the windowed terminal its fonts, using bundled monospace fonts (no system font
//! discovery; see the licenses under `assets/fonts/`).
//!
//! Each preset loads its fonts a different way:
//! - **JetBrains Mono** (4 faces): loaded by the `AssetServer`, with a `FontSource::list` of
//!   fallback fonts for symbols, RTL scripts, CJK, color emoji and Nerd Fonts icons
//! - **Hack** (4 faces): embedded with `include_bytes!` and `Font::from_bytes`, so it needs no
//!   asset files at run time, and given no fallback list
//! - **DejaVu Sans Mono** (regular only): one face for every style, with the fallbacks
//! - **Bevy's default font** (an ASCII subset of Fira Mono): what a terminal uses unconfigured
//!
//! A preset without a fallback list may still show scripts its font lacks: Bevy can find them
//! in other fonts already loaded, here the other presets' fallbacks. Only a list says which
//! fonts are tried, and in which order.
//!
//! The renderer's own test scene (every modifier combination, the color cube, box drawing) is
//! `bevy_terminal`'s `render_test` example. For OS font discovery (`FontSource::monospace()`,
//! named system families, and automatic OS fallback), see `examples/windowed_system_fonts.rs`.
//!
//! Run with:
//! ```sh
//! cargo run --example windowed_fonts --features windowed
//! ```
//!
//! Keys:
//! - `Tab` / `F` (`Shift+Tab` back): cycle font presets
//! - `+` / `-`: increase or decrease font size (the grid refills the window)
//! - `C`: cycle cursor style (`Block`, `Bar`, `Underline`)
//! - `↑`/`↓` or `j`/`k`, `PageUp`/`PageDown`, `Home`/`End`, mouse wheel: scroll
//! - `Q` / `Esc`: quit

use bevy::{
    app::AppExit,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
        mouse::{MouseScrollUnit, MouseWheel},
    },
    prelude::*,
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

// The embedded preset: the bytes are in the binary, so it needs no asset files at run time.
const HACK_REGULAR: &[u8] = include_bytes!("../assets/fonts/hack/Hack-Regular.ttf");
const HACK_BOLD: &[u8] = include_bytes!("../assets/fonts/hack/Hack-Bold.ttf");
const HACK_ITALIC: &[u8] = include_bytes!("../assets/fonts/hack/Hack-Italic.ttf");
const HACK_BOLD_ITALIC: &[u8] = include_bytes!("../assets/fonts/hack/Hack-BoldItalic.ttf");

const MIN_FONT_SIZE: f32 = 10.0;
const MAX_FONT_SIZE: f32 = 48.0;
const FONT_STEP: f32 = 2.0;

const FACES: [(&str, Modifier); 4] = [
    ("Regular", Modifier::empty()),
    ("Bold", Modifier::BOLD),
    ("Italic", Modifier::ITALIC),
    ("Bold+Italic", Modifier::BOLD.union(Modifier::ITALIC)),
];

/// The presets, in the order `Tab` cycles them, with the label the header shows.
const PRESETS: [&str; 4] = [
    "JetBrains Mono (4 faces, AssetServer + FontSource::list fallbacks)",
    "Hack (4 faces, embedded with include_bytes!, no fallback list)",
    "DejaVu Sans Mono (regular only, AssetServer + fallbacks)",
    "Bevy default font (Fira Mono ASCII subset, no fallback list)",
];

/// The fonts of each preset, loaded once.
#[derive(Resource)]
struct FontLibrary([FontFaces; PRESETS.len()]);

#[derive(Resource)]
struct DemoState {
    /// An index into [`PRESETS`].
    preset: usize,
    font_size: f32,
    cursor_style: CursorStyle,
    scroll: u16,
    /// Lines the wheel has turned that are not scrolled yet: less than one.
    wheel_lines: f32,
    page_height: u16,
}

impl Default for DemoState {
    fn default() -> Self {
        Self {
            preset: 0,
            font_size: 16.0,
            cursor_style: CursorStyle::Block,
            scroll: 0,
            wheel_lines: 0.0,
            page_height: 24,
        }
    }
}

/// The lines shown, which never change, and the line the cursor sits on.
#[derive(Resource)]
struct Showcase {
    lines: Vec<Line<'static>>,
    cursor_line: u16,
}

/// A font followed by the fonts that supply the symbols it lacks.
fn with_fallbacks(primary: Handle<Font>, fallbacks: &[Handle<Font>]) -> FontSource {
    let fonts = std::iter::once(primary).chain(fallbacks.iter().cloned());
    FontSource::list(fonts.map(FontSource::from).collect::<Vec<_>>())
}

fn main() -> AppExit {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "bevy_ratatui · windowed fonts".into(),
                    resolution: (1440, 960).into(),
                    ..default()
                }),
                ..default()
            }),
            RatatuiPlugins::default(),
        ))
        .init_resource::<DemoState>()
        .insert_resource(showcase())
        // `RatatuiContext` is inserted during `Startup`, so configure initial fonts in `PostStartup`.
        .add_systems(PostStartup, setup_fonts)
        .add_systems(Update, (handle_input, draw_showcase).chain())
        .run()
}

/// Loads each preset's fonts and applies the first.
fn setup_fonts(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut fonts: ResMut<Assets<Font>>,
    state: Res<DemoState>,
    mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>,
) {
    let dejavu: Handle<Font> = asset_server.load("fonts/dejavu-sans-mono/DejaVuSansMono.ttf");
    // Tried in order for a symbol the primary font lacks.
    let fallbacks = [
        dejavu.clone(),
        asset_server.load("fonts/cascadia-mono/CascadiaMono-Regular.ttf"),
        asset_server.load("fonts/fidelity/FidelityCJK.otf"),
        asset_server.load("fonts/fidelity/FidelityColorEmoji.ttf"),
        asset_server.load("fonts/fidelity/FidelityNerdSymbols.ttf"),
    ];
    let jetbrains = |face: &str| {
        let path = format!("fonts/jetbrains-mono/JetBrainsMono-{face}.ttf");
        with_fallbacks(asset_server.load(path), &fallbacks)
    };
    let jetbrains = FontFaces {
        regular: jetbrains("Regular"),
        bold: Some(jetbrains("Bold")),
        italic: Some(jetbrains("Italic")),
        bold_italic: Some(jetbrains("BoldItalic")),
        synthesize: true,
    };

    let mut hack = |bytes: &[u8]| FontSource::from(fonts.add(Font::from_bytes(bytes.to_vec())));
    let hack = FontFaces {
        regular: hack(HACK_REGULAR),
        bold: Some(hack(HACK_BOLD)),
        italic: Some(hack(HACK_ITALIC)),
        bold_italic: Some(hack(HACK_BOLD_ITALIC)),
        synthesize: true,
    };

    // One face: bold and italic text is drawn with it too.
    let dejavu = FontFaces::regular(with_fallbacks(dejavu, &fallbacks[1..]));

    // What `TerminalRenderConfig::default()` already has.
    let bevy_default = FontFaces::default();

    let library = FontLibrary([jetbrains, hack, dejavu, bevy_default]);
    config.font = library.0[state.preset].clone();
    config.sizing = TerminalSizing::font(state.font_size);
    config.cursor.style = state.cursor_style;
    commands.insert_resource(library);
}

fn handle_input(
    mut key_events: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    library: Res<FontLibrary>,
    mut state: ResMut<DemoState>,
    mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>,
    mut exit: MessageWriter<AppExit>,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let page = i32::from(state.page_height.max(1));

    // A wheel reports lines and a touchpad pixels, often a fraction of a line at a time: the
    // fraction is kept for the next event.
    for event in wheel.read() {
        state.wheel_lines -= match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y / state.font_size,
        };
    }
    let mut delta = state.wheel_lines.trunc() as i32;
    if delta != 0 {
        state.wheel_lines -= delta as f32;
    }

    let mut preset = state.preset;
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
            Key::Tab if shift => preset = (preset + PRESETS.len() - 1) % PRESETS.len(),
            Key::Tab => preset = (preset + 1) % PRESETS.len(),
            Key::Character(ch) if ch.eq_ignore_ascii_case("f") => {
                preset = (preset + 1) % PRESETS.len();
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
                    _ => CursorStyle::Block,
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

    if preset != state.preset
        && let Some(faces) = library.0.get(preset)
    {
        state.preset = preset;
        config.font = faces.clone();
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

/// The lines of the showcase, built once: nothing in them depends on the font chosen.
fn showcase() -> Showcase {
    let mut lines: Vec<Line<'static>> = Vec::new();

    lines.push(heading(
        " 1. Faces: four distinct ones where the preset has four, one where it has one ",
    ));
    const ASCII: &str = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
    for (name, modifier) in FACES {
        lines.push(Line::from(vec![
            label(&format!("{name:<12}")),
            Span::styled(ASCII, Style::new().add_modifier(modifier)),
        ]));
    }
    lines.push(Line::raw(""));

    lines.push(heading(
        " 2. Drawn by the renderer, whatever the font: box drawing, blocks, shades, braille ",
    ));
    for text in [
        "┌─┬─┐ ┏━┳━┓ ╔═╦═╗ ╭─┬─╮   █▉▊▋▌▍▎▏ ▁▂▃▄▅▆▇█ ░▒▓█   ⠁⠃⠇⡇⣇⣧⣷⣿",
        "├─┼─┤ ┣━╋━┫ ╠═╬═╣ ├─┼─┤   ▖▗▘▙▚▛▜▝▞▟ ▀▄ ▐▌",
        "└─┴─┘ ┗━┻━┛ ╚═╩═╝ ╰─┴─╯",
        "",
    ] {
        lines.push(Line::from(text));
    }

    lines.push(heading(
        " 3. Symbols and scripts from the fallback fonts; wide cells keep the | guards aligned ",
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

    lines.push(Line::raw(""));

    lines.push(heading(
        " 4. Blinking cursor (press C to cycle Block / Bar / Underline) ",
    ));
    let cursor_line = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    lines.push(Line::from("cursor here > "));

    Showcase { lines, cursor_line }
}

fn draw_showcase(
    mut state: ResMut<DemoState>,
    showcase: Res<Showcase>,
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

    let total_lines = u16::try_from(showcase.lines.len()).unwrap_or(u16::MAX);

    context.draw(|frame| {
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .areas(frame.area());

        let max_scroll = total_lines.saturating_sub(body.height);
        // Written only on a change, so that the state is not marked changed by every draw.
        if state.page_height != body.height || state.scroll > max_scroll {
            state.page_height = body.height;
            state.scroll = state.scroll.min(max_scroll);
        }

        let header_block = Block::bordered()
            .title(" bevy_ratatui · Windowed Fonts ")
            .border_style(Style::new().fg(Color::Cyan));
        let header_text = vec![
            Line::from(vec![
                Span::styled("Font: ", Style::new().fg(Color::DarkGray)),
                Span::styled(
                    PRESETS.get(state.preset).copied().unwrap_or_default(),
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

        let visible = showcase.lines.iter().skip(state.scroll.into());
        for (line, row) in visible.zip(body.rows()) {
            frame.render_widget(line, row);
        }

        if let Some(relative_row) = showcase.cursor_line.checked_sub(state.scroll)
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
