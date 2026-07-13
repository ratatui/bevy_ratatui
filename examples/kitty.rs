use bevy::{
    app::{AppExit, ScheduleRunnerPlugin},
    prelude::*,
};
use bevy_ratatui::{RatatuiContext, RatatuiPlugins, event::KeyMessage, kitty::KittyEnabled};
use ratatui::crossterm::event::KeyEventKind;
use ratatui::text::Text;

fn main() {
    let wait_duration = std::time::Duration::from_secs_f64(1. / 60.); // 60 FPS
    App::new()
        .add_plugins(RatatuiPlugins::default())
        .add_plugins(ScheduleRunnerPlugin::run_loop(wait_duration))
        .add_systems(PreUpdate, keyboard_input_system)
        .add_systems(Update, draw_scene_system)
        .run();
}

#[derive(Resource, Deref, DerefMut)]
struct LastKeypress(pub KeyMessage);

fn draw_scene_system(
    mut context: ResMut<RatatuiContext>,
    kitty_enabled: Option<Res<KittyEnabled>>,
    last_keypress: Option<Res<LastKeypress>>,
) -> Result {
    context.draw(|frame| {
        let mut text = Text::raw(if kitty_enabled.is_some() {
            "Kitty protocol enabled!"
        } else {
            "Kitty protocol not supported in this terminal."
        });

        text.push_line("Press any key. Press 'q' to Quit.");

        if let Some(key_press) = last_keypress {
            let code_string = format!("{:?}", key_press.code);
            let kind_string = match key_press.kind {
                KeyEventKind::Press => "pressed",
                KeyEventKind::Repeat => "repeated",
                KeyEventKind::Release => "released",
            };
            text.push_line("");
            text.push_line(format!("{code_string} key was {kind_string}!"));
        }

        frame.render_widget(text.centered(), frame.area())
    })?;
    Ok(())
}

fn keyboard_input_system(
    mut messages: MessageReader<KeyMessage>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    use ratatui::crossterm::event::KeyCode;
    for message in messages.read() {
        match message.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                exit.write_default();
            }
            _ => {
                commands.insert_resource(LastKeypress(*message));
            }
        }
    }
}
