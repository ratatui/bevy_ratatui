#![cfg(all(feature = "crossterm", feature = "keyboard"))]

use std::time::Duration;

use bevy::{
    input::{
        ButtonState::{self, Pressed, Released},
        keyboard::{Key, KeyboardFocusLost, KeyboardInput, NativeKey, NativeKeyCode},
    },
    prelude::*,
    time::TimeUpdateStrategy,
};
use bevy_ratatui::{
    crossterm::event::{
        KeyCode as C, KeyEvent,
        KeyEventKind::{self, Press, Release, Repeat},
        KeyEventState, KeyModifiers as M, MediaKeyCode,
        ModifierKeyCode::*,
    },
    event::{FocusMessage, InputSet, KeyMessage},
    translation::{Capability, Detected, EmulationPolicy, ReleaseKey, TranslationPlugin},
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(TranslationPlugin);
    app
}

fn kitty() -> App {
    let mut app = app();
    app.insert_resource(Detected(Capability::KEY_RELEASE));
    app
}

fn event(code: C, modifiers: M, kind: KeyEventKind) -> KeyEvent {
    KeyEvent::new_with_kind(code, modifiers, kind)
}

fn key(c: char, kind: KeyEventKind) -> KeyEvent {
    event(C::Char(c), M::empty(), kind)
}

fn logical(c: char) -> Key {
    Key::Character(c.encode_utf8(&mut [0; 4]).into())
}

fn step(app: &mut App, events: &[KeyEvent]) -> Vec<KeyboardInput> {
    let mut cursor = app
        .world()
        .resource::<Messages<KeyboardInput>>()
        .get_cursor_current();
    for &e in events {
        app.world_mut().write_message(KeyMessage(e));
    }
    app.update();
    cursor.read(app.world().resource()).cloned().collect()
}

fn edges(messages: &[KeyboardInput]) -> Vec<(KeyCode, ButtonState)> {
    messages.iter().map(|e| (e.key_code, e.state)).collect()
}

fn physical(app: &App) -> &ButtonInput<KeyCode> {
    app.world().resource()
}

fn logical_keys(app: &App) -> &ButtonInput<Key> {
    app.world().resource()
}

fn assert_empty(app: &App) {
    assert_eq!(physical(app).get_pressed().count(), 0);
    assert_eq!(logical_keys(app).get_pressed().count(), 0);
}

#[derive(Default, Resource)]
struct Samples(Vec<(bool, bool, bool, bool)>);

fn sample(
    keys: Res<ButtonInput<KeyCode>>,
    logical_keys: Res<ButtonInput<Key>>,
    mut samples: ResMut<Samples>,
) {
    samples.0.push((
        keys.pressed(KeyCode::KeyA),
        logical_keys.pressed(logical('a')),
        keys.just_released(KeyCode::KeyA),
        logical_keys.just_released(logical('a')),
    ));
}

#[test]
fn a_terminal_tap_is_visible_in_post_and_update() {
    let mut app = kitty();
    app.init_resource::<Samples>()
        .add_systems(PreUpdate, sample.in_set(InputSet::Post))
        .add_systems(Update, sample);
    step(&mut app, &[key('a', Press), key('a', Release)]);
    step(&mut app, &[]);
    assert_eq!(
        app.world().resource::<Samples>().0,
        [
            (true, true, false, false),
            (true, true, false, false),
            (false, false, true, true),
            (false, false, true, true)
        ]
    );
}

#[test]
fn flag_only_ctrl_chords_press_and_release_in_order() {
    let mut app = kitty();
    for (c, code) in [('a', KeyCode::KeyA), ('b', KeyCode::KeyB)] {
        let chord = |kind| event(C::Char(c), M::CONTROL, kind);
        let down = step(&mut app, &[chord(Press), chord(Release)]);
        assert_eq!(
            edges(&down),
            [(KeyCode::ControlLeft, Pressed), (code, Pressed)]
        );
        assert!(physical(&app).all_pressed([KeyCode::ControlLeft, code]));
        assert!(logical_keys(&app).all_pressed([Key::Control, logical(c)]));
        let up = step(&mut app, &[]);
        assert_eq!(
            edges(&up),
            [(code, Released), (KeyCode::ControlLeft, Released)]
        );
        assert!(down.iter().chain(&up).all(|e| e.text.is_none()));
        assert_empty(&app);
    }
    assert_eq!(
        app.world().resource::<Detected>().0,
        Capability::KEY_RELEASE
    );
}

