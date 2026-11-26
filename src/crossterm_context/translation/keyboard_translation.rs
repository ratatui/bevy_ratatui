use std::time::Duration;

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
    seen: Frame,
    timed: bool,
}

#[derive(Clone, Copy)]
struct Frame {
    number: u64,
    now: Duration,
    window: Entity,
    emulate_modifiers: bool,
}

impl Frame {
    fn down(self, press: KeyboardInput, source: Source, timed: bool) -> Down {
        Down {
            press,
            source,
            since: self.number,
            seen: self,
            timed,
        }
    }
}

impl ReleaseKey {
    fn expired(self, d: &Down, frame: Frame) -> bool {
        match self {
            Self::Immediate => true,
            Self::FrameCount(n) => frame.number.wrapping_sub(d.seen.number) >= u64::from(n),
            Self::Duration(delay) => frame.now.saturating_sub(d.seen.now) >= delay,
            Self::OnNextKey => false,
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
            // Lower then upper also unifies final sigma and sharp S.
            fn folded(s: &str) -> impl Iterator<Item = char> {
                s.chars()
                    .flat_map(char::to_lowercase)
                    .flat_map(char::to_uppercase)
            }
            a == b || folded(a).eq(folded(b))
        }
        (a, b) => a == b,
    }
}

#[derive(Default)]
struct Held(Vec<Down>);

impl Held {
    /// Releases the selected holds and strips the flags of released native modifiers.
    fn release(&mut self, select: impl Fn(&mut Down) -> bool, out: &mut Vec<KeyboardInput>) {
        let mut stripped = Mods::empty();
        for d in self.0.extract_if(.., select) {
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
        self.strip_modifiers(stripped);
    }

    fn strip_modifiers(&mut self, stripped: Mods) {
        for d in &mut self.0 {
            if let Source::Key(flags) = &mut d.source {
                flags.remove(stripped);
            }
        }
    }

    /// Whether `edge` would hide a press that this frame has published in `out`.
    fn hides(&self, edge: &KeyboardInput, out: &[KeyboardInput], frame: u64) -> bool {
        let new_chord_uses = |bit: Mods| {
            self.0.iter().any(|d| {
                d.since == frame && matches!(d.source, Source::Key(flags) if flags.intersects(bit))
            })
        };
        match edge.state {
            ButtonState::Released => self.0.iter().any(|d| {
                same_key(&d.press, edge)
                    && (d.since == frame
                        || matches!(d.source, Source::Native(bit) if new_chord_uses(bit)))
            }),
            ButtonState::Pressed => {
                !edge.repeat
                    && out
                        .iter()
                        .any(|up| up.state == ButtonState::Released && same_key(up, edge))
            }
        }
    }

    fn sync_modifiers(&mut self, frame: Frame, preserve_new: bool, out: &mut Vec<KeyboardInput>) {
        let (mut flags, mut native, mut synthetic) = (Mods::empty(), Mods::empty(), Mods::empty());
        for d in &self.0 {
            match d.source {
                Source::Key(bits) => flags |= bits,
                Source::Native(bit) => native |= bit,
                Source::Synthetic(bit) => synthetic |= bit,
            }
        }
        let wanted = if frame.emulate_modifiers {
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
        for m in mapping::LEFT_MODIFIERS {
            let bit = mapping::modifier(m).2;
            if missing.contains(bit) {
                let (press, _) =
                    mapping::map(KeyEvent::new(TermKey::Modifier(m), bit), frame.window);
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
            self.sync_modifiers(frame, false, out);
            return;
        }
        let source = match event.code {
            TermKey::Modifier(m) => Source::Native(mapping::modifier(m).2),
            _ => Source::Key(flags),
        };
        // Any key carrying modifier flags keeps the real modifiers it uses alive.
        for d in &mut self.0 {
            if matches!(d.source, Source::Native(bit) if flags.intersects(bit)) {
                d.seen = frame;
            }
        }
        if let Some(d) = self.0.iter_mut().find(|d| same_key(&d.press, &press)) {
            // A real left modifier can take ownership of its synthetic press.
            let promoted = matches!(d.source, Source::Synthetic(_));
            d.source = source;
            d.seen = frame;
            d.timed = emulate_release;
            if promoted {
                return;
            }
            press.repeat = true;
            press.logical_key = d.press.logical_key.clone();
        } else {
            if emulate_release
                && event.kind == KeyEventKind::Press
                && matches!(source, Source::Key(_))
            {
                self.release(|d| d.timed && matches!(d.source, Source::Key(_)), out);
            }
            self.0
                .push(frame.down(press.clone(), source, emulate_release));
        }
        self.sync_modifiers(frame, false, out);
        out.push(press);
    }
}

#[derive(Default)]
pub(super) struct Keyboard {
    held: Held,
    pending: Vec<KeyEvent>,
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

    let keyboard = &mut *keyboard;
    keyboard.frame = keyboard.frame.wrapping_add(1);
    let frame = Frame {
        number: keyboard.frame,
        now: time.elapsed(),
        window: window.0,
        emulate_modifiers: policy.emulate_modifiers(),
    };
    let held = &mut keyboard.held;
    if std::mem::replace(&mut keyboard.policy, *policy) != *policy
        && policy.emulates_release(*detected)
    {
        for d in &mut held.0 {
            d.timed = !matches!(d.source, Source::Synthetic(_));
        }
    }

    let pending = std::mem::take(&mut keyboard.pending);
    let flush = lost || pending.len().saturating_add(keys.len()) > MAX_PENDING;
    let mut events = pending.into_iter().chain(keys.read().map(|m| m.0));
    let mut edges = Vec::new();
    // Stays empty while flushing, so nothing is staged or deferred then.
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
        // Stage the entire event, including implicit modifier and supersession
        // edges. Deferring only its final release would split a chord in half.
        let stage = !out.is_empty();
        if stage {
            staged.0.clone_from(&held.0);
        }
        held.apply(
            event,
            policy.emulates_release(Detected(observed)),
            frame,
            &mut edges,
        );
        if stage
            && edges
                .iter()
                .any(|edge| staged.hides(edge, &out, frame.number))
        {
            std::mem::swap(&mut staged, held);
            keyboard.pending.push(event);
            keyboard.pending.extend(events);
            break;
        }
        detected.set_if_neq(Detected(observed));
        if flush {
            output.write_batch(edges.drain(..));
        } else {
            out.append(&mut edges);
        }
    }

    let emulate_release = policy.emulates_release(*detected);
    let retire_timers = !detected.contains(Capability::KEY_RELEASE);
    let queued = |d: &Down| {
        keyboard.pending.iter().any(|&e| {
            if e.kind == KeyEventKind::Release {
                return false;
            }
            let (mapped, flags) = mapping::map(e, frame.window);
            same_key(&mapped, &d.press)
                || matches!(d.source, Source::Native(bit) if flags.intersects(bit))
        })
    };
    held.release(
        |d| {
            if !emulate_release {
                // Hand the hold to the terminal unless a legacy policy switch needs cleanup.
                return std::mem::take(&mut d.timed)
                    && retire_timers
                    && d.seen.number != frame.number;
            }
            d.timed && d.seen.number != frame.number && timer.expired(d, frame) && !queued(d)
        },
        &mut out,
    );
    held.sync_modifiers(frame, true, &mut out);
    if lost {
        held.release(|d| matches!(d.source, Source::Key(_)), &mut out);
        held.release(|_| true, &mut out);
    }
    output.write_batch(out);
    #[cfg(not(feature = "windowed"))]
    if lost {
        focus_lost.write(KeyboardFocusLost);
    }
}
