//! Error handling for the app.
//!
//! This module provides a plugin that sets up error handling for the app. It installs hooks for
//! panic and error handling that restore the terminal before printing the panic or error message.
//! This ensures that the error message is not messed up by the terminal state.
use std::marker::PhantomData;
use std::panic;

use bevy::prelude::*;
use color_eyre::{
    self,
    config::{EyreHook, HookBuilder, PanicHook},
    eyre,
};

use crate::{RatatuiContext, context::TerminalContext};

/// A plugin that sets up error handling.
///
/// This plugin installs hooks for panic and error handling that restore the terminal before
/// printing the panic or error message. This ensures that the error message is not messed up by the
/// terminal state.
pub struct ErrorPlugin<C: TerminalContext = crate::context::CrosstermContext>(
    PhantomData<fn() -> C>,
);

impl<C: TerminalContext> Default for ErrorPlugin<C> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<C: TerminalContext> Plugin for ErrorPlugin<C> {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(Startup, error_setup::<C>);
    }
}

/// Installs hooks for panic and error handling.
///
/// Makes the app resilient to panics and errors by restoring the terminal before printing the
/// panic or error message. This prevents error messages from being messed up by the terminal
/// state.
pub fn error_setup<C: TerminalContext>() -> Result {
    let (panic_hook, eyre_hook) = HookBuilder::default().into_hooks();
    set_panic_hook::<C>(panic_hook);
    set_error_hook::<C>(eyre_hook)?;

    Ok(())
}

/// Install a panic hook that restores the terminal before printing the panic.
fn set_panic_hook<C: TerminalContext>(panic_hook: PanicHook) {
    let panic_hook = panic_hook.into_panic_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = RatatuiContext::<C>::restore();
        panic_hook(panic_info);
    }));
}

/// Install an error hook that restores the terminal before printing the error.
fn set_error_hook<C: TerminalContext>(eyre_hook: EyreHook) -> Result {
    let eyre_hook = eyre_hook.into_eyre_hook();
    eyre::set_hook(Box::new(move |error| {
        let _ = RatatuiContext::<C>::restore();
        eyre_hook(error)
    }))?;

    Ok(())
}