#[test]
fn repeats_refresh_flags_and_release_uses_press_time_logical_key() {
    let mut app = kitty();
    step(&mut app, &[event(C::Char('A'), M::SHIFT, Press)]);
    let messages = step(&mut app, &[key('a', Repeat)]);
    assert_eq!(
        edges(&messages),
        [(KeyCode::ShiftLeft, Released), (KeyCode::KeyA, Pressed)]
    );
    assert!(messages[1].repeat);
    assert_eq!(messages[1].text.as_deref(), Some("a"));
    assert!(physical(&app).just_released(KeyCode::ShiftLeft));
    assert!(physical(&app).pressed(KeyCode::KeyA));
    assert!(!physical(&app).just_pressed(KeyCode::KeyA));
    let up = step(&mut app, &[key('a', Release)]);
    assert_eq!(up[0].logical_key, logical('A'));
    assert!(up[0].text.is_none() && !up[0].repeat);
    assert_empty(&app);
}

#[test]
fn shared_modifier_survives_until_the_last_chord_releases_it() {
    let mut app = kitty();
    let chord = |c| event(C::Char(c), M::CONTROL, Press);
    step(&mut app, &[chord('a'), chord('b')]);
    let repeat = step(&mut app, &[key('a', Repeat)]);
    assert_eq!(edges(&repeat), [(KeyCode::KeyA, Pressed)]);
    assert!(repeat[0].repeat);
    assert!(physical(&app).pressed(KeyCode::ControlLeft));
    let release = step(&mut app, &[key('b', Release)]);
    assert_eq!(
        edges(&release),
        [(KeyCode::KeyB, Released), (KeyCode::ControlLeft, Released)]
    );
    assert!(physical(&app).pressed(KeyCode::KeyA));
}

#[test]
fn native_modifier_release_strips_flags_and_later_chords_still_emulate() {
    let mut app = app();
    let ctrl = |kind| event(C::Modifier(LeftControl), M::CONTROL, kind);
    let chord = |c, kind| event(C::Char(c), M::CONTROL, kind);
    assert_eq!(
        edges(&step(&mut app, &[ctrl(Press)])),
        [(KeyCode::ControlLeft, Pressed)]
    );
    assert_eq!(app.world().resource::<Detected>().0, Capability::MODIFIER);
    // A held real modifier from a press-only terminal can use a timer.
    app.insert_resource(ReleaseKey::OnNextKey);
    assert_eq!(
        edges(&step(&mut app, &[chord('a', Press)])),
        [(KeyCode::KeyA, Pressed)]
    );
    assert_eq!(
        edges(&step(&mut app, &[ctrl(Release)])),
        [(KeyCode::ControlLeft, Released)]
    );
    assert!(physical(&app).pressed(KeyCode::KeyA));
    step(&mut app, &[key('a', Release)]);
    assert_empty(&app);
    let b = step(&mut app, &[chord('b', Press), chord('b', Release)]);
    assert_eq!(
        edges(&b),
        [(KeyCode::ControlLeft, Pressed), (KeyCode::KeyB, Pressed)]
    );
    step(&mut app, &[]);
    assert_empty(&app);
    assert_eq!(app.world().resource::<Detected>().0, Capability::ALL);
}

