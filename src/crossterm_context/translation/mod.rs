//! Forward terminal keyboard input with [`RatatuiPlugins::enable_input_forwarding`](crate::RatatuiPlugins::enable_input_forwarding).
//! In 0.14, both physical and logical [`ButtonInput`](bevy::input::ButtonInput) are ready in
//! [`InputSet::Post`] and `Update` on the frame the terminal reports a press. Read
//! [`KeyboardInput`](bevy::input::keyboard::KeyboardInput) for individual transitions and repeats.
//! Raw [`KeyMessage`](crate::event::KeyMessage)s remain available.
//!
//! A terminal tap received in one poll normally spans two updates so state-based consumers
//! see the key down. Focus loss flushes deferred taps, then releases all held keys, including
//! emulated modifiers. Kitty sessions start with native key releases enabled. Other sessions
//! learn from input.
//!
//! Under [`ReleaseKey::Immediate`], legacy terminals report a held key only as OS autorepeat
//! presses, so it toggles up and down between repeat ticks. Use a [`ReleaseKey::Duration`]
//! longer than the repeat delay for a sustained hold. Crossterm key events carry no scan codes,
//! so physical positions are inferred from a US layout and shifted symbols on other layouts may
//! map to the wrong [`KeyCode`].
//!
//! # Migrating to 0.14
//!
//! Emulated releases now default to [`ReleaseKey::Immediate`]. Insert
//! `ReleaseKey::Duration(Duration::from_secs(1))` to restore the previous timeout.
//! Replace checks for the former marker resource with `!policy.emulate_capabilities(&detected).is_empty()`.
//! Move systems that depended on the old detection phase to [`InputSet::Post`].

mod keyboard_translation;
mod mapping;

use crate::{
    context::CrosstermContext,
    event::{FocusMessage, InputSet, KeyMessage},
};
use bevy::{
    input::{InputPlugin, InputSystems},
    prelude::*,
    time::TimePlugin,
};
use std::time::Duration;

/// Translates terminal keys into Bevy keyboard input during `PreUpdate`.
pub struct TranslationPlugin;

#[derive(Resource)]
struct TerminalWindow(Entity);

impl Plugin for TranslationPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<InputPlugin>() {
            app.add_plugins(InputPlugin);
        }
        if !app.is_plugin_added::<TimePlugin>() {
            app.add_plugins(TimePlugin);
        }
        let window = app.world_mut().spawn_empty().id();
        app.insert_resource(TerminalWindow(window))
            .add_message::<KeyMessage>()
            .add_message::<FocusMessage>()
            .init_resource::<ReleaseKey>()
            .init_resource::<Detected>()
            .init_resource::<EmulationPolicy>()
            .add_systems(PostStartup, configure_session)
            .configure_sets(
                PreUpdate,
                (
                    InputSet::EmitBevy.before(InputSystems),
                    InputSet::Post.after(InputSystems),
                ),
            )
            .add_systems(
                PreUpdate,
                keyboard_translation::forward_keys.in_set(InputSet::EmitBevy),
            );
    }
}

fn configure_session(context: Option<Res<CrosstermContext>>, mut detected: ResMut<Detected>) {
    if let Some(context) = context {
        if context.kitty_enabled() {
            detected.0.insert(Capability::KEY_RELEASE);
        }
        // Focus reporting is optional; terminals without it simply never lose focus.
        let _ = context.enable_focus_reporting();
    }
}

bitflags::bitflags! {
    /// Keyboard capabilities the terminal can report or the plugin can emulate.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Capability: u8 {
        /// Explicit release and repeat events.
        const KEY_RELEASE = 1;
        /// Standalone modifier key events.
        const MODIFIER = 2;
        const ALL = Self::KEY_RELEASE.bits() | Self::MODIFIER.bits();
    }
}

/// Capabilities observed in this terminal session.
#[derive(Debug, Resource, Default, Deref, Clone, Copy, PartialEq, Eq)]
pub struct Detected(pub Capability);

/// Selects the keyboard capabilities to emulate.
#[derive(Debug, Default, Resource, Clone, Copy, PartialEq, Eq)]
pub enum EmulationPolicy {
    /// Emulate any capability not yet detected, while still supplementing
    /// flag-only modifier chords when no matching native modifier key is held.
    /// That chord synthesis continues even once [`Detected`] is
    /// [`Capability::ALL`] and [`Self::emulate_capabilities`] reports nothing.
    #[default]
    Automatic,
    /// Emulate the selected capabilities.
    Manual(Capability),
}

impl EmulationPolicy {
    /// Capabilities emulated for the given [`Detected`] set.
    pub fn emulate_capabilities(&self, detected: &Detected) -> Capability {
        match self {
            Self::Automatic => !detected.0,
            Self::Manual(caps) => *caps,
        }
    }

    pub(super) fn emulates_release(self, detected: Detected) -> bool {
        self.emulate_capabilities(&detected)
            .contains(Capability::KEY_RELEASE)
    }

    /// Whether chord modifier flags may synthesize missing left-hand modifiers.
    /// Under [`Self::Automatic`], this stays enabled even after native modifier
    /// keys have been seen so flag-only chords still work without duplicating
    /// any currently held native modifier.
    pub(super) fn emulate_modifiers(self) -> bool {
        match self {
            Self::Automatic => true,
            Self::Manual(caps) => caps.contains(Capability::MODIFIER),
        }
    }
}

/// When to release a key on terminals without native releases. A different
/// key press supersedes the previous timer-managed non-modifier key.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReleaseKey {
    /// Release after this much real time since the last press or repeat.
    Duration(Duration),
    /// Release after this many frames, with a minimum of one.
    FrameCount(u32),
    /// Release on the next frame unless refreshed by another press or repeat.
    #[default]
    Immediate,
    /// Hold until the next key press or focus loss.
    OnNextKey,
}
