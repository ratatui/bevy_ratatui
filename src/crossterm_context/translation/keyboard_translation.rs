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
struct Stamp(u64, Duration);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Release {
    Terminal,
    Timer,
}

#[derive(Clone, Copy)]
enum Role {
    Key(Mods, Release),
    Native(Mods, Release),
    Synthetic(Mods),
}

impl Role {
    fn modifier_bit(self) -> Mods {
        match self {
            Self::Native(bit, _) | Self::Synthetic(bit) => bit,
            Self::Key(..) => Mods::empty(),
        }
    }

    fn release_mut(&mut self) -> Option<&mut Release> {
        match self {
            Self::Key(_, r) | Self::Native(_, r) => Some(r),
            Self::Synthetic(_) => None,
        }
    }
}

#[derive(Clone)]
struct Down {
    press: KeyboardInput,
    role: Role,
    since: u64,
    seen: Stamp,
}

#[derive(Clone, Copy)]
struct Frame {
    stamp: Stamp,
    window: Entity,
    emulate_modifiers: bool,
}

impl Frame {
    fn down(self, press: KeyboardInput, role: Role) -> Down {
        Down {
            press,
            role,
            since: self.stamp.0,
            seen: self.stamp,
        }
    }
}

impl ReleaseKey {
    fn expired(self, seen: Stamp, now: Stamp) -> bool {
        match self {
            Self::Immediate => true,
            Self::FrameCount(n) => now.0.wrapping_sub(seen.0) >= u64::from(n),
            Self::Duration(delay) => now.1.saturating_sub(seen.1) >= delay,
            Self::OnNextKey => false,
        }
    }
}

fn same_key(a: &KeyboardInput, b: &KeyboardInput) -> bool {
    if a.key_code != b.key_code {
        return false;
    }
    match a.key_code {
        KeyCode::MediaPlayPause => a.logical_key == b.logical_key,
        KeyCode::Unidentified(_) => match (&a.logical_key, &b.logical_key) {
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
        },
        _ => true,
    }
}

#[derive(Default)]
struct Held(Vec<Down>);

impl Held {
    /// Releases the selected holds and strips the flags of released native modifiers.
    fn release(&mut self, select: impl Fn(&mut Down) -> bool, out: &mut Vec<KeyboardInput>) {
        let mut stripped = Mods::empty();
        for d in self.0.extract_if(.., select) {
            if let Role::Native(bit, _) = d.role {
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
            if let Role::Key(flags, _) = &mut d.role {
                flags.remove(stripped);
            }
        }
    }

    /// Whether `edge` would hide a press that this frame has published in `out`.
    fn hides(&self, edge: &KeyboardInput, out: &[KeyboardInput], frame: u64) -> bool {
        let new_chord_uses = |bit: Mods| {
            self.0.iter().any(|d| {
                d.since == frame && matches!(d.role, Role::Key(flags, _) if flags.intersects(bit))
            })
        };
        match edge.state {
            ButtonState::Released => self.0.iter().any(|d| {
                same_key(&d.press, edge)
                    && (d.since == frame || new_chord_uses(d.role.modifier_bit()))
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
            match d.role {
                Role::Key(bits, _) => flags |= bits,
                Role::Native(bit, _) => native |= bit,
                Role::Synthetic(bit) => synthetic |= bit,
            }
        }
        let wanted = if frame.emulate_modifiers {
            flags - native
        } else {
            Mods::empty()
        };
        self.release(
            |d| {
                matches!(d.role, Role::Synthetic(bit) if !wanted.contains(bit))
                    && !(preserve_new && d.since == frame.stamp.0)
            },
            out,
        );
        let missing = wanted - synthetic;
        for m in mapping::LEFT_MODIFIERS {
            let bit = mapping::modifier(m).2;
            if missing.contains(bit) {
                let (press, _) =
                    mapping::map(KeyEvent::new(TermKey::Modifier(m), bit), frame.window);
                self.0.push(frame.down(press.clone(), Role::Synthetic(bit)));
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
                |d| !matches!(d.role, Role::Synthetic(_)) && same_key(&d.press, &press),
                out,
            );
            if let TermKey::Modifier(m) = event.code {
                self.strip_modifiers(mapping::modifier(m).2);
            }
            self.sync_modifiers(frame, false, out);
            return;
        }
        let release = if emulate_release {
            Release::Timer
        } else {
            Release::Terminal
        };
        let role = match event.code {
            TermKey::Modifier(m) => Role::Native(mapping::modifier(m).2, release),
            _ => Role::Key(flags, release),
        };
        // Any key carrying modifier flags keeps the real modifiers it uses alive.
        for d in &mut self.0 {
            if matches!(d.role, Role::Native(bit, _) if flags.intersects(bit)) {
                d.seen = frame.stamp;
            }
        }
        if let Some(d) = self.0.iter_mut().find(|d| same_key(&d.press, &press)) {
            // A real left modifier can take ownership of its synthetic press.
            let promoted = matches!(d.role, Role::Synthetic(_));
            d.role = role;
            d.seen = frame.stamp;
            if promoted {
                return;
            }
            press.repeat = true;
            press.logical_key = d.press.logical_key.clone();
        } else {
            if emulate_release && event.kind == KeyEventKind::Press && matches!(role, Role::Key(..))
            {
                self.release(
                    |d| match d.role {
                        Role::Key(_, Release::Timer) => true,
                        Role::Native(bit, Release::Timer) => !flags.intersects(bit),
                        _ => false,
                    },
                    out,
                );
            }
            self.0.push(frame.down(press.clone(), role));
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
    let stamp = Stamp(keyboard.frame, time.elapsed());
    let frame = Frame {
        stamp,
        window: window.0,
        emulate_modifiers: policy.emulate_modifiers(),
    };
    let held = &mut keyboard.held;
    if std::mem::replace(&mut keyboard.policy, *policy) != *policy
        && policy.emulates_release(*detected)
    {
        for d in &mut held.0 {
            if let Some(release) = d.role.release_mut() {
                *release = Release::Timer;
            }
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
        if stage && edges.iter().any(|edge| staged.hides(edge, &out, stamp.0)) {
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
                || matches!(d.role, Role::Native(bit, _) if flags.intersects(bit))
        })
    };
    held.release(
        |d| {
            let seen = d.seen;
            let Some(release) = d.role.release_mut() else {
                return false;
            };
            if *release != Release::Timer {
                return false;
            }
            if !emulate_release {
                if !retire_timers {
                    *release = Release::Terminal;
                    return false;
                }
                // Keep a legacy hold refreshed this frame alive for one frame, then retire it.
                return seen.0 != stamp.0;
            }
            seen.0 != stamp.0 && timer.expired(seen, stamp) && !queued(d)
        },
        &mut out,
    );
    held.sync_modifiers(frame, true, &mut out);
    if lost {
        held.release(|d| matches!(d.role, Role::Key(..)), &mut out);
        held.release(|_| true, &mut out);
    }
    output.write_batch(out);
    #[cfg(not(feature = "windowed"))]
    if lost {
        focus_lost.write(KeyboardFocusLost);
    }
}
