use std::{collections::VecDeque, time::Duration};

#[cfg(not(feature = "windowed"))]
use bevy::input::keyboard::KeyboardFocusLost;
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use ratatui::crossterm::event::{KeyCode as TermKey, KeyEvent, KeyEventKind, KeyModifiers as Mods};

use super::{Capability, Detected, EmulationPolicy, ReleaseKey, TerminalWindow, mapping};
use crate::event::{FocusMessage, KeyMessage};

// Beyond this many queued events, publish the whole burst in wire order. This
// bounds latency and storage at the cost of per-tap visibility for large pastes.
const MAX_PENDING: usize = 32;

#[derive(Clone, Copy)]
enum Source {
    Key(Mods),
    Native(Mods),
    Synthetic(Mods),
}

#[derive(Clone)]
struct Down {
    press: KeyboardInput,
    source: Source,
    since: u64,
    seen: u64,
    at: Duration,
    timed: bool,
}

#[derive(Clone, Copy)]
struct Frame {
    number: u64,
    now: Duration,
    window: Entity,
}

impl Frame {
    fn down(self, press: KeyboardInput, source: Source, timed: bool) -> Down {
        Down {
            press,
            source,
            since: self.number,
            seen: self.number,
            at: self.now,
            timed,
        }
    }
}

fn same_key(a: &KeyboardInput, b: &KeyboardInput) -> bool {
    if a.key_code != b.key_code {
        return false;
    }
    if !matches!(a.key_code, KeyCode::Unidentified(_)) {
        return true;
    }
    match (&a.logical_key, &b.logical_key) {
        (Key::Character(a), Key::Character(b)) => {
            // Lower then upper also unifies final sigma and sharp S. Dotless i
            // belongs to its own class under locale-independent case folding.
            fn folded(s: &str) -> impl Iterator<Item = char> {
                s.chars()
                    .flat_map(char::to_lowercase)
                    .flat_map(char::to_uppercase)
            }
            a == b || (a.as_str() != "ı" && b.as_str() != "ı" && folded(a).eq(folded(b)))
        }
        (a, b) => a == b,
    }
}

#[derive(Default)]
struct Held(Vec<Down>);

impl Held {
    fn release(&mut self, select: impl Fn(&Down) -> bool, out: &mut Vec<KeyboardInput>) -> Mods {
        let mut stripped = Mods::empty();
        for d in self.0.extract_if(.., |d| select(d)) {
            if let Source::Native(bit) = d.source {
                stripped |= bit;
            }
            out.push(KeyboardInput {
                state: ButtonState::Released,
                text: None,
                repeat: false,
                ..d.press
            });
        }
        stripped
    }

    fn strip_modifiers(&mut self, stripped: Mods) {
        if !stripped.is_empty() {
            for d in &mut self.0 {
                if let Source::Key(flags) = &mut d.source {
                    flags.remove(stripped);
                }
            }
        }
    }

    fn sync_modifiers(
        &mut self,
        emulate_modifiers: bool,
        frame: Frame,
        preserve_new: bool,
        out: &mut Vec<KeyboardInput>,
    ) {
        let mut flags = Mods::empty();
        let mut native = Mods::empty();
        let mut synthetic = Mods::empty();
        for d in &self.0 {
            match d.source {
                Source::Key(bits) => flags |= bits,
                Source::Native(bit) => native |= bit,
                Source::Synthetic(bit) => synthetic |= bit,
            }
        }
        let wanted = if emulate_modifiers {
            flags - native
        } else {
            Mods::empty()
        };
        self.release(
            |d| {
                matches!(d.source, Source::Synthetic(bit) if !wanted.contains(bit))
                    && !(preserve_new && d.since == frame.number)
            },
            out,
        );
        let missing = wanted - synthetic;
        if missing.is_empty() {
            return;
        }
        for m in mapping::LEFT_MODIFIERS {
            let (key_code, logical_key, bit) = mapping::modifier(m);
            if missing.contains(bit) {
                let press = KeyboardInput {
                    key_code,
                    logical_key,
                    window: frame.window,
                    text: None,
                    state: ButtonState::Pressed,
                    repeat: false,
                };
                self.0
                    .push(frame.down(press.clone(), Source::Synthetic(bit), false));
                out.push(press);
            }
        }
    }

    fn apply(
        &mut self,
        event: KeyEvent,
        emulate_release: bool,
        emulate_modifiers: bool,
        frame: Frame,
        out: &mut Vec<KeyboardInput>,
    ) {
        let (mut press, flags) = mapping::map(event, frame.window);
        if event.kind == KeyEventKind::Release {
            self.release(
                |d| !matches!(d.source, Source::Synthetic(_)) && same_key(&d.press, &press),
                out,
            );
            if let TermKey::Modifier(m) = event.code {
                self.strip_modifiers(mapping::modifier(m).2);
            }
            self.sync_modifiers(emulate_modifiers, frame, false, out);
            return;
        }
        let source = match event.code {
            TermKey::Modifier(m) => Source::Native(mapping::modifier(m).2),
            _ => Source::Key(flags),
        };
        let timed = emulate_release;
        if let Some(d) = self.0.iter_mut().find(|d| same_key(&d.press, &press)) {
            // A real left modifier can take ownership of its synthetic press.
            let promoted = matches!(d.source, Source::Synthetic(_));
            d.source = source;
            d.seen = frame.number;
            d.at = frame.now;
            d.timed = timed;
            if promoted {
                return;
            }
            press.repeat = true;
            press.logical_key = d.press.logical_key.clone();
        } else {
            if event.kind == KeyEventKind::Press && matches!(source, Source::Key(_)) {
                self.release(|d| d.timed && matches!(d.source, Source::Key(_)), out);
            }
            self.0.push(frame.down(press.clone(), source, timed));
        }
        self.sync_modifiers(emulate_modifiers, frame, false, out);
        out.push(press);
    }
}

