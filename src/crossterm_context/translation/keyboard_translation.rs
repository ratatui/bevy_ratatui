//! Input forwarding for the keyboard

use std::{collections::HashSet, hash::Hash, time::Duration};

use crate::crossterm_context::event::{InputSet, KeyMessage};
use bevy::{
    input::{ButtonState, keyboard::KeyboardInput},
    prelude::*,
};
use ratatui::crossterm::event::KeyModifiers;

pub struct TranslationPlugin;

impl Plugin for TranslationPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        if !app.is_plugin_added::<bevy::input::InputPlugin>() {
            // We need this plugin to submit our messages.
            app.add_plugins(bevy::input::InputPlugin);
        }
        if !app.is_plugin_added::<bevy::time::TimePlugin>() {
            // We need this plugin for the delay timer.
            app.add_plugins(bevy::time::TimePlugin);
        }
        app.init_resource::<ReleaseKey>()
            .init_resource::<Detected>()
            .init_resource::<EmulationPolicy>()
            .init_resource::<Emulate>()
            .add_systems(Startup, setup_window)
            .add_systems(
                PreUpdate,
                reset_emulation_check
                    .run_if(resource_changed::<EmulationPolicy>)
                    .in_set(InputSet::Pre),
            )
            .add_systems(
                PreUpdate,
                (detect_capabilities, check_for_emulation)
                    .chain()
                    .run_if(resource_exists::<Emulate>)
                    .in_set(InputSet::CheckEmulation),
            )
            .add_systems(
                PreUpdate,
                (
                    send_key_messages_with_emulation.run_if(resource_exists::<Emulate>),
                    send_key_messages_no_emulation.run_if(not(resource_exists::<Emulate>)),
                )
                    .in_set(InputSet::EmitBevy),
            );
    }
}

bitflags::bitflags! {
    /// Crudely defines some capabilities of terminal. Useful for representing
    /// both detection ([Detect]) and emulation ([EmulationPolicy]).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Capability: u8 {
        /// Represents a terminal that emits its own key release events.
        const KEY_RELEASE = 0b0000_0001;
        /// Represents a terminal that emits its own modifier key press and
        /// release events independently from other keys.
        const MODIFIER = 0b0000_0010;
        const ALL = Self::KEY_RELEASE.bits() | Self::MODIFIER.bits();
    }
}

/// Keyboard emulation policy
///
/// - The [Automatic][EmulationPolicy::Automatic] policy will emulate key release or modifiers if
///   they have not been detected.
///
/// - The [Manual][EmulationPolicy::Manual] policy defines whether modifiers or key releases will be
///   emulated.
///
/// Note: If key releases are emulated and key releases are provided by the
/// terminal, dupliate messages may be sent.
#[derive(Debug, Default, Resource, Clone, Copy)]
pub enum EmulationPolicy {
    /// Emulate everything that has not been detected.
    #[default]
    Automatic,
    /// Define what will be emulated.
    Manual(Capability),
}

impl EmulationPolicy {
    /// Return what capabilities to emulate.
    pub fn emulate_capabilities(&self, detected: &Detected) -> Capability {
        match self {
            EmulationPolicy::Automatic => !detected.0,
            EmulationPolicy::Manual(capabilitiy) => *capabilitiy,
        }
    }
}

/// This represents the currently detected capabilities of the terminal.
///
/// When the program starts, will be empty and this will return true:
/// `detected.is_empty()`. When a key is pressed and released, if a key release
/// was emitted, then `detected` will contain [Capability::KEY_RELEASE].
///
/// Likewise for a modifier key, if any modifier events are detected, then
/// `detected` will contain [Capability::MODIFIER].
///
/// Once those flags are set, they are never unset.
#[derive(Debug, Resource, Default, Deref)]
pub struct Detected(pub Capability);

/// A new type so we can implement Default and use with `Local`.
#[derive(Debug, Deref, DerefMut)]
struct Modifiers(KeyModifiers);