#[test]
fn synthetic_modifiers_transfer_to_native_left_or_right() {
    for (side, code) in [
        (LeftShift, KeyCode::ShiftLeft),
        (RightShift, KeyCode::ShiftRight),
    ] {
        let mut app = kitty();
        step(&mut app, &[key('A', Press)]);
        let real = step(&mut app, &[event(C::Modifier(side), M::SHIFT, Press)]);
        if side == LeftShift {
            assert!(real.is_empty());
        } else {
            assert_eq!(
                edges(&real),
                [(KeyCode::ShiftLeft, Released), (code, Pressed)]
            );
        }
        let up = step(
            &mut app,
            &[
                event(C::Modifier(side), M::empty(), Release),
                key('a', Release),
            ],
        );
        assert_eq!(edges(&up), [(code, Released), (KeyCode::KeyA, Released)]);
        assert_eq!(up[1].logical_key, logical('A'));
        assert_empty(&app);
    }
}

#[test]
fn orphan_repeat_starts_a_hold_and_repeat_release_has_no_extra_frame() {
    let mut app = app();
    let first = step(&mut app, &[key('a', Repeat)]);
    assert!(!first[0].repeat);
    let next = step(
        &mut app,
        &[key('a', Repeat), key('a', Release), key('x', Release)],
    );
    assert_eq!(
        edges(&next),
        [(KeyCode::KeyA, Pressed), (KeyCode::KeyA, Released)]
    );
    assert!(next[0].repeat && !next[1].repeat);
    assert_empty(&app);
}

#[test]
fn focus_loss_flushes_pending_releases_every_hold_and_allows_future_input() {
    let mut app = kitty();
    step(
        &mut app,
        &[key('a', Press), key('a', Release), key('b', Press)],
    );
    let mut lost = app
        .world()
        .resource::<Messages<KeyboardFocusLost>>()
        .get_cursor_current();
    for msg in [FocusMessage::Lost, FocusMessage::Gained, FocusMessage::Lost] {
        app.world_mut().write_message(msg);
    }
    let batch = step(&mut app, &[event(C::Char('z'), M::CONTROL, Press)]);
    assert_eq!(
        edges(&batch),
        [
            (KeyCode::KeyA, Released),
            (KeyCode::KeyB, Pressed),
            (KeyCode::ControlLeft, Pressed),
            (KeyCode::KeyZ, Pressed),
            (KeyCode::KeyB, Released),
            (KeyCode::KeyZ, Released),
            (KeyCode::ControlLeft, Released)
        ]
    );
    assert_empty(&app);
    assert_eq!(
        lost.read(app.world().resource()).count(),
        usize::from(!cfg!(feature = "windowed"))
    );
    step(&mut app, &[key('c', Press)]);
    assert!(physical(&app).pressed(KeyCode::KeyC));
    assert_eq!(lost.read(app.world().resource()).count(), 0);
}

#[test]
fn distinct_unicode_holds_match_case_changing_releases() {
    let mut app = kitty();
    let downs = step(&mut app, &[key('É', Press), key('ü', Press)]);
    assert_eq!(downs.len(), 2);
    assert!(!downs[1].repeat);
    let up = step(&mut app, &[key('é', Release)]);
    assert_eq!(up[0].logical_key, logical('É'));
    assert!(logical_keys(&app).pressed(logical('ü')));
    assert!(step(&mut app, &[key('é', Release)]).is_empty());
    step(&mut app, &[key('Ü', Release)]);
    assert_empty(&app);
}

#[test]
fn unicode_matching_folds_sharp_s_and_final_sigma() {
    let mut app = kitty();
    step(&mut app, &[key('ß', Press), key('Σ', Press)]);
    let up = step(&mut app, &[key('ẞ', Release)]);
    assert_eq!(up[0].logical_key, logical('ß'));
    assert!(logical_keys(&app).pressed(logical('Σ')));
    let up = step(&mut app, &[key('ς', Release)]);
    assert_eq!(up[0].logical_key, logical('Σ'));
    assert_empty(&app);
}

#[test]
fn standalone_modifier_release_also_clears_flag_only_holds() {
    let mut app = kitty();
    step(&mut app, &[key('A', Press)]);
    let up = step(
        &mut app,
        &[event(C::Modifier(RightShift), M::empty(), Release)],
    );
    assert_eq!(edges(&up), [(KeyCode::ShiftLeft, Released)]);
    assert!(physical(&app).pressed(KeyCode::KeyA));
    step(&mut app, &[key('a', Release)]);
    assert_empty(&app);
}

