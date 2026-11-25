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
                .env(
                    "PTY_KEYBOARD",
                    if cfg!(feature = "keyboard") { "1" } else { "" },
                )
                .env("PTY_MOUSE", if cfg!(feature = "mouse") { "1" } else { "" })
                .status();
            assert!(status.expect("python3").success(), "pty scenarios failed");
        }
    }
}

#[cfg(all(unix, feature = "crossterm", not(feature = "windowed")))]
mod scenario {
    use std::{
        io::{Write, stdin},
        panic, process,
        sync::Barrier,
        thread,
    };

    use bevy::prelude::*;
    use bevy_ratatui::{RatatuiContext, RatatuiPlugins};

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
                let _ = thread::spawn(|| panic!("worker thread panic")).join();
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
            #[cfg(feature = "keyboard")]
            "forwarding" | "forwarding_panic" => forwarding(scenario == "forwarding_panic"),
            #[cfg(feature = "keyboard")]
            "focus_unavailable" => {
                let context = RatatuiContext::init().expect("a context");
                RatatuiContext::restore().expect("restored");
                assert!(context.enable_focus_reporting().is_err());
                App::new()
                    .insert_resource(context)
                    .add_plugins(bevy_ratatui::translation::TranslationPlugin)
                    .update();
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
                #[cfg(feature = "mouse")]
                assert!(!old.mouse_enabled());
                let new = RatatuiContext::init().expect("resumed context");
                assert!(new.enable_kitty_protocol().expect("kitty resumed"));
                #[cfg(feature = "mouse")]
                new.enable_mouse_capture().expect("mouse resumed");
                drop(old);
                eprint!("<STALE-DROPPED>");
                drop(new);
            }
            "rollback" => {
                // With the PTY as controlling terminal and stdout a closed pipe,
                // both attempts must get past sizing and raw mode and fail writing
                // `EnterAlternateScreen` with `BrokenPipe`. The second attempt also
                // proves `ACTIVE` did not latch.
                for _ in 0..2 {
                    let err = RatatuiContext::init().expect_err("broken stdout must fail");
                    let kind = err
                        .downcast_ref::<std::io::Error>()
                        .map(std::io::Error::kind);
                    assert_eq!(kind, Some(std::io::ErrorKind::BrokenPipe), "{err}");
                    assert!(
                        !bevy_ratatui::crossterm::terminal::is_raw_mode_enabled()
                            .expect("raw mode query"),
                        "raw mode leaked after failed init"
                    );
                    snapshot();
                }
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

    #[cfg(feature = "keyboard")]
    fn forwarding(panic_after_input: bool) {
        use bevy::input::keyboard::Key;
        use bevy_ratatui::{
            crossterm::event,
            translation::{Capability, Detected},
        };
        use std::time::{Duration, Instant};

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            RatatuiPlugins {
                enable_input_forwarding: true,
                ..plugins(true, false)
            },
        ));
        app.update();
        assert_eq!(
            app.world().resource::<Detected>().0,
            Capability::KEY_RELEASE
        );
        let context = app.world().resource::<RatatuiContext>();
        assert!(context.focus_reporting_enabled());
        context.enable_focus_reporting().expect("already enabled");

        // The driver writes a, focus lost, b, focus gained, c, which may arrive in
        // several reads. Each focus change ends a poll batch, so every phase below
        // is visible after exactly one update.
        eprint!("<SEND-KEYS>");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut phase = |what: &str, done: fn(&ButtonInput<KeyCode>, &ButtonInput<Key>) -> bool| {
            while !done(app.world().resource(), app.world().resource()) {
                assert!(Instant::now() < deadline, "timed out waiting for {what}");
                event::poll(Duration::from_millis(10)).expect("driver input");
                app.update();
            }
        };
        phase("focus loss to clear a", |keys, logical| {
            keys.just_released(KeyCode::KeyA)
                && keys.get_pressed().count() == 0
                && logical.get_pressed().count() == 0
        });
        phase("b without c", |keys, _| {
            keys.pressed(KeyCode::KeyB) && !keys.pressed(KeyCode::KeyC)
        });
        phase("b and c", |keys, _| {
            keys.all_pressed([KeyCode::KeyB, KeyCode::KeyC])
        });
        if panic_after_input {
            panic!("panic with focus reporting enabled");
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