impl Default for Modifiers {
    fn default() -> Self {
        Self(KeyModifiers::empty())
    }
}

/// This is a double-buffer for the last keys pressed.
///
/// - `.0` is current frame's last keys pressed.
/// - `.1` is the next frame's last keys pressed.
///
/// During processing we read from `.0` and write to `.1`.
#[derive(Default)]
struct LastPress(HashSet<KeyboardInput>, HashSet<KeyboardInput>);

impl LastPress {
    fn swap(&mut self) {
        std::mem::swap(&mut self.0, &mut self.1);
    }
}

/// If the terminal does not support sending key release events, this plugin will
/// emulate them. A crossterm event stream might look like this:
///
/// `Press A, Press B`
///
/// which will result in bevy messages, i.e., a key release will be emitted on the
/// next press key event.
///
/// `Press A, Release A, Press B`
///
/// `A` is released when `B` is pressed, but how does one deal with releasing
/// `B`? If another key is pressed, that will release `B`. [ReleaseKey] concerns
/// itself with how to handle the last pressed key assuming it will be a while
/// before another key press comes.
///
/// This plugin will emit messages like so depending on the variant:
///
/// - Duration
///
///   `Press A, Release A, Press B, (timer finishes) Release B`
///
///   The default is a one second duration before emitting a release on the last
///   pressed key
///
/// - FrameCount
///
///   `Press A, Release A, Press B, (frame count reached) Release B`
///
/// - Immediate
///
///   `Press A, Release A, Press B, (next frame) Release B`
///
/// - OnNextKey
///
///   `Press A, Release A, Press B`
///
///   The `Release B` message won't come until another key is pressed. This will
///   make it look like the last key pressed is being held down according to the
///   bevy API.
#[derive(Resource, Debug)]
pub enum ReleaseKey {
    /// Release key after a short duration.
    Duration(Duration),
    /// Release key after a number of frames.
    FrameCount(u32),
    /// Release key next frame.
    Immediate,
    /// Do not emit a key release until someone presses another key.
    OnNextKey,
}

impl Default for ReleaseKey {
    /// Set the release key timer for 1 second by default.
    fn default() -> Self {
        ReleaseKey::Duration(Duration::from_secs(1))
    }
}

#[derive(Default)]
enum ReleaseKeyState {
    #[default]
    None,
    Count(u32),
    Timer(Timer),
}

impl ReleaseKey {
    fn tick(&self, state: &mut ReleaseKeyState, delta: Duration) {
        use ReleaseKey::*;
        match self {
            FrameCount(_) | Immediate => {
                if let ReleaseKeyState::Count(c) = state {
                    *c = c.saturating_add(1);
                } else {
                    *state = ReleaseKeyState::Count(0);
                }
            }
            Duration(d) => {
                if let ReleaseKeyState::Timer(timer) = state {
                    timer.tick(delta);
                } else {
                    *state = ReleaseKeyState::Timer(Timer::new(*d, TimerMode::Once));
                }
            }
            _ => (),
        }
    }

    fn finished(&self, state: &ReleaseKeyState) -> bool {
        use ReleaseKey::*;
        match self {
            OnNextKey => false,
            FrameCount(target) => {
                let ReleaseKeyState::Count(count) = state else {
                    return false;
                };
                count >= target
            }
            Duration(_) => {
                let ReleaseKeyState::Timer(timer) = state else {
                    return false;
                };
                timer.is_finished()
            }
            Immediate => true,
        }
    }

