//! Event handling.
//!
//! This module provides a plugin for handling events, and a wrapper around
//! `crossterm::event::KeyEvent`.
//!
//! # Example
//!
//! ```rust
//! use bevy::{app::AppExit, prelude::*};
//! use bevy_ratatui::event::KeyMessage;
//! use ratatui::crossterm::event::KeyCode;
//!
//! fn keyboard_input_system(mut messages: MessageReader<KeyMessage>, mut exit: MessageWriter<AppExit>) {
//!     for message in messages.read() {
//!         match message.code {
//!             KeyCode::Char('q') | KeyCode::Esc => {
//!                 exit.write_default();
//!             }
//!             _ => {}
//!         }
//!     }
//! }
//! ```
use std::time::Duration;

use bevy::{app::AppExit, prelude::*};
use ratatui::crossterm::event::{self, Event::Key, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Size;

/// A plugin for handling events.
///
/// This plugin reads events from the terminal environment and forwards them as Bevy messages using the
/// `KeyMessage` message.
pub struct EventPlugin {
    /// Adds an input handler that signals bevy to exit when an interrupt keypress (control+c) is read.
    pub control_c_interrupt: bool,
}

impl Default for EventPlugin {
    fn default() -> Self {
        Self {
            control_c_interrupt: true,
        }
    }
}

impl Plugin for EventPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_message::<KeyMessage>()
            .add_message::<MouseMessage>()
            .add_message::<FocusMessage>()
            .add_message::<ResizeMessage>()
            .add_message::<PasteMessage>()
            .add_message::<CrosstermMessage>()
            .configure_sets(
                Update,
                (
                    InputSet::Pre,
                    InputSet::EmitCrossterm,
                    InputSet::CheckEmulation,
                    InputSet::EmitBevy,
                    InputSet::Post,
                )
                    .chain(),
            )
            .add_systems(
                PreUpdate,
                crossterm_event_system.in_set(InputSet::EmitCrossterm),
            );

        if self.control_c_interrupt {
            app.add_systems(Update, control_c_interrupt_system.in_set(InputSet::Post));
        }
    }
}

/// InputSet defines when the input messages are emitted.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum InputSet {
    /// Run before any input messages are emitted.
    Pre,
    /// Emit the crossterm messages.
    EmitCrossterm,
    /// Check for emulation
    CheckEmulation,
    /// Emit the bevy messages if [crate::translation::TranslationPlugin] has been added.
    EmitBevy,
    /// Run after all input messages are emitted.
    Post,
}

/// A message that is sent whenever an event is read from crossterm.
#[derive(Message, Deref, Clone, PartialEq, Eq, Hash, Debug)]
pub struct CrosstermMessage(pub event::Event);

/// A message that is sent whenever a key event is read from crossterm.
#[derive(Message, Deref, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct KeyMessage(pub event::KeyEvent);

/// A message that is sent whenever a mouse event is read from crossterm.
#[derive(Message, Deref, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MouseMessage(pub event::MouseEvent);

/// A message that is sent when the terminal gains or loses focus.
#[derive(Message, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FocusMessage {
    Gained,
    Lost,
}

/// An event that is sent when the terminal is resized.
#[derive(Message, Deref, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ResizeMessage(pub Size);

/// An event that is sent when text is pasted into the terminal.
#[derive(Message, Deref, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PasteMessage(pub String);

/// System that reads events from crossterm and forwards them as Bevy messages.
pub fn crossterm_event_system(
    mut messages: MessageWriter<CrosstermMessage>,
    mut keys: MessageWriter<KeyMessage>,
    mut mouse: MessageWriter<MouseMessage>,
    mut focus: MessageWriter<FocusMessage>,
    mut paste: MessageWriter<PasteMessage>,
    mut resize: MessageWriter<ResizeMessage>,
) -> Result {
    while event::poll(Duration::ZERO)? {
        let event = event::read()?;
        match event {
            Key(event) => {
                keys.write(KeyMessage(event));
            }
            event::Event::FocusLost => {
                focus.write(FocusMessage::Lost);
            }
            event::Event::FocusGained => {
                focus.write(FocusMessage::Gained);
            }
            event::Event::Mouse(event) => {
                mouse.write(MouseMessage(event));
            }
            event::Event::Paste(ref s) => {
                paste.write(PasteMessage(s.clone()));
            }
            event::Event::Resize(columns, rows) => {
                resize.write(ResizeMessage(Size::new(columns, rows)));
            }
        }
        messages.write(CrosstermMessage(event));
    }
    Ok(())
}

/// System that sends an `AppExit` message when `Ctrl+C` is pressed.
fn control_c_interrupt_system(
    mut key_messages: MessageReader<KeyMessage>,
    mut exit: MessageWriter<AppExit>,
) {
    for message in key_messages.read() {
        if message.kind == KeyEventKind::Press
            && message.modifiers == KeyModifiers::CONTROL
            && message.code == KeyCode::Char('c')
        {
            exit.write_default();
        }
    }
}
