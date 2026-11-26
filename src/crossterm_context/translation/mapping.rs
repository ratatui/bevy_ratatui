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
    const BASE: &str = "abcdefghijklmnopqrstuvwxyz0123456789-=[]\\;',./` ";
    const SHIFTED: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ)!@#$%^&*(_+{}|:\"<>?~";
    #[rustfmt::skip]
    const CODES: [KeyCode; 48] = [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM,
        KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
        Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
        Minus, Equal, BracketLeft, BracketRight, Backslash, Semicolon, Quote, Comma, Period,
        Slash, Backquote, Space,
    ];
    const KEYPAD_CHARS: &str = "0123456789+-*/.=,";
    #[rustfmt::skip]
    const KEYPAD: [KeyCode; 17] = [
        Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
        NumpadAdd, NumpadSubtract, NumpadMultiply, NumpadDivide, NumpadDecimal, NumpadEqual,
        NumpadComma,
    ];
    const _: () = assert!(BASE.len() == CODES.len());
    const _: () = assert!(SHIFTED.len() + 1 == CODES.len());
    const _: () = assert!(KEYPAD_CHARS.len() == KEYPAD.len());
    let code = |table: &[KeyCode], i: usize| table.get(i).copied().unwrap_or(UNKNOWN);
    if state.contains(KeyEventState::KEYPAD)
        && let Some(i) = KEYPAD_CHARS.find(c)
    {
        return (code(&KEYPAD, i), false);
    }
    if let Some(i) = BASE.find(c) {
        return (code(&CODES, i), false);
    }
    // Caps Lock, not Shift, explains an uppercase letter it is on for.
    let shifted = !(c.is_ascii_uppercase() && state.contains(KeyEventState::CAPS_LOCK));
    SHIFTED
        .find(c)
        .map_or((UNKNOWN, false), |i| (code(&CODES, i), shifted))
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
        LeftHyper | RightHyper => (KeyCode::Hyper, Key::Hyper, Mods::HYPER),
        LeftMeta | RightMeta => (KeyCode::Meta, Key::Meta, Mods::META),
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

// With Num Lock off, keypad navigation keeps its own physical keys.
fn keypad_navigation(code: KeyCode) -> KeyCode {
    use KeyCode::*;
    match code {
        Enter => NumpadEnter,
        ArrowLeft => Numpad4,
        ArrowRight => Numpad6,
        ArrowUp => Numpad8,
        ArrowDown => Numpad2,
        Home => Numpad7,
        End => Numpad1,
        PageUp => Numpad9,
        PageDown => Numpad3,
        Delete => NumpadDecimal,
        Insert => Numpad0,
        code => code,
    }
}

pub(super) fn map(event: KeyEvent, window: Entity) -> (KeyboardInput, Mods) {
    use TermKey::*;
    let mut mods = event.modifiers;
    let mut text = None;
    // Expands to a match on `event.code` that maps identically named keys directly.
    macro_rules! same_named {
        ($($name:ident)|*; $($term:pat => $mapped:expr,)*) => {
            match event.code {
                $(TermKey::$name => (KeyCode::$name, Key::$name),)*
                $($term => $mapped,)*
            }
        };
    }
    let (key_code, logical_key) = same_named! {
        Backspace | Enter | Home | End | PageUp | PageDown | Tab | Delete | Insert | CapsLock
            | ScrollLock | NumLock | PrintScreen | Pause;
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
            let key = if c == ' ' { Key::Space } else { Key::Character(s.into()) };
            (code, key)
        },
        BackTab => {
            mods.insert(Mods::SHIFT);
            (KeyCode::Tab, Key::Tab)
        },
        Left => (KeyCode::ArrowLeft, Key::ArrowLeft),
        Right => (KeyCode::ArrowRight, Key::ArrowRight),
        Up => (KeyCode::ArrowUp, Key::ArrowUp),
        Down => (KeyCode::ArrowDown, Key::ArrowDown),
        Esc => (KeyCode::Escape, Key::Escape),
        Menu => (KeyCode::ContextMenu, Key::ContextMenu),
        KeypadBegin => (KeyCode::Numpad5, UNKNOWN_KEY),
        Null => (UNKNOWN, UNKNOWN_KEY),
        F(n) => function_key(n),
        Media(m) => media(m),
        Modifier(m) => {
            let (code, key, _) = modifier(m);
            (code, key)
        },
    };
    let key_code = if event.state.contains(KeyEventState::KEYPAD) {
        keypad_navigation(key_code)
    } else {
        key_code
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
