use std::marker::PhantomData;

use bevy::prelude::*;

use crate::{RatatuiContext, context::TerminalContext};

use super::kitty::KittyEnabled;

#[cfg(feature = "mouse")]
use super::mouse::MouseEnabled;

/// Plugin responsible for cleaning up resources in the correct order when exiting.
///
/// If raw mode, the alternate view, and the Kitty protocol are disabled in the wrong order, it can
/// cause issues for the terminal buffer after the application exits.
pub struct CleanupPlugin<C: TerminalContext = crate::context::CrosstermContext>(
    PhantomData<fn() -> C>,
);

impl<C: TerminalContext> Default for CleanupPlugin<C> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<C: TerminalContext> Plugin for CleanupPlugin<C> {
    fn build(&self, app: &mut App) {
        app.add_systems(Last, cleanup::<C>);
    }
}

fn cleanup<C: TerminalContext>(mut exit: MessageReader<AppExit>, mut commands: Commands) {
    for _ in exit.read() {
        commands.remove_resource::<KittyEnabled>();
        #[cfg(feature = "mouse")]
        commands.remove_resource::<MouseEnabled>();
        commands.remove_resource::<RatatuiContext<C>>();
    }
}