#[test]
fn first_kitty_key_survives_idle_but_unseeded_presses_use_current_timeout() {
    assert_eq!(ReleaseKey::default(), ReleaseKey::Immediate);
    let mut native = kitty();
    step(&mut native, &[key('a', Press)]);
    step(&mut native, &[]);
    assert!(physical(&native).pressed(KeyCode::KeyA));
    for timer in [
        ReleaseKey::Immediate,
        ReleaseKey::FrameCount(0),
        ReleaseKey::Duration(Duration::ZERO),
    ] {
        let mut plain = app();
        plain.insert_resource(timer);
        step(&mut plain, &[key('a', Press)]);
        assert!(physical(&plain).pressed(KeyCode::KeyA));
        let refreshed = step(&mut plain, &[key('a', Press)]);
        assert!(refreshed[0].repeat && physical(&plain).pressed(KeyCode::KeyA));
        step(&mut plain, &[]);
        assert_empty(&plain);
    }
}

#[test]
fn frame_count_starts_again_on_each_legacy_repeat() {
    let mut app = app();
    app.insert_resource(ReleaseKey::FrameCount(2));
    step(&mut app, &[key('a', Press)]);
    step(&mut app, &[]);
    assert!(step(&mut app, &[key('a', Press)])[0].repeat);
    assert!(step(&mut app, &[]).is_empty());
    assert!(physical(&app).pressed(KeyCode::KeyA));
    assert_eq!(edges(&step(&mut app, &[])), [(KeyCode::KeyA, Released)]);
    assert_empty(&app);
}

#[test]
fn changing_a_running_timer_uses_real_time_when_virtual_time_is_paused() {
    let mut app = app();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        10,
    )))
    .insert_resource(ReleaseKey::OnNextKey);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    step(&mut app, &[key('b', Press)]);
    step(&mut app, &[]);
    step(&mut app, &[]);
    assert!(physical(&app).pressed(KeyCode::KeyB));
    app.insert_resource(ReleaseKey::Duration(Duration::from_millis(30)));
    assert_eq!(edges(&step(&mut app, &[])), [(KeyCode::KeyB, Released)]);
    assert_eq!(
        app.world().resource::<Time<Virtual>>().elapsed(),
        Duration::ZERO
    );
}

#[test]
fn policy_changes_reconcile_held_keys_without_replaying_input() {
    let mut app = app();
    app.insert_resource(ReleaseKey::OnNextKey);
    step(&mut app, &[event(C::Char('a'), M::ALT, Press)]);
    app.insert_resource(EmulationPolicy::Manual(Capability::empty()));
    assert_eq!(
        edges(&step(&mut app, &[])),
        [(KeyCode::KeyA, Released), (KeyCode::AltLeft, Released)]
    );
    assert_empty(&app);
    let plain = step(&mut app, &[event(C::Char('b'), M::ALT, Press)]);
    assert_eq!(edges(&plain), [(KeyCode::KeyB, Pressed)]);
    app.insert_resource(EmulationPolicy::Manual(Capability::KEY_RELEASE));
    app.insert_resource(ReleaseKey::Immediate);
    assert_eq!(edges(&step(&mut app, &[])), [(KeyCode::KeyB, Released)]);
    step(&mut app, &[]);
    assert_empty(&app);
}

#[test]
fn switching_to_emulated_release_times_out_native_holds() {
    let mut app = kitty();
    step(&mut app, &[key('a', Press)]);
    step(&mut app, &[]);
    assert!(physical(&app).pressed(KeyCode::KeyA));
    app.insert_resource(EmulationPolicy::Manual(Capability::KEY_RELEASE));
    assert_eq!(edges(&step(&mut app, &[])), [(KeyCode::KeyA, Released)]);
    assert_empty(&app);
}

