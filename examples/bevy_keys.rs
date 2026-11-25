use bevy::{
    app::{AppExit, ScheduleRunnerPlugin},
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use bevy_ratatui::{RatatuiContext, RatatuiPlugins};
use bevy_ratatui::{
    event::KeyMessage,
    translation::{Capability, Detected, EmulationPolicy, ReleaseKey},
};
use ratatui::crossterm::event::KeyEventKind;
use ratatui::text::Text;

fn main() -> Result<()> {
    color_eyre::install()?;

    let wait_duration = std::time::Duration::from_secs_f64(1. / 60.); // 60 FPS
    App::new()
        .add_plugins(RatatuiPlugins {
            enable_input_forwarding: true,
            ..default()
        })
        .add_plugins(ScheduleRunnerPlugin::run_loop(wait_duration))
        .add_systems(Update, draw_scene_system)
        .add_systems(Update, hotkeys)
        .run();

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw_scene_system(
    mut context: ResMut<RatatuiContext>,
    mut keys: MessageReader<KeyMessage>,
    mut forwarded: MessageReader<KeyboardInput>,
    mut last_keypress: Local<Option<KeyMessage>>,
    mut last_bevy_keypress: Local<Option<KeyboardInput>>,
    physical: Res<ButtonInput<KeyCode>>,
    logical: Res<ButtonInput<Key>>,
    detected: Res<Detected>,
    release_key: Res<ReleaseKey>,
    policy: Res<EmulationPolicy>,
) -> Result {
    if let Some(key) = keys.read().last() {
        *last_keypress = Some(*key);
    }
    if let Some(key) = forwarded.read().last() {
        *last_bevy_keypress = Some(key.clone());
    }
    let kitty_enabled = context.kitty_enabled();
    context.draw(|frame| {
        let mut text = Text::raw(if kitty_enabled {
            "Kitty protocol enabled!"
        } else {
            "Kitty protocol not supported in this terminal."
        });

        text.push_line(match detected.0 {
            Capability::ALL => "Detected modifiers and key release.",
            Capability::KEY_RELEASE => "Detected key release but not modifiers.",
            Capability::MODIFIER => "Detected modifiers but not key release.",
            _ => "Did not detect modifiers or key release.",
        });
        text.push_line(match policy.emulate_capabilities(&detected) {
            Capability::ALL => "Emulate modifiers and key release.",
            Capability::KEY_RELEASE => "Emulate key release.",
            Capability::MODIFIER => "Emulate modifiers.",
            _ => "Do not emulate modifiers or key release.",
        });
        text.push_line(format!(
            "Press 'r' to cycle release key policy, currently {:?}",
            release_key
        ));

        text.push_line("Press any key. Press 'q' to Quit.");

        if let Some(key_press) = *last_keypress {
            let code_string = format!("{:?}", key_press.code);
            let kind_string = match key_press.kind {
                KeyEventKind::Press => "pressed",
                KeyEventKind::Repeat => "repeated",
                KeyEventKind::Release => "released",
            };
            text.push_line("");
            text.push_line(format!("{code_string} key was {kind_string}!"));
        }

        if let Some(key_press) = last_bevy_keypress.as_ref() {
            let code_string = format!("{:?}", key_press.key_code);
            let kind_string = match key_press.state {
                ButtonState::Pressed => "pressed",
                ButtonState::Released => "released",
            };
            text.push_line("");
            let repeat = if key_press.repeat { " (repeat)" } else { "" };
            text.push_line(format!("bevy {code_string} key was {kind_string}{repeat}!"));
        }

        text.push_line("");
        for key in physical.get_pressed() {
            text.push_line(format!("bevy {key:?} key is pressed!"));
        }
        for key in logical.get_pressed() {
            text.push_line(format!("logical {key:?} is pressed"));
        }

        frame.render_widget(text.centered(), frame.area())
    })?;

    Ok(())
}

fn hotkeys(
    input: Res<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
    mut release_key: ResMut<ReleaseKey>,
) {
    use bevy::input::keyboard::KeyCode::*;
    if input.just_pressed(KeyQ) || input.just_pressed(Escape) {
        exit.write_default();
    } else if input.just_pressed(KeyR) {
        use ReleaseKey::*;
        *release_key = match *release_key {
            OnNextKey => FrameCount(60),
            FrameCount(_) => Duration(std::time::Duration::from_secs(2)),
            Duration(_) => Immediate,
            Immediate => OnNextKey,
        };
    }
}
