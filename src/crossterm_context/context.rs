use std::{
    io::{self, BufWriter, Write, stderr, stdout},
    panic,
    sync::{Arc, Mutex, MutexGuard, Weak},
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

const RAW: u8 = 1 << 0;
const ALT: u8 = 1 << 1;
const CURSOR: u8 = 1 << 2;
const KITTY: u8 = 1 << 3;
#[cfg(feature = "mouse")]
const MOUSE: u8 = 1 << 4;

#[cfg(any(unix, windows))]
type TtyOut = std::fs::File;
#[cfg(not(any(unix, windows)))]
type TtyOut = io::Stdout;

static ACTIVE: Mutex<Option<Session>> = Mutex::new(None);

fn active() -> MutexGuard<'static, Option<Session>> {
    ACTIVE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Clone the OS stdout handle so teardown and draws never contend on Rust's `Stdout` lock.
fn clone_stdout() -> io::Result<TtyOut> {
    #[cfg(unix)]
    {
        Ok(std::os::fd::AsFd::as_fd(&stdout())
            .try_clone_to_owned()?
            .into())
    }
    #[cfg(windows)]
    {
        Ok(std::os::windows::io::AsHandle::as_handle(&stdout())
            .try_clone_to_owned()?
            .into())
    }
    #[cfg(not(any(unix, windows)))]
    Ok(stdout())
}

fn step(modes: &mut u8, bit: u8, err: &mut Option<io::Error>, op: impl FnOnce() -> io::Result<()>) {
    if *modes & bit != 0 {
        match op() {
            Ok(()) => *modes &= !bit,
            Err(e) => _ = err.get_or_insert(e),
        }
    }
}

struct Session {
    owner: Arc<()>,
    output: TtyOut,
    retired: bool,
    modes: u8,
}

impl Session {
    fn owns(&self, owner: &Arc<()>) -> bool {
        !self.retired && Arc::ptr_eq(&self.owner, owner)
    }

    /// Hold `ACTIVE` through the final teardown syscall so a competing panic hook,
    /// `Drop`, or `init` cannot overtake an in-progress restore.
    fn finish_if(owns: impl FnOnce(&Session) -> bool) -> io::Result<()> {
        let mut guard = active();
        let Some(session) = guard.as_mut().filter(|s| owns(s)) else {
            return Ok(());
        };
        let result = session.teardown();
        if session.modes == 0 {
            *guard = None;
        }
        result
    }

    fn teardown(&mut self) -> io::Result<()> {
        self.retired = true;
        let (m, out, mut err) = (&mut self.modes, &mut self.output, None);
        step(m, KITTY, &mut err, || {
            out.execute(PopKeyboardEnhancementFlags).map(drop)
        });
        #[cfg(feature = "mouse")]
        step(m, MOUSE, &mut err, || {
            out.execute(DisableMouseCapture).map(drop)
        });
        step(m, ALT, &mut err, || {
            out.execute(LeaveAlternateScreen).map(drop)
        });
        step(m, CURSOR, &mut err, || out.execute(cursor::Show).map(drop));
        step(m, RAW, &mut err, terminal::disable_raw_mode);
        err.map_or(Ok(()), Err)
    }
}

/// Output writer scoped to one session; retired contexts (including Ratatui's
/// cursor-showing `Terminal::drop`) cannot write into a restored or newer session.
#[derive(Debug)]
pub struct SessionWriter(Arc<()>);

impl Write for SessionWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match active().as_mut().filter(|s| s.owns(&self.0)) {
            Some(s) => s.output.write(bytes),
            None => Ok(bytes.len()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match active().as_mut().filter(|s| s.owns(&self.0)) {
            Some(s) => s.output.flush(),
            None => Ok(()),
        }
    }
}

/// Ratatui context that draws to the terminal using crossterm and restores it at most once.
#[derive(Resource, Deref, DerefMut, Debug)]
pub struct CrosstermContext {
    #[deref]
    terminal: Terminal<CrosstermBackend<BufWriter<SessionWriter>>>,
    owner: Arc<()>,
}

impl CrosstermContext {
    /// Start a terminal session, installing a process-wide panic hook before changing modes.
    pub fn init() -> Result<Self> {
        if std::thread::panicking() {
            return Err("cannot initialize a terminal while panicking".into());
        }
        install_hook();
        let owner = Arc::new(());
        let writer = BufWriter::new(SessionWriter(owner.clone()));
        let terminal = Terminal::new(CrosstermBackend::new(writer))?;
        let mut output = clone_stdout()?;
        let mut guard = active();
        if guard.is_some() {
            return Err("a crossterm session is already active".into());
        }
        let raw_owned = !terminal::is_raw_mode_enabled()?;
        if raw_owned {
            terminal::enable_raw_mode()?;
        }
        if let Err(err) = output.execute(EnterAlternateScreen) {
            if raw_owned {
                let _ = terminal::disable_raw_mode();
            }
            return Err(err.into());
        }
        let modes = ALT | CURSOR | if raw_owned { RAW } else { 0 };
        *guard = Some(Session {
            owner: owner.clone(),
            output,
            retired: false,
            modes,
        });
        Ok(Self { terminal, owner })
    }

    /// Restore the active terminal session; repeated calls are no-ops once clean.
    pub fn restore() -> Result {
        Ok(Session::finish_if(|_| true)?)
    }

    /// Enable the Kitty keyboard protocol if supported; returns whether it is enabled.
    pub fn enable_kitty_protocol(&self) -> io::Result<bool> {
        let mut guard = active();
        let Some(s) = guard.as_mut().filter(|s| s.owns(&self.owner)) else {
            return Err(io::Error::other("crossterm session is not active"));
        };
        if s.modes & KITTY != 0 {
            return Ok(true);
        }
        if !terminal::supports_keyboard_enhancement()? {
            return Ok(false);
        }
        s.output
            .execute(PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::all()))?;
        s.modes |= KITTY;
        Ok(true)
    }

    /// Whether this session has the Kitty keyboard protocol enabled.
    pub fn kitty_enabled(&self) -> bool {
        active()
            .as_ref()
            .is_some_and(|s| s.owns(&self.owner) && s.modes & KITTY != 0)
    }

    /// Enable mouse capture for this session.
    #[cfg(feature = "mouse")]
    pub fn enable_mouse_capture(&self) -> io::Result<()> {
        let mut guard = active();
        let Some(s) = guard.as_mut().filter(|s| s.owns(&self.owner)) else {
            return Err(io::Error::other("crossterm session is not active"));
        };
        if s.modes & MOUSE == 0 {
            s.output.execute(EnableMouseCapture)?;
            s.modes |= MOUSE;
        }
        Ok(())
    }

    /// Whether this session captures mouse input.
    #[cfg(feature = "mouse")]
    pub fn mouse_enabled(&self) -> bool {
        active()
            .as_ref()
            .is_some_and(|s| s.owns(&self.owner) && s.modes & MOUSE != 0)
    }
}

impl Drop for CrosstermContext {
    fn drop(&mut self) {
        if let Err(err) = Session::finish_if(|s| Arc::ptr_eq(&s.owner, &self.owner)) {
            let _ = writeln!(stderr(), "Failed to restore terminal: {err}");
        }
    }
}

fn install_hook() {
    type Hook = Box<dyn Fn(&panic::PanicHookInfo<'_>) + Send + Sync>;
    static INSTALLED: Mutex<Weak<Hook>> = Mutex::new(Weak::new());
    let mut installed = INSTALLED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if installed.strong_count() == 0 {
        let previous = Arc::new(panic::take_hook());
        *installed = Arc::downgrade(&previous);
        panic::set_hook(Box::new(move |info| {
            let _ = Session::finish_if(|_| true);
            previous(info);
        }));
    }
}
