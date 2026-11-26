//! Opt-in OS system font discovery showcase for the windowed backend.
//!
//! Unlike `windowed_fonts` (which bundles font files and needs no system fonts),
//! this example enables `bevy/system_font_discovery` directly via Cargo's additive features so
//! the OS font catalog resolves generic monospace families, named installed monospace families,
//! and automatic cross-script fallbacks (CJK, Emoji, RTL, Indic, Thai) with no bundled font
//! assets required.
//!
//! In a downstream crate, enable `features = ["system_font_discovery"]` on your own `bevy`
//! dependency alongside `bevy_ratatui = { features = ["windowed"] }`.
//!
//! Run with:
//! ```sh
//! cargo run --example windowed_system_fonts --features windowed,bevy/system_font_discovery
//! ```
//!
//! Keys:
//! - `Tab` / `F` (`Shift+Tab` back): cycle system monospace font presets
//! - `+` / `-`: increase or decrease font size (grid refills the window automatically)
//! - `C`: cycle cursor style (`Block`, `Bar`, `Underline`)
//! - `Q` / `Esc`: quit

use bevy::{
    app::AppExit,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
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

const MIN_FONT_SIZE: f32 = 10.0;
const MAX_FONT_SIZE: f32 = 48.0;
const FONT_STEP: f32 = 2.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SystemFontPreset {
    #[default]
    GenericMonospace,
    UiMonospace,
    DeveloperStack,
    ClassicStack,
    MonospaceExact,
}

impl SystemFontPreset {
    fn next(self) -> Self {
        match self {
            Self::GenericMonospace => Self::UiMonospace,
            Self::UiMonospace => Self::DeveloperStack,
            Self::DeveloperStack => Self::ClassicStack,
            Self::ClassicStack => Self::MonospaceExact,
            Self::MonospaceExact => Self::GenericMonospace,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::GenericMonospace => Self::MonospaceExact,
            Self::UiMonospace => Self::GenericMonospace,
            Self::DeveloperStack => Self::UiMonospace,
            Self::ClassicStack => Self::DeveloperStack,
            Self::MonospaceExact => Self::ClassicStack,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::GenericMonospace => "FontSource::monospace() (OS default monospace)",
            Self::UiMonospace => "FontSource::ui_monospace() (OS UI monospace)",
            Self::DeveloperStack => {
                "FontSource::families(\"JetBrains Mono, Fira Code, Hack, Cascadia Mono, Menlo, Consolas, monospace\")"
            }
            Self::ClassicStack => {
                "FontSource::families(\"DejaVu Sans Mono, Liberation Mono, SF Mono, Courier New, monospace\")"
            }
            Self::MonospaceExact => {
                "FontSource::monospace() with synthesize: false (OS faces only)"
            }
        }
    }

    fn faces(self) -> FontFaces {
        match self {
            Self::GenericMonospace => FontFaces::regular(FontSource::monospace()),
            Self::UiMonospace => FontFaces::regular(FontSource::ui_monospace()),
            Self::DeveloperStack => FontFaces::regular(FontSource::families(
                "JetBrains Mono, Fira Code, Hack, Cascadia Mono, Menlo, Consolas, monospace",
            )),
            Self::ClassicStack => FontFaces::regular(FontSource::families(
                "DejaVu Sans Mono, Liberation Mono, SF Mono, Courier New, monospace",
            )),
            Self::MonospaceExact => FontFaces {
                regular: FontSource::monospace(),
                bold: None,
                italic: None,
                bold_italic: None,
                synthesize: false,
            },
        }
    }
}

#[derive(Resource)]
struct DemoState {
    preset: SystemFontPreset,
    font_size: f32,
    cursor_style: CursorStyle,
}

impl Default for DemoState {
    fn default() -> Self {
        Self {
            preset: SystemFontPreset::GenericMonospace,
            font_size: 18.0,
            cursor_style: CursorStyle::Block,
        }
    }
}

fn main() -> AppExit {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "bevy_ratatui · windowed system font discovery".into(),
                    resolution: (1280, 820).into(),
                    ..default()
                }),
                ..default()
            }),
            RatatuiPlugins::default(),
        ))
        .init_resource::<DemoState>()
        .add_systems(Startup, configure_initial_font)
        .add_systems(Update, (handle_input, draw_demo).chain())
        .run()
}

fn configure_initial_font(
    state: Res<DemoState>,
    mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>,
) {
    config.font = state.preset.faces();
    config.sizing = TerminalSizing::font(state.font_size);
    config.cursor.style = state.cursor_style;
}

fn handle_input(
    mut key_events: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<DemoState>,
    mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>,
    mut exit: MessageWriter<AppExit>,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

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
                state.preset = if shift {
                    state.preset.prev()
                } else {
                    state.preset.next()
                };
                config.font = state.preset.faces();
            }
            Key::Character(ch) if ch.eq_ignore_ascii_case("f") => {
                state.preset = state.preset.next();
                config.font = state.preset.faces();
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
            _ => {}
        }
    }
}

