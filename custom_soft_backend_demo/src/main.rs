//! Demonstrates using a custom soft_ratatui backend with bevy_ratatui.

use bevy::{app::AppExit, prelude::*};
use bevy_ratatui::{
    RatatuiContext, RatatuiPlugins,
    context::TerminalContext,
    windowed::SoftTerminalContext,
};
use ratatui::{
    Terminal,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
};
use soft_ratatui::embedded_graphics_unicodefonts::{
    mono_7x13_atlas, mono_7x13_bold_atlas, mono_7x13_italic_atlas,
};
use soft_ratatui::{EmbeddedGraphics, SoftBackend};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(ImagePlugin::default_nearest()),
            RatatuiPlugins::default().in_context::<EmbeddedWindowedContext>(),
        ))
        .add_systems(Update, (draw_system, exit_system))
        .run();
}

#[derive(Deref, DerefMut)]
struct EmbeddedWindowedContext(Terminal<SoftBackend<EmbeddedGraphics>>);

impl TerminalContext for EmbeddedWindowedContext {
    type Backend = SoftBackend<EmbeddedGraphics>;

    fn init() -> Result<Self> {
        let backend = SoftBackend::<EmbeddedGraphics>::new(
            72,
            24,
            mono_7x13_atlas(),
            Some(mono_7x13_bold_atlas()),
            Some(mono_7x13_italic_atlas()),
        );

        Ok(Self(Terminal::new(backend)?))
    }

    fn restore() -> Result<()> {
        Ok(())
    }

    fn configure_plugin_group(
        _group: &RatatuiPlugins,
        builder: bevy::app::PluginGroupBuilder,
    ) -> bevy::app::PluginGroupBuilder {
        Self::configure_windowed_plugin_group(builder)
    }
}

fn draw_system(mut context: ResMut<RatatuiContext<EmbeddedWindowedContext>>) -> Result {
    context.draw(|frame| {
        let [header, body] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(frame.area());

        let header_text = Paragraph::new(Line::from("Custom Soft Backend Demo").centered())
            .block(Block::default().borders(Borders::BOTTOM))
            .style(
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(header_text, header);

        let body_text = Text::from(vec![
            Line::from("This app does not use bevy_ratatui::WindowedContext."),
            Line::from(""),
            Line::from("It defines its own TerminalContext around:"),
            Line::from("Terminal<SoftBackend<EmbeddedGraphics>>")
                .style(Style::default().fg(Color::LightYellow)),
            Line::from(""),
            Line::from("The custom context chooses:"),
            Line::from("- embedded unicodefont atlases"),
            Line::from("- mono_7x13 regular/bold/italic"),
            Line::from("- 72x24 terminal cells"),
            Line::from("- no external font files"),
            Line::from(""),
            Line::from("Press Q or Escape to quit.").style(Style::default().fg(Color::LightGreen)),
        ]);

        frame.render_widget(
            Paragraph::new(body_text).block(Block::default().borders(Borders::ALL)),
            body,
        );
    })?;

    Ok(())
}

fn exit_system(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::KeyQ) || keys.just_pressed(KeyCode::Escape) {
        exit.write_default();
    }
}
