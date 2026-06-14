use bevy::{app::AppExit, prelude::*};
use bevy_ratatui::event::KeyMessage;
use bevy_ratatui::{RatatuiContext, RatatuiPlugins};
use ratatui::crossterm::event::KeyCode;
use ratatui::text::Text;

fn main() -> Result<()> {
    color_eyre::install()?;

    App::new()
        .add_plugins((
            MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
                std::time::Duration::from_secs_f32(1. / 60.),
            )),
            RatatuiPlugins::default(),
        ))
        .add_systems(PreUpdate, input_system)
        .add_systems(Update, draw_system)
        .run();

    Ok(())
}

fn draw_system(mut context: ResMut<RatatuiContext>) -> Result {
    context.draw(|frame| {
        let text = Text::raw("hello world\npress 'q' to quit");
        frame.render_widget(text, frame.area());
    })?;

    Ok(())
}

fn input_system(mut messages: MessageReader<KeyMessage>, mut exit: MessageWriter<AppExit>) {
    for message in messages.read() {
        if let KeyCode::Char('q') = message.code {
            exit.write_default();
        }
    }
}
