use std::{
    io::{self, Stdout, Write, stderr, stdout},
    panic,
    sync::{
        Mutex, MutexGuard, Once,
        atomic::{AtomicU64, Ordering},
    },
};

use bevy::prelude::*;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        ExecutableCommand, cursor,
        event::{
            KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
        },
        terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    },
};

#[cfg(feature = "mouse")]
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};

static ACTIVE: Mutex<Option<Session>> = Mutex::new(None);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn active() -> MutexGuard<'static, Option<Session>> {
    ACTIVE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Session {
    id: u64,
    raw: bool,
    kitty: bool,
    #[cfg(feature = "mouse")]
    mouse: bool,
}

impl Session {
    /// Hold `ACTIVE` across teardown so a racing panic hook, `Drop`, or `init`
    /// cannot overtake an in-progress restore.
    fn finish_if(
        matches: impl FnOnce(&Session) -> bool,
        terminal: Option<&mut Terminal<CrosstermBackend<Stdout>>>,
    ) -> io::Result<()> {
        let mut guard = active();
        let Some(session) = guard.take_if(|s| matches(s)) else {
            return Ok(());
        };
        session.teardown(terminal)
    }

    fn teardown(self, terminal: Option<&mut Terminal<CrosstermBackend<Stdout>>>) -> io::Result<()> {
        let mut out = stdout();
        let mut first_err = None;
        let mut record = |res: io::Result<()>| {
            if let Err(err) = res {
                first_err.get_or_insert(err);
            }
        };

        if self.kitty {
            record(out.execute(PopKeyboardEnhancementFlags).map(drop));
        }
        #[cfg(feature = "mouse")]
        if self.mouse {
            record(out.execute(DisableMouseCapture).map(drop));
        }
        record(out.execute(LeaveAlternateScreen).map(drop));
        // Clearing Ratatui's `hidden_cursor` here avoids a duplicate `Show` on normal drop.
        record(match terminal {
            Some(terminal) => terminal.show_cursor(),
            None => out.execute(cursor::Show).map(drop),
        });
        if self.raw {
            record(terminal::disable_raw_mode());
        }
        first_err.map_or(Ok(()), Err)
    }
}

/// Ratatui context that draws to the terminal using crossterm and restores it at most once.
#[derive(Resource, Deref, DerefMut, Debug)]
pub struct CrosstermContext {
    #[deref]
    terminal: Terminal<CrosstermBackend<Stdout>>,
    id: u64,
}

impl CrosstermContext {
    /// Start a terminal session, installing a process-wide panic hook before changing modes.
    pub fn init() -> Result<Self> {
        if std::thread::panicking() {
            return Err("cannot initialize a terminal while panicking".into());
        }
        install_hook();
        if active().is_some() {
            return Err("a crossterm session is already active".into());
        }
        let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        let mut guard = active();
        if guard.is_some() {
            return Err("a crossterm session is already active".into());
        }
        let raw = !terminal::is_raw_mode_enabled()?;
        if raw {
            terminal::enable_raw_mode()?;
        }
        if let Err(err) = stdout().execute(EnterAlternateScreen) {
            if raw {
                let _ = terminal::disable_raw_mode();
            }
            return Err(err.into());
        }
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        *guard = Some(Session {
            id,
            raw,
            kitty: false,
            #[cfg(feature = "mouse")]
            mouse: false,
        });
        Ok(Self { terminal, id })
    }

    /// Restore the active terminal session; repeated calls are no-ops.
    pub fn restore() -> Result {
        Ok(Session::finish_if(|_| true, None)?)
    }

    /// Enable the Kitty keyboard protocol if supported; returns whether it is enabled.
    pub fn enable_kitty_protocol(&self) -> io::Result<bool> {
        {
            let guard = active();
            let Some(session) = guard.as_ref().filter(|s| s.id == self.id) else {
                return Err(io::Error::other("crossterm session is not active"));
            };
            if session.kitty {
                return Ok(true);
            }
        }
        // Probe without holding `ACTIVE` so a blocking terminal query cannot stall a panic hook.
        if !terminal::supports_keyboard_enhancement()? {
            return Ok(false);
        }
        let mut guard = active();
        let Some(session) = guard.as_mut().filter(|s| s.id == self.id) else {
            return Err(io::Error::other("crossterm session is not active"));
        };
        if !session.kitty {
            stdout().execute(PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::all()))?;
            session.kitty = true;
        }
        Ok(true)
    }

    /// Whether this session has the Kitty keyboard protocol enabled.
    pub fn kitty_enabled(&self) -> bool {
        active()
            .as_ref()
            .is_some_and(|s| s.id == self.id && s.kitty)
    }

    /// Enable mouse capture for this session.
    #[cfg(feature = "mouse")]
    pub fn enable_mouse_capture(&self) -> io::Result<()> {
        let mut guard = active();
        let Some(session) = guard.as_mut().filter(|s| s.id == self.id) else {
            return Err(io::Error::other("crossterm session is not active"));
        };
        if !session.mouse {
            stdout().execute(EnableMouseCapture)?;
            session.mouse = true;
        }
        Ok(())
    }

    /// Whether this session captures mouse input.
    #[cfg(feature = "mouse")]
    pub fn mouse_enabled(&self) -> bool {
        active()
            .as_ref()
            .is_some_and(|s| s.id == self.id && s.mouse)
    }
}

impl Drop for CrosstermContext {
    fn drop(&mut self) {
        if let Err(err) = Session::finish_if(|s| s.id == self.id, Some(&mut self.terminal)) {
            let _ = writeln!(stderr(), "Failed to restore terminal: {err}");
        }
    }
}

fn install_hook() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let _ = Session::finish_if(|_| true, None);
            previous(info);
        }));
    });
}