    fn reset(&self, state: &mut ReleaseKeyState) {
        use ReleaseKey::*;
        match self {
            FrameCount(_) | Immediate => *state = ReleaseKeyState::Count(0),
            Duration(d) => {
                if let ReleaseKeyState::Timer(timer) = state {
                    timer.reset();
                } else {
                    *state = ReleaseKeyState::Timer(Timer::new(*d, TimerMode::Once));
                }
            }
            _ => (),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn detect_capabilities(mut keys: MessageReader<KeyMessage>, mut detected: ResMut<Detected>) {
    for key_message in keys.read() {
        if matches!(
            key_message.code,
            ratatui::crossterm::event::KeyCode::Modifier(_)
        ) {
            detected.0 |= Capability::MODIFIER;
        }
        if matches!(
            key_message.kind,
            ratatui::crossterm::event::KeyEventKind::Release
        ) {
            detected.0 |= Capability::KEY_RELEASE;
        }
    }
}

/// Marker resource used to determine whether this plugin will emulate any
/// terminal capabilities. If it is not present, that's the best case because
/// the terminal doesn't require this plugin to emulate any capabilities. It's a
/// simpler and faster code path.
#[derive(Debug, Resource, Default)]
pub struct Emulate;

fn check_for_emulation(
    detected: Res<Detected>,
    policy: Res<EmulationPolicy>,
    mut commands: Commands,
) {
    if policy.emulate_capabilities(&detected).is_empty() {
        // We don't need to emulate anything, so don't.
        commands.remove_resource::<Emulate>();
    }
}

fn reset_emulation_check(mut commands: Commands) {
    commands.insert_resource(Emulate);
}

#[allow(clippy::too_many_arguments)]
fn send_key_messages_with_emulation(
    mut keys: MessageReader<KeyMessage>,
    bevy_window: Single<Entity, With<DummyWindow>>,
    mut modifiers: Local<Modifiers>,
    mut delayed_modifiers: Local<Modifiers>,
    mut last_pressed: Local<LastPress>,
    mut keyboard_input: MessageWriter<KeyboardInput>,
    release_key: Res<ReleaseKey>,
    mut release_key_state: Local<ReleaseKeyState>,
    time: Res<Time>,
    detected: Res<Detected>,
    policy: Res<EmulationPolicy>,
) {
    release_key.tick(&mut release_key_state, time.delta());
    if keys.is_empty() && !release_key.finished(&release_key_state) {
        return;
    }

    for key_message in keys.read() {
        if let Some((bevy_message, mods, repeated)) = key_message_to_bevy(key_message, *bevy_window)
        {
            let emulation = policy.emulate_capabilities(&detected);
            if emulation.contains(Capability::MODIFIER) && mods != **modifiers {
                let delta = mods.symmetric_difference(**modifiers);
                for flag in delta {
                    let state = if mods.contains(flag) {
                        // This flag has been added.
                        ButtonState::Pressed
                    } else {
                        // This flag has been removed.
                        ButtonState::Released
                    };
                    let modifier_message =
                        modifier_to_bevy(crossterm_modifier_to_bevy_key(flag), state, *bevy_window);
                    keyboard_input.write(modifier_message);
                }
                **modifiers = mods;
            }

            if repeated {
                // Repeated key messages are converted to key release messages by
                // `key_message_to_bevy()`. But are queued up to emit a key press
                // on the next frame.
                last_pressed.1.insert(bevy_message.clone());
            }
            if emulation.contains(Capability::KEY_RELEASE) {
                // Must send the release messages ourselves.
                let wrapped = bevy_message.clone();
                if let Some(last_press) = last_pressed.0.take(&wrapped) {
                    // It's being held down. Pass to the next frame.
                    last_pressed.1.insert(last_press);
                } else {
                    last_pressed.1.insert(wrapped);
                    keyboard_input.write(bevy_message);
                }
            } else {
                keyboard_input.write(bevy_message);
            }
        }
    }
    for e in last_pressed.0.drain() {
        // In general this is where we emit key released messages. However,
        // we also emit key pressed messages for repeated keys.
        let reciprocal_message = KeyboardInput {
            state: match e.state {
                ButtonState::Pressed => ButtonState::Released,
                ButtonState::Released => ButtonState::Pressed,
            },
            repeat: e.state == ButtonState::Released,
            ..e
        };
        keyboard_input.write(reciprocal_message);
    }

    if release_key.finished(&release_key_state)
        && policy
            .emulate_capabilities(&detected)
            .contains(Capability::MODIFIER)
    {
        // Delay modifier release until the next tick, if they were pressed under the current tick
        if let ReleaseKey::Immediate | ReleaseKey::FrameCount(0 | 1) = &*release_key {
            std::mem::swap(&mut *modifiers, &mut *delayed_modifiers);
        };

        // Release the modifiers too if we've timed out.
        for flag in **modifiers {
            let state = ButtonState::Released;
            let modifier_message =
                modifier_to_bevy(crossterm_modifier_to_bevy_key(flag), state, *bevy_window);
            keyboard_input.write(modifier_message);
        }
        **modifiers = KeyModifiers::empty();
    }
    last_pressed.swap();
    release_key.reset(&mut release_key_state);
}

/// Send bevy messages without any emulation. This merely shows how simple life is
/// when emulation is not involved.
fn send_key_messages_no_emulation(
    mut keys: MessageReader<KeyMessage>,
    bevy_window: Single<Entity, With<DummyWindow>>,
    mut keyboard_input: MessageWriter<KeyboardInput>,
    mut key_repeat_queue: Local<Vec<KeyboardInput>>,
) {
    for bevy_message in key_repeat_queue.drain(..) {
        keyboard_input.write(bevy_message);
    }

    for key_message in keys.read() {
        if let Some((bevy_message, _modifiers, repeated)) =
            key_message_to_bevy(key_message, *bevy_window)
        {
            if repeated {
                key_repeat_queue.push(KeyboardInput {
                    state: ButtonState::Pressed,
                    ..bevy_message.clone()
                });
            }
            keyboard_input.write(bevy_message);
        }
    }
}

#[derive(Debug, Component)]
struct DummyWindow;

/// This is a dummy window to satisfy the [KeyboardInput] struct.
fn setup_window(mut commands: Commands) {
    // Insert our window entity so that other parts of our app can use them.
    commands.spawn(DummyWindow);
}

fn modifier_to_bevy(
    modifier: bevy::input::keyboard::Key,
    state: bevy::input::ButtonState,
    window: Entity,
) -> bevy::input::keyboard::KeyboardInput {
    use bevy::input::keyboard::{Key as k, KeyCode as c};
    let key_code = match modifier {
        k::Control => c::ControlLeft,
        k::Shift => c::ShiftLeft,
        k::Alt => c::AltLeft,
        k::Hyper => c::Hyper,
        k::Meta => c::Meta,
        k::Super => c::SuperLeft,
        x => panic!("No such modifier {x:?}"),
    };
    let logical_key = modifier;
    bevy::input::keyboard::KeyboardInput {
        key_code,
        state,
        window,
        logical_key,

        // No text produced for modifier keys.
        text: None,

        // Assuming that modifier keys do not repeat.
        repeat: false,
    }
}

fn key_message_to_bevy(
    key_event: &ratatui::crossterm::event::KeyEvent,
    window: Entity,
) -> Option<(
    bevy::input::keyboard::KeyboardInput,
    ratatui::crossterm::event::KeyModifiers,
    bool,
)> {
    let ratatui::crossterm::event::KeyEvent {
        code,
        modifiers,
        kind,
        state: _state,
    } = key_event;
    let mut repeated = false;
    let state = match kind {
        ratatui::crossterm::event::KeyEventKind::Press => bevy::input::ButtonState::Pressed,
        ratatui::crossterm::event::KeyEventKind::Repeat => {
            repeated = true;
            bevy::input::ButtonState::Released
        }
        ratatui::crossterm::event::KeyEventKind::Release => bevy::input::ButtonState::Released,
    };
    let key_code = to_bevy_keycode(code);
    let logical_key = to_bevy_key(code);

    // Using `into()` here because a String or a SmolStr are required depending on the enabled bevy
    // features.
    #[allow(clippy::useless_conversion)]
    let text = logical_key
        .as_ref()
        .and_then(logical_key_to_text)
        .map(|s| s.into());

    key_code
        .zip(logical_key)
        .map(|((key_code, mods), logical_key)| {
            (
                bevy::input::keyboard::KeyboardInput {
                    key_code,
                    state,
                    window,
                    logical_key,
                    text,

                    // Repeat messages are sent next frame based on 'repeated' tuple element.
                    repeat: false,
                },
                *modifiers | mods,
                repeated,
            )
        })
}

fn to_bevy_keycode(
    key_code: &ratatui::crossterm::event::KeyCode,
) -> Option<(
    bevy::input::keyboard::KeyCode,
    ratatui::crossterm::event::KeyModifiers,
)> {
    use bevy::input::keyboard::KeyCode as b;
    use ratatui::crossterm::event::{KeyCode as c, KeyModifiers as m};
    let mut mods = ratatui::crossterm::event::KeyModifiers::empty();
    match key_code {
        c::Backspace => Some(b::Backspace),
        c::Enter => Some(b::Enter),
        c::Left => Some(b::ArrowLeft),
        c::Right => Some(b::ArrowRight),
        c::Up => Some(b::ArrowUp),
        c::Down => Some(b::ArrowDown),
        c::Home => Some(b::Home),
        c::End => Some(b::End),
        c::PageUp => Some(b::PageUp),
        c::PageDown => Some(b::PageDown),
        c::Tab => Some(b::Tab),
        c::BackTab => {
            mods |= m::SHIFT;
            Some(b::Tab)
        }
        c::Delete => Some(b::Delete),
        c::Insert => Some(b::Insert),
        c::F(f) => match f {
            1 => Some(b::F1),
            2 => Some(b::F2),
            3 => Some(b::F3),
            4 => Some(b::F4),
            5 => Some(b::F5),
            6 => Some(b::F6),
            7 => Some(b::F7),
            8 => Some(b::F8),
            9 => Some(b::F9),
            10 => Some(b::F10),
            11 => Some(b::F11),
            12 => Some(b::F12),
            13 => Some(b::F13),
            14 => Some(b::F14),
            15 => Some(b::F15),
            16 => Some(b::F16),
            17 => Some(b::F17),
            18 => Some(b::F18),
            19 => Some(b::F19),
            20 => Some(b::F20),
            31 => Some(b::F31),
            32 => Some(b::F32),
            33 => Some(b::F33),
            34 => Some(b::F34),
            35 => Some(b::F35),
            _ => None,
        },
        c::Char(c) => match c {
            '!' => {
                mods |= m::SHIFT;
                Some(b::Digit1)
            }
            '@' => {
                mods |= m::SHIFT;
                Some(b::Digit2)
            }
            '#' => {
                mods |= m::SHIFT;
                Some(b::Digit3)
            }
            '$' => {
                mods |= m::SHIFT;
                Some(b::Digit4)
            }
            '%' => {
                mods |= m::SHIFT;
                Some(b::Digit5)
            }
            '^' => {
                mods |= m::SHIFT;
                Some(b::Digit6)
            }
            '&' => {
                mods |= m::SHIFT;
                Some(b::Digit7)
            }
            '*' => {
                mods |= m::SHIFT;
                Some(b::Digit8)
            }
            '(' => {
                mods |= m::SHIFT;
                Some(b::Digit9)
            }
            ')' => {
                mods |= m::SHIFT;
                Some(b::Digit0)
            }
            '-' => {
                mods |= m::SHIFT;
                Some(b::Minus)
            }
            '[' => Some(b::BracketLeft),
            ']' => Some(b::BracketRight),
            '{' => {
                mods |= m::SHIFT;
                Some(b::BracketLeft)
            }
            '}' => {
                mods |= m::SHIFT;
                Some(b::BracketRight)
            }
            ',' => Some(b::Comma),
            '=' => Some(b::Equal),
            '<' => {
                mods |= m::SHIFT;
                Some(b::Comma)
            }
            '+' => {
                mods |= m::SHIFT;
                Some(b::Equal)
            }
            '.' => Some(b::Period),
            '>' => {
                mods |= m::SHIFT;
                Some(b::Period)
            }
            '\'' => Some(b::Quote),
            '"' => {
                mods |= m::SHIFT;
                Some(b::Quote)
            }
            ';' => Some(b::Semicolon),
            ':' => {
                mods |= m::SHIFT;
                Some(b::Semicolon)
            }
            '/' => Some(b::Slash),
            '?' => {
                mods |= m::SHIFT;
                Some(b::Slash)
            }
            '`' => Some(b::Backquote),
            '~' => {
                mods |= m::SHIFT;
                Some(b::Backquote)
            }
            ' ' => Some(b::Space),
            '1' => Some(b::Digit1),
            '2' => Some(b::Digit2),
            '3' => Some(b::Digit3),
            '4' => Some(b::Digit4),
            '5' => Some(b::Digit5),
            '6' => Some(b::Digit6),
            '7' => Some(b::Digit7),
            '8' => Some(b::Digit8),
            '9' => Some(b::Digit9),
            '0' => Some(b::Digit0),
            'a' => Some(b::KeyA),
            'b' => Some(b::KeyB),
            'c' => Some(b::KeyC),
            'd' => Some(b::KeyD),
            'e' => Some(b::KeyE),
            'f' => Some(b::KeyF),
            'g' => Some(b::KeyG),
            'h' => Some(b::KeyH),
            'i' => Some(b::KeyI),
            'j' => Some(b::KeyJ),
            'k' => Some(b::KeyK),
            'l' => Some(b::KeyL),
            'm' => Some(b::KeyM),
            'n' => Some(b::KeyN),
            'o' => Some(b::KeyO),
            'p' => Some(b::KeyP),
            'q' => Some(b::KeyQ),
            'r' => Some(b::KeyR),
            's' => Some(b::KeyS),
            't' => Some(b::KeyT),
            'u' => Some(b::KeyU),
            'v' => Some(b::KeyV),
            'w' => Some(b::KeyW),
            'x' => Some(b::KeyX),
            'y' => Some(b::KeyY),
            'z' => Some(b::KeyZ),
            'A' => {
                mods |= m::SHIFT;
                Some(b::KeyA)
            }
            'B' => {
                mods |= m::SHIFT;
                Some(b::KeyB)
            }
            'C' => {
                mods |= m::SHIFT;
                Some(b::KeyC)
            }
            'D' => {
                mods |= m::SHIFT;
                Some(b::KeyD)
            }
            'E' => {
                mods |= m::SHIFT;
                Some(b::KeyE)
            }
            'F' => {
                mods |= m::SHIFT;
                Some(b::KeyF)
            }
            'G' => {
                mods |= m::SHIFT;
                Some(b::KeyG)
            }
            'H' => {
                mods |= m::SHIFT;
                Some(b::KeyH)
            }
            'I' => {
                mods |= m::SHIFT;
                Some(b::KeyI)
            }
            'J' => {
                mods |= m::SHIFT;
                Some(b::KeyJ)
            }
            'K' => {
                mods |= m::SHIFT;
                Some(b::KeyK)
            }
            'L' => {
                mods |= m::SHIFT;
                Some(b::KeyL)
            }
            'M' => {
                mods |= m::SHIFT;
                Some(b::KeyM)
            }
            'N' => {
                mods |= m::SHIFT;
                Some(b::KeyN)
            }
            'O' => {
                mods |= m::SHIFT;
                Some(b::KeyO)
            }
            'P' => {
                mods |= m::SHIFT;
                Some(b::KeyP)
            }
            'Q' => {
                mods |= m::SHIFT;
                Some(b::KeyQ)
            }
            'R' => {
                mods |= m::SHIFT;
                Some(b::KeyR)
            }
            'S' => {
                mods |= m::SHIFT;
                Some(b::KeyS)
            }
            'T' => {
                mods |= m::SHIFT;
                Some(b::KeyT)
            }
            'U' => {
                mods |= m::SHIFT;
                Some(b::KeyU)
            }
            'V' => {
                mods |= m::SHIFT;
                Some(b::KeyV)
            }
            'W' => {
                mods |= m::SHIFT;
                Some(b::KeyW)
            }
            'X' => {
                mods |= m::SHIFT;
                Some(b::KeyX)
            }
            'Y' => {
                mods |= m::SHIFT;
                Some(b::KeyY)
            }
            'Z' => {
                mods |= m::SHIFT;
                Some(b::KeyZ)
            }
            _ => None,
        },
        c::Null => None,
        c::Esc => Some(b::Escape),
        c::CapsLock => Some(b::CapsLock),
        c::ScrollLock => Some(b::ScrollLock),
        c::NumLock => Some(b::NumLock),
        c::PrintScreen => Some(b::PrintScreen),
        c::Pause => Some(b::Pause),
        c::Menu => Some(b::ContextMenu),
        c::KeypadBegin => None,
        c::Media(media) => {
            use ratatui::crossterm::event::MediaKeyCode::*;
            match media {
                Play => Some(b::MediaPlayPause),
                Pause => Some(b::Pause),
                PlayPause => Some(b::MediaPlayPause),
                Reverse => None,
                Stop => Some(b::MediaStop),
                FastForward => Some(b::MediaTrackNext),
                Rewind => Some(b::MediaTrackPrevious),
                TrackNext => Some(b::MediaTrackNext),
                TrackPrevious => Some(b::MediaTrackPrevious),
                Record => None,
                LowerVolume => Some(b::AudioVolumeDown),
                RaiseVolume => Some(b::AudioVolumeUp),
                MuteVolume => Some(b::AudioVolumeMute),
            }
        }
        c::Modifier(modifier) => {
            use ratatui::crossterm::event::ModifierKeyCode::*;
            match modifier {
                LeftShift => Some(b::ShiftLeft),
                LeftControl => Some(b::ControlLeft),
                LeftAlt => Some(b::AltLeft),
                LeftSuper => Some(b::SuperLeft),
                LeftHyper => Some(b::Hyper),
                LeftMeta => Some(b::Meta),
                RightShift => Some(b::ShiftRight),
                RightControl => Some(b::ControlRight),
                RightAlt => Some(b::AltRight),
                RightSuper => Some(b::SuperRight),
                RightHyper => Some(b::Hyper),
                RightMeta => Some(b::Meta),
                IsoLevel3Shift => None,
                IsoLevel5Shift => None,
            }
        }
    }
    .map(|key_code| (key_code, mods))
}

fn to_bevy_key(
    key_code: &ratatui::crossterm::event::KeyCode,
) -> Option<bevy::input::keyboard::Key> {
    use bevy::input::keyboard::Key as b;
    use ratatui::crossterm::event::KeyCode as c;
    match key_code {
        c::Backspace => Some(b::Backspace),
        c::Enter => Some(b::Enter),
        c::Left => Some(b::ArrowLeft),
        c::Right => Some(b::ArrowRight),
        c::Up => Some(b::ArrowUp),
        c::Down => Some(b::ArrowDown),
        c::Home => Some(b::Home),
        c::End => Some(b::End),
        c::PageUp => Some(b::PageUp),
        c::PageDown => Some(b::PageDown),
        c::Tab => Some(b::Tab),
        c::BackTab => {
            // mods |= m::SHIFT;
            Some(b::Tab)
        }
        c::Delete => Some(b::Delete),
        c::Insert => Some(b::Insert),
        c::F(f) => match f {
            1 => Some(b::F1),
            2 => Some(b::F2),
            3 => Some(b::F3),
            4 => Some(b::F4),
            5 => Some(b::F5),
            6 => Some(b::F6),
            7 => Some(b::F7),
            8 => Some(b::F8),
            9 => Some(b::F9),
            10 => Some(b::F10),
            11 => Some(b::F11),
            12 => Some(b::F12),
            13 => Some(b::F13),
            14 => Some(b::F14),
            15 => Some(b::F15),
            16 => Some(b::F16),
            17 => Some(b::F17),
            18 => Some(b::F18),
            19 => Some(b::F19),
            20 => Some(b::F20),
            31 => Some(b::F31),
            32 => Some(b::F32),
            33 => Some(b::F33),
            34 => Some(b::F34),
            35 => Some(b::F35),
            _ => None,
        },
        c::Char(c) => Some({
            let mut tmp = [0u8; 4];
            let s = c.encode_utf8(&mut tmp);
            b::Character(s.into())
        }),
        c::Null => None,
        c::Esc => Some(b::Escape),
        c::CapsLock => Some(b::CapsLock),
        c::ScrollLock => Some(b::ScrollLock),
        c::NumLock => Some(b::NumLock),
        c::PrintScreen => Some(b::PrintScreen),
        c::Pause => Some(b::Pause),
        c::Menu => Some(b::ContextMenu),
        c::KeypadBegin => None,
        c::Media(media) => {
            use ratatui::crossterm::event::MediaKeyCode::*;
            match media {
                Play => Some(b::MediaPlay),
                Pause => Some(b::Pause),
                PlayPause => Some(b::MediaPlayPause),
                Reverse => None,
                Stop => Some(b::MediaStop),
                FastForward => Some(b::MediaFastForward),
                Rewind => Some(b::MediaRewind),
                TrackNext => Some(b::MediaTrackNext),
                TrackPrevious => Some(b::MediaTrackPrevious),
                Record => Some(b::MediaRecord),
                LowerVolume => Some(b::AudioVolumeDown),
                RaiseVolume => Some(b::AudioVolumeUp),
                MuteVolume => Some(b::AudioVolumeMute),
            }
        }
        c::Modifier(modifier) => {
            use ratatui::crossterm::event::ModifierKeyCode::*;
            match modifier {
                LeftShift => Some(b::Shift),
                LeftControl => Some(b::Control),
                LeftAlt => Some(b::Alt),
                LeftSuper => Some(b::Super),
                LeftHyper => Some(b::Hyper),
                LeftMeta => Some(b::Meta),
                RightShift => Some(b::Shift),
                RightControl => Some(b::Control),
                RightAlt => Some(b::Alt),
                RightSuper => Some(b::Super),
                RightHyper => Some(b::Hyper),
                RightMeta => Some(b::Meta),
                IsoLevel3Shift => Some(b::AltGraph),
                IsoLevel5Shift => None,
            }
        }
    }
}

fn crossterm_modifier_to_bevy_key(
    modifier: ratatui::crossterm::event::KeyModifiers,
) -> bevy::input::keyboard::Key {
    let mut i = modifier.into_iter();
    let modifier = i.next().expect("mod");
    use bevy::input::keyboard::Key as k;
    use ratatui::crossterm::event::KeyModifiers as c;
    let result = match modifier {
        c::SHIFT => k::Shift,
        c::CONTROL => k::Control,
        c::ALT => k::Alt,
        c::SUPER => k::Super,
        c::HYPER => k::Hyper,
        c::META => k::Meta,
        x => panic!("Given a modifier of {x:?}"),
    };
    assert!(i.next().is_none());
    result
}

fn logical_key_to_text(logical_key: &bevy::input::keyboard::Key) -> Option<String> {
    if let bevy::input::keyboard::Key::Character(character) = logical_key {
        Some(String::from(character.clone()))
    } else {
        None
    }
}
