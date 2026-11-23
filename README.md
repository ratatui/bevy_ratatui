
# bevy_ratatui

[![Crate Badge]][Crate]
[![Docs Badge]][Docs]
[![Downloads Badge]][Downloads]
[![License Badge]][License]

Set up a Ratatui application, using bevy to manage the update loop, handle
input events, draw to the buffer, etcetera.

## getting started

`cargo add bevy_ratatui ratatui crossterm`

```rust
use bevy::prelude::*;
use bevy::app::ScheduleRunnerPlugin;
use bevy_ratatui::{RatatuiContext, RatatuiPlugins};

fn main() {
    let frame_time = std::time::Duration::from_secs_f32(1. / 60.);

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(frame_time)),
            RatatuiPlugins::default(),
        ))
        .add_systems(Update, draw_system)
        .run();
}

fn draw_system(mut context: ResMut<RatatuiContext>) -> Result {
    context.draw(|frame| {
        let text = ratatui::text::Text::raw("hello world");
        frame.render_widget(text, frame.area());
    })?;

    Ok(())
}
```

To read user input, you can listen for the crossterm input messages forwarded by
this crate:

```rust
use bevy::app::AppExit;
use bevy_ratatui::event::KeyMessage;
use crossterm::event::KeyCode;

fn input_system(mut messages: MessageReader<KeyMessage>, mut exit: MessageWriter<AppExit>) {
    for message in messages.read() {
        if let KeyCode::Char('q') = message.code {
            exit.write_default();
        }
    }
}
```

...or use the `enable_input_forwarding` option in `RatatuiPlugins` which will
map crossterm input events to normal bevy input messages.

## demo

![Made with VHS](https://vhs.charm.sh/vhs-2g0S6RgGGQHseTCNItEQhg.gif)

See the [demo example](examples/demo.rs) for the code and more information.
For splitting the terminal into regions with Ratatui layouts, see the
[layout example](examples/layout.rs), and the
[layout stress test](examples/layout_test.rs) for nested layouts, every
constraint and flex mode, and every widget. For a larger multi-file game
example, see the [snake example](examples/snake/README.md).

## features

- `windowed`: Render your ratatui application in a window instead of the
  terminal buffer. Reference the `demo` example for how to set up a Bevy
  project to handle either mode. The buffer is rendered on the GPU by
  [bevy_terminal](https://github.com/gold-silver-copper/bevy_terminal) using
  Bevy's built-in default monospace font (ASCII coverage, paired with
  `bevy_terminal`'s procedural box-drawing, block, shade, and braille sprites,
  with no dependency on system font discovery); the grid fills the window and
  follows its scale factor. For non-ASCII Unicode glyphs (such as scrollbar
  arrows, accented Latin, or Nerd Fonts) or dedicated bold/italic faces, supply
  your own font asset via `FontFaces`. This enables only the Bevy features the
  windowed backend itself needs — windowing, rendering, text (with
  `default_font`), and `bevy_ui`. If your app also wants audio, 3D, scenes,
  gamepads or image codecs, enable those on your own `bevy` dependency; Cargo
  features are additive.

  The terminal's appearance is a `TerminalRenderConfig` component on the
  `RatatuiContext` resource's entity. Configure it at startup in `PostStartup`
  (after `RatatuiContext` is inserted in `Startup`), or swap fonts and sizing
  live in any `Update` system while the terminal is running:

  ```rust,ignore
  use bevy::prelude::*;
  use bevy_ratatui::{
      RatatuiContext,
      windowed::{FontFaces, TerminalRenderConfig, TerminalSizing},
  };

  // Register with `.add_systems(PostStartup, configure_terminal)` (or run in `Update` to swap live):
  fn configure_terminal(
      asset_server: Res<AssetServer>,
      mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>,
  ) {
      config.sizing = TerminalSizing::font(24.0);
      // Dynamically loaded from assets/:
      config.font = FontFaces::regular(asset_server.load("fonts/MyFont.ttf"));
      // Or embedded into the binary with `fonts.add(Font::from_bytes(include_bytes!(...).to_vec()))`.
  }
  ```

  For a full runnable example loading all four JetBrains Mono faces (regular,
  bold, italic, and bold-italic) both dynamically via `AssetServer` and from
  embedded bytes, with live font/size/cursor switching, see
  [windowed_fonts](examples/windowed_fonts.rs)
  (`cargo run --example windowed_fonts --features windowed`; JetBrains Mono is
  licensed under SIL OFL 1.1, see `assets/fonts/jetbrains-mono/OFL.txt`).

There are also a handful of features relating to running Bevy in `no_std` mode.

## see also

### integrates with

- [bevy](https://github.com/bevyengine/bevy): A refreshingly simple data-driven
  game engine built in Rust.
- [ratatui](https://github.com/ratatui/ratatui): A Rust crate for cooking up
  terminal user interfaces (TUIs).

### more tools

- [egui_ratatui](https://github.com/gold-silver-copper/egui_ratatui): A ratatui
  backend that is also an egui widget. Deploy on web with WASM or ship natively
  with bevy, macroquad, or eframe. Demo at
  <https://gold-silver-copper.github.io/>.
- [bevy_ratatui_camera](https://github.com/cxreiff/bevy_ratatui_camera): Print
  a bevy scene to the terminal. Provides a ratatui widget that converts a bevy
  camera's rendered image to text and draws it to the terminal with ratatui.

### alternatives

- [widgetui](https://github.com/TheEmeraldBee/widgetui): A wrapper for ratatui
  that reduces boilerplate and handles the update loop. Uses an approach
  similar to bevy systems.
- [bevyterm](https://github.com/Mimea005/bevyterm): A bevy crossterm
  integration that uses bevy systems to set up a terminal application.

## compatibility

| bevy | bevy_ratatui      |
|------|-------------------|
| 0.20 | 0.13.0            |
| 0.19 | 0.11.2–0.12.0     |
| 0.18 | 0.11.0–0.11.1     |
| 0.17 | 0.10              |
| 0.16 | 0.9               |
| 0.15 | 0.7               |
| 0.14 | 0.6               |
| 0.13 | 0.5               |

## license

Copyright (c) Josh McKinney
Copyright (c) Cooper Jax Reiff

This project is licensed under either of

- Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[Crate]: https://crates.io/crates/bevy_ratatui
[Crate Badge]: https://img.shields.io/crates/v/bevy_ratatui
[Docs]: https://docs.rs/bevy_ratatui
[Docs Badge]: https://img.shields.io/badge/docs-bevy_ratatui-886666
[Downloads]: https://crates.io/crates/bevy_ratatui
[Downloads Badge]: https://img.shields.io/crates/d/bevy_ratatui.svg
[License]: ./LICENSE-MIT
[License Badge]: https://img.shields.io/crates/l/bevy_ratatui