#[derive(Default)]
pub(super) struct Keyboard {
    held: Held,
    pending: VecDeque<KeyEvent>,
    frame: u64,
    policy: EmulationPolicy,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn forward_keys(
    mut keyboard: Local<Keyboard>,
    mut keys: MessageReader<KeyMessage>,
    mut focus: MessageReader<FocusMessage>,
    mut output: MessageWriter<KeyboardInput>,
    #[cfg(not(feature = "windowed"))] mut focus_lost: MessageWriter<KeyboardFocusLost>,
    window: Res<TerminalWindow>,
    time: Res<Time<Real>>,
    policy: Res<EmulationPolicy>,
    timer: Res<ReleaseKey>,
    mut detected: ResMut<Detected>,
) {
    let mut lost = false;
    for msg in focus.read() {
        lost |= *msg == FocusMessage::Lost;
    }

    keyboard.frame = keyboard.frame.wrapping_add(1);
    let frame = Frame {
        number: keyboard.frame,
        now: time.elapsed(),
        window: window.0,
    };
    let policy_changed = keyboard.policy != *policy;
    keyboard.policy = *policy;
    if policy_changed
        && policy
            .emulate_capabilities(&detected)
            .contains(Capability::KEY_RELEASE)
    {
        for d in &mut keyboard.held.0 {
            d.timed = !matches!(d.source, Source::Synthetic(_));
        }
    }

    let emulate_modifiers = policy.emulate_modifiers();
    let pending = std::mem::take(&mut keyboard.pending);
    let flush = lost || pending.len().saturating_add(keys.len()) > MAX_PENDING;
    let mut events = pending.into_iter().chain(keys.read().map(|m| m.0));
    let mut edges = Vec::new();
    let mut out: Vec<KeyboardInput> = Vec::new();
    let mut staged = Held::default();
    while let Some(event) = events.next() {
        let mut observed = detected.0;
        if event.kind != KeyEventKind::Press {
            observed.insert(Capability::KEY_RELEASE);
        }
        if matches!(event.code, TermKey::Modifier(_)) {
            observed.insert(Capability::MODIFIER);
        }
        let next_detected = Detected(observed);
        let emulate_release = policy
            .emulate_capabilities(&next_detected)
            .contains(Capability::KEY_RELEASE);

        // Stage the entire event, including implicit modifier and supersession
        // edges. Deferring only its final release would split a chord in half.
        if !flush && !out.is_empty() {
            staged.0.clone_from(&keyboard.held.0);
            staged.apply(event, emulate_release, emulate_modifiers, frame, &mut edges);
            let hides_edge = edges.iter().any(|edge| match edge.state {
                ButtonState::Released => keyboard
                    .held
                    .0
                    .iter()
                    .any(|d| d.since == frame.number && same_key(&d.press, edge)),
                ButtonState::Pressed => {
                    !edge.repeat
                        && out
                            .iter()
                            .any(|up| up.state == ButtonState::Released && same_key(up, edge))
                }
            });
            if hides_edge {
                keyboard.pending.push_back(event);
                keyboard.pending.extend(events);
                break;
            }
            std::mem::swap(&mut staged, &mut keyboard.held);
        } else {
            keyboard
                .held
                .apply(event, emulate_release, emulate_modifiers, frame, &mut edges);
        }
        detected.set_if_neq(next_detected);
        if flush {
            output.write_batch(edges.drain(..));
        } else {
            out.append(&mut edges);
        }
    }

    let emulate_release = policy
        .emulate_capabilities(&detected)
        .contains(Capability::KEY_RELEASE);
    let stripped = keyboard.held.release(
        |d| {
            d.timed
                && d.seen != frame.number
                && ((policy_changed && !emulate_release)
                    || match *timer {
                        ReleaseKey::Immediate => true,
                        ReleaseKey::FrameCount(n) => {
                            frame.number.wrapping_sub(d.seen) >= u64::from(n)
                        }
                        ReleaseKey::Duration(delay) => frame.now.saturating_sub(d.at) >= delay,
                        ReleaseKey::OnNextKey => false,
                    })
        },
        &mut out,
    );
    keyboard.held.strip_modifiers(stripped);
    keyboard
        .held
        .sync_modifiers(emulate_modifiers, frame, true, &mut out);
    if lost {
        keyboard
            .held
            .release(|d| matches!(d.source, Source::Key(_)), &mut out);
        keyboard.held.release(|_| true, &mut out);
    }
    output.write_batch(out);
    #[cfg(not(feature = "windowed"))]
    if lost {
        focus_lost.write(KeyboardFocusLost);
    }
}
