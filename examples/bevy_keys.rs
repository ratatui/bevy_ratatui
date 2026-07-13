use std::ops::DerefMut;

use bevy::{
    app::{AppExit, ScheduleRunnerPlugin},
    input::{ButtonState, keyboard::KeyboardInput},
    prelude::*,
};
use bevy_ratatui::{RatatuiContext, RatatuiPlugins};
use bevy_ratatui::{
    event::KeyMessage,
    kitty::KittyEnabled,
    translation::{Capability, Detected, Emulate, EmulationPolicy, ReleaseKey},
};
use ratatui::crossterm::event::KeyEventKind;
use ratatui::text::Text;

fn main() {
    let wait_duration = std::time::Duration::from_secs_f64(1. / 60.); // 60 FPS
    App::new()
        .add_plugins(RatatuiPlugins {
            enable_input_forwarding: true,
            ..default()
        })
        .add_plugins(ScheduleRunnerPlugin::run_loop(wait_duration))
        .add_systems(
            PreUpdate,
            (
                keyboard_input_system,
                bevy_keyboard_input_system,
                bevy_keypresses,
            ),
        )
        .add_systems(Update, draw_scene_system)
        .add_systems(Update, hotkeys)
        .run();
}

#[derive(Resource, Deref, DerefMut)]
struct LastKeypress(pub KeyMessage);

#[derive(Resource, Deref, DerefMut)]
struct LastBevyKeypress(pub KeyboardInput);

#[derive(Resource, Deref, DerefMut)]
struct BevyKeypresses(pub Vec<KeyCode>);

#[allow(clippy::too_many_arguments)]
fn draw_scene_system(
    mut context: ResMut<RatatuiContext>,
    kitty_enabled: Option<Res<KittyEnabled>>,
    last_keypress: Option<Res<LastKeypress>>,
    last_bevy_keypress: Option<Res<LastBevyKeypress>>,
    bevy_keypresses: Option<Res<BevyKeypresses>>,
    emulate: Option<Res<Emulate>>,
    detected: Res<Detected>,
    release_key: Res<ReleaseKey>,
    policy: Res<EmulationPolicy>,
) -> Result {
    context.draw(|frame| {
        let mut text = Text::raw(if kitty_enabled.is_some() {
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
        text.push_line(if emulate.is_some() {
            "Emulate marker is present."
        } else {
            "Emulate marker is not present."
        });
        text.push_line(format!(
            "Press 'r' to cycle release key policy, currently {:?}",
            release_key
        ));

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

        if let Some(key_press) = last_bevy_keypress {
            let code_string = format!("{:?}", key_press.key_code);
            let kind_string = match key_press.state {
                ButtonState::Pressed => "pressed",
                ButtonState::Released => "released",
            };
            text.push_line("");
            text.push_line(format!("bevy {code_string} key was {kind_string}!"));
        }

        if let Some(key_presses) = bevy_keypresses {
            text.push_line("");
            for key_press in &key_presses.0 {
                let code_string = format!("{:?}", key_press);
                text.push_line(format!("bevy {code_string} key is pressed!"));
            }
        }

        frame.render_widget(text.centered(), frame.area())
    })?;

    Ok(())
}

fn hotkeys(
    input: Res<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
    mut release_key: ResMut<ReleaseKey>,
    mut policy: ResMut<EmulationPolicy>,
) {
    use bevy::input::keyboard::KeyCode::*;
    if input.just_pressed(KeyQ) | input.just_pressed(Escape) {
        exit.write_default();
    } else if input.just_pressed(KeyR) {
        use ReleaseKey::*;
        *release_key = match *release_key {
            OnNextKey => FrameCount(60),
            FrameCount(_) => Duration(std::time::Duration::from_secs(2)),
            Duration(_) => Immediate,
            Immediate => OnNextKey,
        };
    } else if input.just_pressed(KeyP) {
        // Mutate the policy to ensure that the Emulate marker is removed
        // (however briefly).
        policy.deref_mut();
    }
}

fn keyboard_input_system(mut messages: MessageReader<KeyMessage>, mut commands: Commands) {
    for message in messages.read() {
        commands.insert_resource(LastKeypress(*message));
    }
}

fn bevy_keyboard_input_system(mut messages: MessageReader<KeyboardInput>, mut commands: Commands) {
    for message in messages.read() {
        commands.insert_resource(LastBevyKeypress(message.clone()));
    }
}

fn bevy_keypresses(keys: Res<ButtonInput<KeyCode>>, mut commands: Commands) {
    commands.insert_resource(BevyKeypresses(keys.get_pressed().cloned().collect()));
}
