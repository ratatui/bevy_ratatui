
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

## drawing

Call `context.draw` once per frame, in `Update` (or, with `windowed`, in
`PostUpdate` before `TerminalSystems::Sync`). Ratatui is double buffered: each
`draw` presents a whole frame and then clears the buffer the next one renders
into, so a second `draw` in the same frame replaces the first. Split the frame
with layouts inside one draw system and let other systems supply its data, as
the [layout example](examples/layout.rs) does.

When parts of the screen belong to separate systems or plugins, they can each
render into `context.current_buffer_mut()` (with `Widget::render` or
`StatefulWidget::render`), and one system ordered after them all presents the
frame with `context.apply_buffer_with_cursor(..)` (Ratatui 0.30.1 or later).
The [multi_draw example](examples/multi_draw.rs) shows the whole pattern, with
a small `FramePass` resource the systems share. It keeps four rules:

1. The first system to draw in a frame calls `context.autoresize()`, once,
   before drawing; frames nothing draws do not resize, so the windowed backend
   keeps showing the last frame while its window is resized.
2. If no system drew, the presenting system does nothing: presenting an
   untouched buffer would blank the screen.
3. Every system draws its part on every frame that is presented; a region
   nobody drew is presented as blank.
4. A system that wants the cursor stores its position in the resource, and the
   presenting system passes `pass.cursor.take()` to `apply_buffer_with_cursor`,
   so the cursor hides again on frames that set none.
   `get_frame().set_cursor_position(..)` does nothing outside `draw`.

## demo

![Made with VHS](https://vhs.charm.sh/vhs-2g0S6RgGGQHseTCNItEQhg.gif)

See the [demo example](examples/demo.rs) for the code and more information.
For splitting the terminal into regions with Ratatui layouts, see the
[layout example](examples/layout.rs), and the
[layout stress test](examples/layout_test.rs) for nested layouts, every
constraint and flex mode, and every widget. For a larger multi-file game
example, see the [snake example](examples/snake/README.md). For several
systems drawing one frame, see the [multi_draw example](examples/multi_draw.rs).

## features

- `windowed`: Render your ratatui application in a window instead of the
  terminal buffer. Reference the `demo` example for how to set up a Bevy
  project to handle either mode, [windowed_fonts](examples/windowed_fonts.rs)
  for custom font assets and sizing on `TerminalRenderConfig`,
  [windowed_mouse](examples/windowed_mouse.rs) for the cell under the mouse
  (`WindowedCursor`), or
  [windowed_system_fonts](examples/windowed_system_fonts.rs) with
  `bevy/system_font_discovery` for OS font discovery. Rendered on the GPU via
  [bevy_terminal](https://github.com/gold-silver-copper/bevy_terminal); enables
  the Bevy features the windowed backend needs (windowing, rendering, text with
  `default_font`, and `bevy_ui`) and a set of platform defaults (X11 and
  Wayland, WebGL2, multithreading). The default font has a single face, so bold
  and italic text render as regular until you set bold and italic faces on
  `TerminalRenderConfig`.

  The context is a `Terminal<RatatuiBackend>` (it was soft_ratatui's
  `SoftBackend`, whose pixmap methods are gone), which is a breaking change for
  code naming the backend. The grid starts at 80×24 and follows the primary
  window. Configure the terminal through the `TerminalRenderConfig` component
  on the context, in `Startup` or later; see `bevy_ratatui::windowed`. Draw in
  `Update` (or in `PostUpdate` before `TerminalSystems::Sync`, when
  `bevy_terminal` reads the grid); drawing in `Last` shows one frame late.

  Because Cargo features are additive and unified across the dependency graph,
  enabling `bevy_ratatui/windowed` anywhere in a build switches
  `RatatuiContext` and `RatatuiPlugins` to the windowed backend for the entire
  build, and `RatatuiPlugins` then ignores its three crossterm flags
  (`enable_kitty_protocol`, `enable_mouse_capture`, `enable_input_forwarding`).
  Libraries should leave enabling `windowed` to the application.
- `scrolling-regions`: Enable Ratatui's scrolling regions for every backend.
  Use this rather than `ratatui/scrolling-regions`, which leaves the windowed
  backend without the methods Ratatui then requires.

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

| bevy | bevy_ratatui             |
|------|--------------------------|
| 0.20 | 0.13.0–0.14.0 (upcoming) |
| 0.19 | 0.11.2–0.12.0            |
| 0.18 | 0.11.0–0.11.1            |
| 0.17 | 0.10                     |
| 0.16 | 0.9                      |
| 0.15 | 0.7                      |
| 0.14 | 0.6                      |
| 0.13 | 0.5                      |

## license

Copyright (c) Josh McKinney
Copyright (c) Cooper Jax Reiff

This project is licensed under either of

- Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

The fonts under [assets/fonts](assets/fonts) are used by the examples only, are
not part of the published crate, and keep their own licenses, which sit beside
them.

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
