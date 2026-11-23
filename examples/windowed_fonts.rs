//! Demonstrates windowed custom font loading (regular, bold, italic, and bold-italic) and live
//! runtime swapping using JetBrains Mono (SIL OFL 1.1, see `assets/fonts/jetbrains-mono/OFL.txt`).
//!
//! Run with:
//! ```sh
//! cargo run --example windowed_fonts --features windowed
//! ```
//!
//! Keys:
//! - `Tab` / `F`: cycle font presets (dynamic 4-face, embedded 4-face, regular-only, built-in default)
//! - `+` / `-`: increase or decrease font size (grid refills the window automatically)
//! - `C`: cycle cursor style (`Block`, `Bar`, `Underline`)
//! - `Q` / `Esc`: quit

use bevy::{app::AppExit, prelude::*};
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
    widgets::{Block, Borders, Gauge, Paragraph},
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
}

impl Default for DemoState {
    fn default() -> Self {
        Self {
            preset: FontPreset::DynamicFourFace,
            font_size: 20.0,
            cursor_style: CursorStyle::Block,
        }
    }
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, RatatuiPlugins::default()))
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
    library: Option<Res<FontLibrary>>,
    mut state: ResMut<DemoState>,
    mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyQ) || keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
        return;
    }

    let Some(library) = library else {
        return;
    };

    if keys.just_pressed(KeyCode::Tab) || keys.just_pressed(KeyCode::KeyF) {
        state.preset = state.preset.next();
        config.font = library.faces(state.preset);
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
}

fn draw_showcase(
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
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .areas(frame.area());

        let [styles_area, unicode_area] =
            Layout::horizontal([Constraint::Percentage(52), Constraint::Percentage(48)])
                .areas(body);

        let header_block = Block::bordered()
            .title(" bevy_ratatui · Windowed Fonts Showcase ")
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
                Span::styled("  ·  ", Style::new().fg(Color::DarkGray)),
                Span::styled(metrics, Style::new().fg(Color::Green)),
            ]),
        ];
        frame.render_widget(Paragraph::new(header_text).block(header_block), header);

        let style_lines = vec![
            Line::from("Regular:     The quick brown fox jumps over 0123456789."),
            Line::styled(
                "Bold:        The quick brown fox jumps over 0123456789.",
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Line::styled(
                "Italic:      The quick brown fox jumps over 0123456789.",
                Style::new().add_modifier(Modifier::ITALIC),
            ),
            Line::styled(
                "Bold Italic: The quick brown fox jumps over 0123456789.",
                Style::new().add_modifier(Modifier::BOLD | Modifier::ITALIC),
            ),
            Line::default(),
            Line::from(vec![
                Span::styled("Dim ", Style::new().add_modifier(Modifier::DIM)),
                Span::styled(
                    "Underlined ",
                    Style::new().add_modifier(Modifier::UNDERLINED),
                ),
                Span::styled("Crossed ", Style::new().add_modifier(Modifier::CROSSED_OUT)),
                Span::styled("Reversed ", Style::new().add_modifier(Modifier::REVERSED)),
            ]),
            Line::from(vec![
                Span::styled(
                    "Red ",
                    Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Green ",
                    Style::new().fg(Color::Green).add_modifier(Modifier::ITALIC),
                ),
                Span::styled(
                    "Blue ",
                    Style::new()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::BOLD | Modifier::ITALIC),
                ),
                Span::styled(
                    "Magenta",
                    Style::new().fg(Color::Magenta).bg(Color::DarkGray),
                ),
            ]),
            Line::default(),
            Line::from("Cursor preview: [ ]  (press C to cycle style)"),
        ];
        let styles_widget = Paragraph::new(style_lines).block(
            Block::default()
                .title(" Faces & Text Modifiers ")
                .borders(Borders::ALL),
        );
        frame.render_widget(styles_widget, styles_area);

        // Place the cursor inside the `[ ]` slot when the panel is tall enough.
        if styles_area.height > 10 && styles_area.width > 20 {
            frame.set_cursor_position(Position::new(styles_area.x + 18, styles_area.y + 9));
        }

        let [unicode_top, unicode_gauge] =
            Layout::vertical([Constraint::Min(7), Constraint::Length(3)]).areas(unicode_area);

        let unicode_lines = vec![
            Line::styled(
                "Glyphs from font (missing in ASCII default):",
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Line::from("  Arrows:  ↑ ↓ ← → ↔ ↕ ⇒ ⇔"),
            Line::from("  Bullets: • ● ○ · ▪ ▫ ◆ ◇"),
            Line::from("  Latin-1: café naïve résumé über Ångström"),
            Line::from("  Math:    ± × ÷ ≠ ≤ ≥ ≈ ∞ ∑ √"),
            Line::default(),
            Line::styled(
                "Procedural sprites (always crisp):",
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Line::from("  Blocks:  ▁▂▃▄▅▆▇█  Shades: ░▒▓█  Braille: ⣿⣷⣶⣤⣀"),
        ];
        frame.render_widget(
            Paragraph::new(unicode_lines).block(
                Block::default()
                    .title(" Unicode vs. Procedural Sprites ")
                    .borders(Borders::ALL),
            ),
            unicode_top,
        );

        let ratio =
            ((state.font_size - MIN_FONT_SIZE) / (MAX_FONT_SIZE - MIN_FONT_SIZE)).clamp(0.0, 1.0);
        let gauge = Gauge::default()
            .block(Block::bordered().title(" Font Size "))
            .gauge_style(Style::new().fg(Color::Cyan).bg(Color::DarkGray))
            .ratio(f64::from(ratio))
            .label(format!("{:.0} px", state.font_size));
        frame.render_widget(gauge, unicode_gauge);

        let footer_text = Line::from(vec![
            Span::styled("[Tab/F]", Style::new().fg(Color::Yellow)),
            Span::raw(" Switch font   "),
            Span::styled("[+/-]", Style::new().fg(Color::Yellow)),
            Span::raw(" Font size   "),
            Span::styled("[C]", Style::new().fg(Color::Yellow)),
            Span::raw(" Cursor style   "),
            Span::styled("[Q/Esc]", Style::new().fg(Color::Yellow)),
            Span::raw(" Quit"),
        ]);
        frame.render_widget(Paragraph::new(footer_text).block(Block::bordered()), footer);
    })?;

    Ok(())
}
