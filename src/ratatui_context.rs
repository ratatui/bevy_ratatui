use bevy::prelude::*;

use super::context_trait::TerminalContext;

#[cfg(all(feature = "crossterm", not(feature = "windowed")))]
pub type DefaultContext = crate::context::CrosstermContext;

#[cfg(feature = "windowed")]
pub type DefaultContext = crate::context::WindowedContext;

/// A bevy Resource that wraps [ratatui::Terminal], setting up the terminal context when
/// initialized (i.e. entering raw mode), restores the prior terminal state when dropped (i.e.
/// exiting raw mode), and can be brought into Bevy systems to interact with Ratatui. For example,
/// use this resource to draw to the terminal each frame, like the below example.
///
/// # Example
///
/// ```rust
/// use bevy::prelude::*;
/// use bevy_ratatui::RatatuiContext;
///
/// fn draw_system(mut context: ResMut<RatatuiContext>) {
///     context.draw(|frame| {
///         // Draw widgets etc. to the terminal
///     });
/// }
/// ```
#[derive(Resource, Deref, DerefMut)]
pub struct RatatuiContext<C: TerminalContext = DefaultContext>(pub C);

impl<C: TerminalContext> Drop for RatatuiContext<C> {
    fn drop(&mut self) {
        if let Err(err) = C::restore() {
            eprintln!("Failed to restore terminal: {}", err);
        }
    }
}

impl<C: TerminalContext> RatatuiContext<C> {
    pub fn init() -> Result<Self> {
        Ok(Self(C::init()?))
    }

    pub fn restore() -> Result {
        C::restore()
    }
}
