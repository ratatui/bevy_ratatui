use bevy::input::{
    ButtonState,
    keyboard::{Key, KeyCode, KeyboardInput, NativeKey, NativeKeyCode},
};
use bevy::prelude::Entity;
use ratatui::crossterm::event::{
    KeyCode as TermKey, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers as Mods, MediaKeyCode,
    ModifierKeyCode,
};

const UNKNOWN: KeyCode = KeyCode::Unidentified(NativeKeyCode::Unidentified);
const UNKNOWN_KEY: Key = Key::Unidentified(NativeKey::Unidentified);

// Crossterm supplies a character, not a scan code. Infer US-layout positions
// where possible and preserve the exact logical character on every layout.
fn character(c: char, state: KeyEventState) -> (KeyCode, bool) {
    use KeyCode::*;
    const LETTERS: &[KeyCode] = &[
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO,
        KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    ];
    const KEYPAD: &[KeyCode] = &[
        Numpad0,
        Numpad1,
        Numpad2,
        Numpad3,
        Numpad4,
        Numpad5,
        Numpad6,
        Numpad7,
        Numpad8,
        Numpad9,
        NumpadAdd,
        NumpadSubtract,
        NumpadMultiply,
        NumpadDivide,
        NumpadDecimal,
        NumpadEqual,
        NumpadComma,
    ];
    if state.contains(KeyEventState::KEYPAD)
        && let Some(code) = "0123456789+-*/.=,".find(c).and_then(|i| KEYPAD.get(i))
    {
        return (*code, false);
    }
    if c.is_ascii_alphabetic() {
        let index = usize::from(c.to_ascii_lowercase() as u8 - b'a');
        return (
            LETTERS.get(index).copied().unwrap_or(UNKNOWN),
            c.is_ascii_uppercase() && !state.contains(KeyEventState::CAPS_LOCK),
        );
    }
    const SYMBOLS: &[(char, char, KeyCode)] = &[
        ('0', ')', Digit0),
        ('1', '!', Digit1),
        ('2', '@', Digit2),
        ('3', '#', Digit3),
        ('4', '$', Digit4),
        ('5', '%', Digit5),
        ('6', '^', Digit6),
        ('7', '&', Digit7),
        ('8', '*', Digit8),
        ('9', '(', Digit9),
        ('-', '_', Minus),
        ('=', '+', Equal),
        ('[', '{', BracketLeft),
        (']', '}', BracketRight),
        ('\\', '|', Backslash),
        (';', ':', Semicolon),
        ('\'', '"', Quote),
        (',', '<', Comma),
        ('.', '>', Period),
        ('/', '?', Slash),
        ('`', '~', Backquote),
    ];
    SYMBOLS
        .iter()
        .find_map(|&(base, shifted, code)| {
            (c == base || c == shifted).then_some((code, c == shifted))
        })
        .unwrap_or((if c == ' ' { Space } else { UNKNOWN }, false))
}

fn function_key(n: u8) -> (KeyCode, Key) {
    macro_rules! pairs {
        ($($key:ident),*) => { &[$((KeyCode::$key, Key::$key)),*] };
    }
    const FUNCTIONS: &[(KeyCode, Key)] = pairs!(
        F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20,
        F21, F22, F23, F24, F25, F26, F27, F28, F29, F30, F31, F32, F33, F34, F35
    );
    n.checked_sub(1)
        .and_then(|i| FUNCTIONS.get(usize::from(i)))
        .cloned()
        .unwrap_or((UNKNOWN, UNKNOWN_KEY))
}

pub(super) const LEFT_MODIFIERS: [ModifierKeyCode; 6] = {
    use ModifierKeyCode::*;
    [
        LeftShift,
        LeftControl,
        LeftAlt,
        LeftSuper,
        LeftHyper,
        LeftMeta,
    ]
};

pub(super) fn modifier(m: ModifierKeyCode) -> (KeyCode, Key, Mods) {
    use ModifierKeyCode::*;
    match m {
        LeftShift => (KeyCode::ShiftLeft, Key::Shift, Mods::SHIFT),
        RightShift => (KeyCode::ShiftRight, Key::Shift, Mods::SHIFT),
        LeftControl => (KeyCode::ControlLeft, Key::Control, Mods::CONTROL),
        RightControl => (KeyCode::ControlRight, Key::Control, Mods::CONTROL),
        LeftAlt => (KeyCode::AltLeft, Key::Alt, Mods::ALT),
        RightAlt => (KeyCode::AltRight, Key::Alt, Mods::ALT),
        LeftSuper => (KeyCode::SuperLeft, Key::Super, Mods::SUPER),
        RightSuper => (KeyCode::SuperRight, Key::Super, Mods::SUPER),
        LeftHyper => (KeyCode::Hyper, Key::Hyper, Mods::HYPER),
        RightHyper => (KeyCode::Hyper, Key::Hyper, Mods::HYPER),
        LeftMeta => (KeyCode::Meta, Key::Meta, Mods::META),
        RightMeta => (KeyCode::Meta, Key::Meta, Mods::META),
        IsoLevel3Shift => (KeyCode::AltRight, Key::AltGraph, Mods::empty()),
        IsoLevel5Shift => (UNKNOWN, UNKNOWN_KEY, Mods::empty()),
    }
}