#[test]
fn learning_native_releases_hands_timed_holds_to_the_terminal() {
    let ctrl = |kind| event(C::Modifier(LeftControl), M::CONTROL, kind);
    for timer in [
        ReleaseKey::Immediate,
        ReleaseKey::FrameCount(1),
        ReleaseKey::Duration(Duration::ZERO),
    ] {
        let mut app = app();
        app.insert_resource(timer);
        step(
            &mut app,
            &[ctrl(Press), event(C::Char('a'), M::CONTROL, Press)],
        );
        let up = step(&mut app, &[key('a', Release)]);
        assert_eq!(edges(&up), [(KeyCode::KeyA, Released)], "{timer:?}");
        for _ in 0..3 {
            assert!(step(&mut app, &[]).is_empty(), "{timer:?}");
        }
        assert!(physical(&app).pressed(KeyCode::ControlLeft));
        let up = step(&mut app, &[ctrl(Release)]);
        assert_eq!(edges(&up), [(KeyCode::ControlLeft, Released)]);
        assert_empty(&app);
    }

    // A release learned between presses, across frames or within one, ends supersession.
    for frames in [
        &[
            &[key('a', Press)][..],
            &[key('x', Release)],
            &[key('b', Press)],
        ][..],
        &[&[key('a', Press), key('x', Release), key('b', Press)]],
    ] {
        let mut app = app();
        app.insert_resource(ReleaseKey::OnNextKey);
        for events in frames {
            step(&mut app, events);
        }
        step(&mut app, &[]);
        assert!(physical(&app).all_pressed([KeyCode::KeyA, KeyCode::KeyB]));
        step(&mut app, &[key('a', Release)]);
        assert!(physical(&app).pressed(KeyCode::KeyB));
        step(&mut app, &[key('b', Release)]);
        assert_empty(&app);
    }
}

#[test]
fn a_chord_keeps_its_held_native_modifier_alive() {
    for policy in [
        EmulationPolicy::Manual(Capability::ALL),
        EmulationPolicy::Automatic,
    ] {
        let mut app = app();
        app.insert_resource(policy);
        step(
            &mut app,
            &[event(C::Modifier(LeftControl), M::CONTROL, Press)],
        );
        let chord = step(&mut app, &[event(C::Char('a'), M::CONTROL, Press)]);
        assert_eq!(edges(&chord), [(KeyCode::KeyA, Pressed)], "{policy:?}");
        assert!(physical(&app).all_pressed([KeyCode::ControlLeft, KeyCode::KeyA]));
        let up = step(&mut app, &[]);
        assert_eq!(
            edges(&up),
            [(KeyCode::ControlLeft, Released), (KeyCode::KeyA, Released)]
        );
        assert_empty(&app);
    }
}

#[test]
fn keypad_navigation_does_not_collide_with_dedicated_keys() {
    let mut app = kitty();
    let pad = |kind| KeyEvent {
        state: KeyEventState::KEYPAD,
        ..event(C::Left, M::empty(), kind)
    };
    step(&mut app, &[event(C::Left, M::empty(), Press)]);
    let down = step(&mut app, &[pad(Press)]);
    assert_eq!(edges(&down), [(KeyCode::Numpad4, Pressed)]);
    assert_eq!(down[0].logical_key, Key::ArrowLeft);
    let up = step(&mut app, &[pad(Release)]);
    assert_eq!(edges(&up), [(KeyCode::Numpad4, Released)]);
    assert!(physical(&app).pressed(KeyCode::ArrowLeft));
    step(&mut app, &[event(C::Left, M::empty(), Release)]);
    assert_empty(&app);
}

