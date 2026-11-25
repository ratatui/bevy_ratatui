//! Real pseudo-terminal lifecycle tests: `tests/pty.py` drives this binary once per scenario
//! under a PTY, answers terminal queries on the wire, and asserts exact escape bytes and termios.

fn main() {
    #[cfg(all(unix, feature = "crossterm", not(feature = "windowed")))]
    match std::env::var("PTY_SCENARIO") {
        Ok(scenario) => scenario::run(&scenario),
        Err(_) => {
            let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pty.py");
            let exe = std::env::current_exe().expect("the test binary");
            let status = std::process::Command::new("python3")
                .arg(script)
                .arg(exe)
                .env("PTY_MOUSE", if cfg!(feature = "mouse") { "1" } else { "" })
                .status();
            assert!(status.expect("python3").success(), "pty scenarios failed");
        }
    }
}

#[cfg(all(unix, feature = "crossterm", not(feature = "windowed")))]
mod scenario {
    use std::{
        io::{Write, stdin, stdout},
        panic, process,
        sync::Barrier,
        thread,
    };

    use bevy::prelude::*;
    use bevy_ratatui::{RatatuiContext, RatatuiPlugins};

    unsafe extern "C" {
        fn dup2(old: i32, new: i32) -> i32;
    }

    pub fn run(scenario: &str) {
        match scenario {
            "exit" => {
                app(true, true, |mut exit: MessageWriter<AppExit>| {
                    exit.write_default();
                });
                eprint!("<RUN-RETURNED>");
                panic!("unrelated panic after the app");
            }
            "retained_exit" => {
                let mut app = App::new();
                app.add_plugins((MinimalPlugins, plugins(true, true)))
                    .add_systems(
                        Update,
                        (draw, |mut exit: MessageWriter<AppExit>| {
                            exit.write_default();
                        })
                            .chain(),
                    );
                app.update();
                eprint!("<APP-ALIVE>");
                snapshot();
                drop(app);
            }
            "panic" => app(true, true, || panic!("main thread panic")),
            "worker_panic" => app(true, true, |mut exit: MessageWriter<AppExit>| {
                let _stdout_lock = stdout().lock();
                let _ = thread::spawn(|| panic!("worker thread panic")).join();
                drop(_stdout_lock);
                exit.write_default();
            }),
            "abort" => {
                let previous = panic::take_hook();
                panic::set_hook(Box::new(move |info| {
                    eprint!("<USER-HOOK>");
                    previous(info);
                    process::abort();
                }));
                app(true, true, || panic!("panic before abort"));
            }
            "hook_after" => {
                App::new()
                    .add_plugins((MinimalPlugins, plugins(false, false)))
                    .add_systems(PostStartup, || {
                        let previous = panic::take_hook();
                        panic::set_hook(Box::new(move |info| {
                            eprint!("<LATE-HOOK>");
                            previous(info);
                        }));
                    })
                    .add_systems(Update, (draw, || panic!("panic after a late hook")).chain())
                    .run();
            }
            "direct" => {
                let mut context = RatatuiContext::init().expect("a context");
                if RatatuiContext::init().is_err() {
                    eprint!("<REFUSED>");
                }
                context
                    .draw(|frame| frame.render_widget("direct", frame.area()))
                    .expect("drawn");
                drop(context);
                RatatuiContext::restore().expect("nothing left to restore");
            }
            "resume" => {
                let mut old = RatatuiContext::init().expect("first context");
                assert!(old.enable_kitty_protocol().expect("kitty"));
                assert!(old.kitty_enabled());
                #[cfg(feature = "mouse")]
                {
                    old.enable_mouse_capture().expect("mouse");
                    assert!(old.mouse_enabled());
                }
                old.draw(|frame| frame.render_widget("first", frame.area()))
                    .expect("drawn");
                RatatuiContext::restore().expect("restored");
                RatatuiContext::restore().expect("second restore is no-op");
                assert!(!old.kitty_enabled());
                let new = RatatuiContext::init().expect("resumed context");
                assert!(new.enable_kitty_protocol().expect("kitty resumed"));
                #[cfg(feature = "mouse")]
                new.enable_mouse_capture().expect("mouse resumed");
                // Dropping the retired drawn context must emit neither Leave nor Show.
                drop(old);
                eprint!("<STALE-DROPPED>");
                drop(new);
            }
            "rollback_retry" => {
                assert!(RatatuiContext::init().is_err());
                snapshot();
                assert_eq!(unsafe { dup2(0, 1) }, 1);
                let mut ctx = RatatuiContext::init().expect("retry succeeds");
                ctx.draw(|frame| frame.render_widget("retried", frame.area()))
                    .expect("drawn");
                drop(ctx);
            }
            "race" => {
                for _ in 0..20 {
                    let context = RatatuiContext::init().expect("a context");
                    let barrier = Barrier::new(2);
                    thread::scope(|scope| {
                        let worker = scope.spawn(|| {
                            barrier.wait();
                            panic!("racing panic");
                        });
                        barrier.wait();
                        drop(context);
                        let _ = worker.join();
                    });
                }
            }
            _ => panic!("unknown scenario {scenario}"),
        }
    }

    fn plugins(kitty: bool, mouse: bool) -> RatatuiPlugins {
        RatatuiPlugins {
            enable_kitty_protocol: kitty,
            enable_mouse_capture: mouse,
            ..default()
        }
    }

    fn app<M>(kitty: bool, mouse: bool, frame: impl IntoSystem<(), (), M>) {
        App::new()
            .add_plugins((MinimalPlugins, plugins(kitty, mouse)))
            .add_systems(Update, (draw, frame).chain())
            .run();
    }

    fn draw(mut context: ResMut<RatatuiContext>) -> Result {
        context.draw(|frame| frame.render_widget("hello", frame.area()))?;
        Ok(())
    }

    fn snapshot() {
        eprint!("<SNAP>");
        let _ = std::io::stderr().flush();
        stdin().read_line(&mut String::new()).expect("driver reply");
    }
}