fn media(m: MediaKeyCode) -> (KeyCode, Key) {
    use MediaKeyCode::*;
    match m {
        Play => (KeyCode::MediaPlayPause, Key::MediaPlay),
        Pause => (KeyCode::MediaPlayPause, Key::MediaPause),
        PlayPause => (KeyCode::MediaPlayPause, Key::MediaPlayPause),
        Reverse => (UNKNOWN, UNKNOWN_KEY),
        Stop => (KeyCode::MediaStop, Key::MediaStop),
        FastForward => (UNKNOWN, Key::MediaFastForward),
        Rewind => (UNKNOWN, Key::MediaRewind),
        TrackNext => (KeyCode::MediaTrackNext, Key::MediaTrackNext),
        TrackPrevious => (KeyCode::MediaTrackPrevious, Key::MediaTrackPrevious),
        Record => (UNKNOWN, Key::MediaRecord),
        LowerVolume => (KeyCode::AudioVolumeDown, Key::AudioVolumeDown),
        RaiseVolume => (KeyCode::AudioVolumeUp, Key::AudioVolumeUp),
        MuteVolume => (KeyCode::AudioVolumeMute, Key::AudioVolumeMute),
    }
}

pub(super) fn map(event: KeyEvent, window: Entity) -> (KeyboardInput, Mods) {
    use TermKey::*;
    let mut mods = event.modifiers;
    let mut text = None;
    let (key_code, logical_key) = match event.code {
        Char(c) => {
            let (code, shifted) = character(c, event.state);
            if shifted {
                mods.insert(Mods::SHIFT);
            }
            let command = Mods::CONTROL | Mods::ALT | Mods::SUPER | Mods::HYPER | Mods::META;
            let mut buf = [0; 4];
            let s: &str = c.encode_utf8(&mut buf);
            if event.kind != KeyEventKind::Release && !c.is_control() && !mods.intersects(command) {
                text = Some(s.into());
            }
            (
                code,
                if c == ' ' {
                    Key::Space
                } else {
                    Key::Character(s.into())
                },
            )
        }
        BackTab => {
            mods.insert(Mods::SHIFT);
            (KeyCode::Tab, Key::Tab)
        }
        Enter if event.state.contains(KeyEventState::KEYPAD) => (KeyCode::NumpadEnter, Key::Enter),
        Backspace => (KeyCode::Backspace, Key::Backspace),
        Enter => (KeyCode::Enter, Key::Enter),
        Left => (KeyCode::ArrowLeft, Key::ArrowLeft),
        Right => (KeyCode::ArrowRight, Key::ArrowRight),
        Up => (KeyCode::ArrowUp, Key::ArrowUp),
        Down => (KeyCode::ArrowDown, Key::ArrowDown),
        Home => (KeyCode::Home, Key::Home),
        End => (KeyCode::End, Key::End),
        PageUp => (KeyCode::PageUp, Key::PageUp),
        PageDown => (KeyCode::PageDown, Key::PageDown),
        Tab => (KeyCode::Tab, Key::Tab),
        Delete => (KeyCode::Delete, Key::Delete),
        Insert => (KeyCode::Insert, Key::Insert),
        Esc => (KeyCode::Escape, Key::Escape),
        CapsLock => (KeyCode::CapsLock, Key::CapsLock),
        ScrollLock => (KeyCode::ScrollLock, Key::ScrollLock),
        NumLock => (KeyCode::NumLock, Key::NumLock),
        PrintScreen => (KeyCode::PrintScreen, Key::PrintScreen),
        Pause => (KeyCode::Pause, Key::Pause),
        Menu => (KeyCode::ContextMenu, Key::ContextMenu),
        KeypadBegin => (KeyCode::Numpad5, UNKNOWN_KEY),
        Null => (UNKNOWN, UNKNOWN_KEY),
        F(n) => function_key(n),
        Media(m) => media(m),
        Modifier(m) => {
            let (code, key, _) = modifier(m);
            (code, key)
        }
    };
    (
        KeyboardInput {
            key_code,
            logical_key,
            text,
            window,
            repeat: false,
            state: if event.kind == KeyEventKind::Release {
                ButtonState::Released
            } else {
                ButtonState::Pressed
            },
        },
        mods,
    )
}