fn draw_demo(
    state: Res<DemoState>,
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

    context.draw(|frame| {
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .areas(frame.area());

        let header_block = Block::bordered()
            .title(" bevy_ratatui · System Font Discovery (`--features windowed,bevy/system_font_discovery`) ")
            .border_style(Style::new().fg(Color::Cyan));
        let header_lines = vec![
            Line::from(vec![
                Span::styled("Source: ", Style::new().fg(Color::DarkGray)),
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
                Span::styled("  ·  ", Style::new().fg(Color::DarkGray)),
                Span::styled(metrics, Style::new().fg(Color::Green)),
            ]),
        ];
        frame.render_widget(Paragraph::new(header_lines).block(header_block), header);

        let heading = |text: &'static str| {
            Line::from(Span::styled(
                text,
                Style::new()
                    .fg(Color::Black)
                    .bg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ))
        };

        let mut lines = vec![
            heading(" 1. OS-discovered monospace faces (zero bundled font files) "),
            Line::from(vec![
                Span::raw("Regular:     The quick brown fox jumps over 0123456789 != => ->"),
            ]),
            Line::from(Span::styled(
                "Bold:        The quick brown fox jumps over 0123456789 != => ->",
                Style::new().add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Italic:      The quick brown fox jumps over 0123456789 != => ->",
                Style::new().add_modifier(Modifier::ITALIC),
            )),
            Line::from(Span::styled(
                "Bold+Italic: The quick brown fox jumps over 0123456789 != => ->",
                Style::new().add_modifier(Modifier::BOLD | Modifier::ITALIC),
            )),
            Line::raw(""),
            heading(" 2. Automatic OS script fallback (coverage depends on installed host fonts) "),
            Line::from("|Latin/Ext|ÀÁÂÃÄÅÆÇÈÉÊË café naïve résumé Ångström Łódź ğşİı|"),
            Line::from("|Greek/Cyr|ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩ | АБВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩ|"),
            Line::from("|Symbols  |← ↑ → ↓ ↔ ⇐ ⇒ ⇔ ↺ ↻ | ± × ÷ ≠ ≤ ≥ ≈ ∞ ∑ √ ∫ ∈ ∉ ∩ ∪ ∀ ∃ | ♠ ♣ ♥ ♦ ★ ☆ ♪ ♫|"),
            Line::from("|Wide CJK |汉字|日本語|한글|中文测试|  |ｆｕｌｌｗｉｄｔｈ|  guards: |0123456789|"),
            Line::from("|Mixed    |a汉b字c日d本e語f|  |narrow|wide|narrow|  the guards | must | stay | aligned |"),
            Line::from("|Emoji    |🙂|🚀|🎉|❤️|✅|⚠️|👍🏽|🇺🇸|👩‍💻|🏳️‍🌈|  (color emoji via host OS fallback)"),
            Line::from("|RTL/Indic|שלום עולם|مرحبا بالعالم|  |नमस्ते|தமிழ்|ไทย|"),
            Line::raw(""),
            heading(" 3. Procedural box drawing & blocks (always sharp regardless of system font) "),
            Line::from("┌─┬─┐ ┏━┳━┓ ╔═╦═╗ ╭─┬─╮   █▉▊▋▌▍▎▏ ▁▂▃▄▅▆▇█ ░▒▓█   ⠁⠃⠇⡇⣇⣧⣷⣿"),
            Line::from("├─┼─┤ ┣━╋━┫ ╠═╬═╣ ├─┼─┤   ▖▗▘▙▚▛▜▝▞▟  ◢◣◤◥ ▲▼◀▶ ●○◆◇"),
            Line::from("└─┴─┘ ┗━┻━┛ ╚═╩═╝ ╰─┴─╯"),
            Line::raw(""),
        ];
        let cursor_row = u16::try_from(lines.len()).unwrap_or(u16::MAX);
        lines.push(Line::from("cursor here > "));

        let body_block = Block::bordered()
            .title(" Live System Font Preview ")
            .border_style(Style::new().fg(Color::DarkGray));
        let body_inner = body_block.inner(body);
        // Not wrapped, so that the cursor's row is the line's index.
        frame.render_widget(Paragraph::new(lines).block(body_block), body);

        if cursor_row < body_inner.height && body_inner.width > 14 {
            frame.set_cursor_position(Position::new(
                body_inner.x.saturating_add(14),
                body_inner.y.saturating_add(cursor_row),
            ));
        }

        let footer_block = Block::bordered().border_style(Style::new().fg(Color::DarkGray));
        let controls = Line::from(vec![
            Span::styled(" Tab/F ", Style::new().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" switch system font   "),
            Span::styled(" +/- ", Style::new().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" font size   "),
            Span::styled(" C ", Style::new().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" cursor   "),
            Span::styled(" Q/Esc ", Style::new().fg(Color::Black).bg(Color::Red)),
            Span::raw(" quit"),
        ]);
        frame.render_widget(Paragraph::new(controls).block(footer_block), footer);
    })?;

    Ok(())
}