#[test]
fn supersession_and_repeated_taps_preserve_visible_down_and_up_frames() {
    let mut legacy = app();
    legacy.insert_resource(ReleaseKey::OnNextKey);
    assert_eq!(
        edges(&step(&mut legacy, &[key('a', Press), key('b', Press)])),
        [(KeyCode::KeyA, Pressed)]
    );
    assert_eq!(
        edges(&step(&mut legacy, &[])),
        [(KeyCode::KeyA, Released), (KeyCode::KeyB, Pressed)]
    );
    let mut app = kitty();
    let taps = [key('z', Press), key('z', Release)].repeat(2);
    for (frame, state) in [Pressed, Released, Pressed, Released]
        .into_iter()
        .enumerate()
    {
        let messages = step(&mut app, if frame == 0 { &taps } else { &[] });
        assert_eq!(edges(&messages), [(KeyCode::KeyZ, state)]);
        assert_eq!(physical(&app).pressed(KeyCode::KeyZ), state == Pressed);
    }
}

#[test]
fn oversized_burst_flushes_the_pending_prefix_and_new_input_in_wire_order() {
    let mut app = kitty();
    step(&mut app, &[key('a', Press), key('a', Release)]);
    let taps: Vec<_> = ('b'..='z')
        .flat_map(|c| [key(c, Press), key(c, Release)])
        .collect();
    let output = step(&mut app, &taps);
    let transitions: Vec<_> = output
        .into_iter()
        .map(|e| (e.logical_key, e.state))
        .collect();
    let expected: Vec<_> = [(logical('a'), Released)]
        .into_iter()
        .chain(('b'..='z').flat_map(|c| [(logical(c), Pressed), (logical(c), Released)]))
        .collect();
    assert_eq!(transitions, expected);
    assert_empty(&app);
    assert!(step(&mut app, &[]).is_empty());
}

#[test]
fn every_modifier_bit_pattern_handles_text_repeats_and_cleanup() {
    let mut app = kitty();
    for bits in 0..=u8::MAX {
        let mods = M::from_bits_retain(bits);
        let key = |kind| event(C::Char('x'), mods, kind);
        let command = M::CONTROL | M::ALT | M::HYPER | M::SUPER | M::META;
        let text = (!mods.intersects(command)).then_some("x");
        for kind in [Press, Repeat] {
            let messages = step(&mut app, &[key(kind)]);
            let letter = messages.last().unwrap();
            assert_eq!(
                (letter.key_code, letter.text.as_deref(), letter.repeat),
                (KeyCode::KeyX, text, kind == Repeat)
            );
            for (bit, code) in [
                (M::SHIFT, KeyCode::ShiftLeft),
                (M::CONTROL, KeyCode::ControlLeft),
                (M::ALT, KeyCode::AltLeft),
                (M::SUPER, KeyCode::SuperLeft),
                (M::HYPER, KeyCode::Hyper),
                (M::META, KeyCode::Meta),
            ] {
                assert_eq!(physical(&app).pressed(code), mods.contains(bit));
            }
        }
        let up = step(&mut app, &[key(Release)]);
        assert!(
            up.iter()
                .all(|e| e.state == Released && e.text.is_none() && !e.repeat)
        );
        assert_empty(&app);
    }
}

