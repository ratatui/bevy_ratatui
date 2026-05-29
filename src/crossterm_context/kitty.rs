//! Enhanced kitty keyboard protocol.
use std::io::{self, stdout};

use bevy::prelude::*;
use ratatui::crossterm::{
    ExecutableCommand,
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    terminal::supports_keyboard_enhancement,
};

use crate::ContextSystems;

/// Plugin responsible for enabling the Kitty keyboard protocol in the current buffer.
///
/// Provides additional information involving keyboard events. For example, key release events will
/// be reported.
///
/// Refer to the above link for a list of terminals that support the protocol. An `Ok` result is not
/// a guarantee that all features are supported: you should have fallbacks that you use until you
/// detect the event type you are looking for.
///
/// [kitty keyboard protocol]: https://sw.kovidgoyal.net/kitty/keyboard-protocol/
pub struct KittyPlugin;

impl Plugin for KittyPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(Startup, kitty_setup.in_set(ContextSystems::PostSetup));
    }
}

fn kitty_setup(mut commands: Commands) {
    if enable_kitty_protocol().is_ok() {
        commands.insert_resource(KittyEnabled);
    }
}

/// A resource indicating that the Kitty keyboard protocol was successfully enabled in the current
/// buffer.
#[derive(Resource)]
pub struct KittyEnabled;

impl Drop for KittyEnabled {
    fn drop(&mut self) {
        let _ = disable_kitty_protocol();
    }
}

/// Enables support for the Kitty keyboard protocol.
///
/// See [KittyPlugin].
pub fn enable_kitty_protocol() -> io::Result<()> {
    if supports_keyboard_enhancement()? {
        stdout().execute(PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::all()))?;
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Kitty keyboard protocol is not supported by this terminal.",
    ))
}

/// Disables the Kitty keyboard protocol, restoring the buffer to normal.
///
/// See [KittyPlugin].
pub fn disable_kitty_protocol() -> io::Result<()> {
    stdout().execute(PopKeyboardEnhancementFlags)?;
    Ok(())
}