#[test]
fn maps_characters_keypad_and_named_keys() {
    use KeyCode::*;
    let mut app = kitty();
    let keypad = KeyEventState::KEYPAD;
    let caps = KeyEventState::CAPS_LOCK;
    let none = KeyEventState::empty();
    let unknown = Unidentified(NativeKeyCode::Unidentified);
    let unknown_key = Key::Unidentified(NativeKey::Unidentified);
    let mut cases = vec![
        (C::Char('-'), none, Minus, logical('-'), Some("-"), false),
        (C::Char('_'), none, Minus, logical('_'), Some("_"), true),
        (
            C::Char('\\'),
            none,
            Backslash,
            logical('\\'),
            Some("\\"),
            false,
        ),
        (C::Char('|'), none, Backslash, logical('|'), Some("|"), true),
        (C::Char(' '), none, Space, Key::Space, Some(" "), false),
        (C::Char('é'), none, unknown, logical('é'), Some("é"), false),
        (
            C::Char('中'),
            none,
            unknown,
            logical('中'),
            Some("中"),
            false,
        ),
        (C::Char('\x01'), none, unknown, logical('\x01'), None, false),
        (C::Char('A'), caps, KeyA, logical('A'), Some("A"), false),
        (C::CapsLock, none, CapsLock, Key::CapsLock, None, false),
        (
            C::KeypadBegin,
            none,
            Numpad5,
            unknown_key.clone(),
            None,
            false,
        ),
        (C::Enter, keypad, NumpadEnter, Key::Enter, None, false),
        (C::Left, keypad, Numpad4, Key::ArrowLeft, None, false),
        (C::Right, keypad, Numpad6, Key::ArrowRight, None, false),
        (C::Up, keypad, Numpad8, Key::ArrowUp, None, false),
        (C::Down, keypad, Numpad2, Key::ArrowDown, None, false),
        (C::Home, keypad, Numpad7, Key::Home, None, false),
        (C::End, keypad, Numpad1, Key::End, None, false),
        (C::PageUp, keypad, Numpad9, Key::PageUp, None, false),
        (C::PageDown, keypad, Numpad3, Key::PageDown, None, false),
        (C::Insert, keypad, Numpad0, Key::Insert, None, false),
        (C::Delete, keypad, NumpadDecimal, Key::Delete, None, false),
        (C::Left, none, ArrowLeft, Key::ArrowLeft, None, false),
        (C::BackTab, none, Tab, Key::Tab, None, true),
        (C::F(0), none, unknown, unknown_key.clone(), None, false),
        (C::F(36), none, unknown, unknown_key.clone(), None, false),
        (C::Null, none, unknown, unknown_key, None, false),
        (
            C::Modifier(IsoLevel3Shift),
            none,
            AltRight,
            Key::AltGraph,
            None,
            false,
        ),
        (
            C::Media(MediaKeyCode::Record),
            none,
            unknown,
            Key::MediaRecord,
            None,
            false,
        ),
    ];
    for (i, code) in [
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
    ]
    .into_iter()
    .enumerate()
    {
        let s = &"0123456789+-*/.=,"[i..=i];
        let c = char::from(s.as_bytes()[0]);
        cases.push((C::Char(c), keypad, code, logical(c), Some(s), false));
    }
    for (term, state, code, logical_key, text, shifted) in cases {
        let press = KeyEvent {
            state,
            ..event(term, M::empty(), Press)
        };
        let down = step(&mut app, &[press]);
        let mapped = down.last().unwrap();
        assert_eq!(
            (mapped.key_code, &mapped.logical_key, mapped.text.as_deref()),
            (code, &logical_key, text),
            "{press:?}"
        );
        assert_eq!(physical(&app).pressed(ShiftLeft), shifted, "{press:?}");
        step(
            &mut app,
            &[KeyEvent {
                kind: Release,
                ..press
            }],
        );
        assert_empty(&app);
    }
    for n in 1..=35 {
        let f = |kind| event(C::F(n), M::empty(), kind);
        let messages = step(&mut app, &[f(Press)]);
        assert_eq!(format!("{:?}", messages[0].key_code), format!("F{n}"));
        assert_eq!(format!("{:?}", messages[0].logical_key), format!("F{n}"));
        assert!(messages[0].text.is_none());
        step(&mut app, &[f(Release)]);
        assert_empty(&app);
    }
}

#[test]
fn emulate_capabilities_reflects_detected_and_deferred_events_do_not_mutate_early() {
    let detected = Detected(Capability::ALL);
    let copy: Detected = detected;
    assert_eq!(
        EmulationPolicy::Automatic.emulate_capabilities(&copy),
        Capability::empty()
    );

    let mut app = app();
    let down = step(&mut app, &[key('a', Press), key('a', Release)]);
    assert_eq!(edges(&down), [(KeyCode::KeyA, Pressed)]);
    assert_eq!(app.world().resource::<Detected>().0, Capability::empty());
    let up = step(&mut app, &[]);
    assert_eq!(edges(&up), [(KeyCode::KeyA, Released)]);
    assert_eq!(
        app.world().resource::<Detected>().0,
        Capability::KEY_RELEASE
    );
    assert_empty(&app);
}
