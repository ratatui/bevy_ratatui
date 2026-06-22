# Snake example

This example shows a complete terminal game built with `bevy_ratatui`. It is intentionally larger
than the small examples in this repository so the full `bevy_ratatui` application flow remains
visible: terminal events enter as Bevy messages, systems update resources in an explicit order, and
a final system projects those resources into a Ratatui frame.

Run it with:

```sh
cargo run --example snake
```

## `bevy_ratatui` application pattern

The reusable part of this example is the path through the Bevy schedules:

1. `RatatuiPlugins` initializes the terminal context and emits terminal events such as `KeyMessage`.
2. A `PreUpdate` system translates those terminal-specific messages into the application's own
   `GameCommand` messages.
3. Chained `Update` systems synchronize terminal-dependent resources, read commands, and advance
   application state in a deliberate order.
4. A `PostUpdate` system borrows `RatatuiContext` and calls `draw`, so rendering sees the state
   produced by the current update without owning or mutating the game rules.

This input-resource-render flow applies to dashboards, editors, and other terminal applications as
well as games. The turn queue, crash grace, and partial-block animation are Snake policies rather
than requirements imposed by `bevy_ratatui`.

## Module map

The first reading path follows the reusable `bevy_ratatui` integration:

- [`main.rs`](main.rs) wires the Bevy app and acts as the Rustdoc table of contents.
- [`controls.rs`](controls.rs) translates `KeyMessage` values into `GameCommand` messages. This
  keeps terminal key codes out of the game rules.
- [`game_loop.rs`](game_loop.rs) owns the Bevy runtime glue and `GameTiming`: applying commands,
  advancing logical movement, and managing the wall-clock collision grace period.
- [`terminal.rs`](terminal.rs) lays out and renders the current game state with Ratatui.

The game-rule modules can be read without following terminal rendering details:

- [`game.rs`](game.rs) owns state transitions for movement, scoring, collision, restart, and resize.
- [`snake.rs`](snake.rs) owns logical segments, the latest movement snapshot, direction, and queued
  turns.
- [`geometry.rs`](geometry.rs) owns board coordinates and movement directions shared by the rules,
  controls, and renderer.

The remaining modules support terminal sizing and the optional smooth-rendering details:

- [`playfield.rs`](playfield.rs) derives the logical board and availability state from the terminal
  size so update and rendering follow the same environmental policy.
- [`hud.rs`](hud.rs) renders score, controls, overlays, and the small-terminal fallback.
- [`terminal_cells.rs`](terminal_cells.rs) owns the wide-cell projection and the foreground,
  background, and Unicode glyph rules for individual terminal cells.
- [`snake_rendering.rs`](snake_rendering.rs) renders stable body cells and smoothly interpolated
  head and tail cells, including fractional coverage calculations.

The later modules are not prerequisites for understanding how `RatatuiPlugins`, messages,
resources, schedules, and `RatatuiContext::draw` fit together.

## Design notes

The logical board fills the terminal area left after the HUD and controls. A terminal cell is
usually taller than it is wide, so the renderer draws each logical board cell as two side-by-side
terminal cells. This makes vertical and horizontal movement feel closer in physical distance
without making the arena so dense that play becomes empty and hard to read.

The rules still move on whole board cells, but rendering draws the head and tail between the
previous and current cell positions using Unicode eighth-block glyphs. The middle body stays on the
stable logical path. That split keeps motion responsive without making turns flicker as neighboring
body segments round through a corner at different times. The moving head leaves body-colored
underlay in the old head cell. The moving tail reveals board background in the old tail cell while
leaving body-colored underlay in the current tail cell so it stays connected to the rest of the
snake. Collision, food placement, and scoring never depend on fractional coordinates. The fruit
uses a double-width emoji in the same two-column cell shape as the snake.

The example restarts the current round after a terminal resize changes the playable board. Snake
segments and food are stored as board coordinates, so resizing in place would need a policy for
scaling or clipping an active snake. Restarting keeps the example predictable and preserves the
high score for the session. If the terminal becomes too small to fit the minimum board, the game
loop suspends without changing the player's pause state. Returning to the same valid dimensions
continues the current round instead of allowing the snake to move invisibly behind the size warning.

The controls use a small turn queue instead of a single pending direction. If the snake is moving
right and the player presses `Up, Left` before the next movement tick, the game applies `Up` on the
next tick and `Left` on the tick after that. Reversal checks compare a new input to the last queued
direction, so `Right, Up, Left` is valid but `Right, Up, Down` is not.
Direction keys accept terminal key-repeat events, but pause and restart only accept the initial
press. Repeating a movement direction is harmless and responsive; repeating a toggle or reset would
make one physical key hold produce several state changes.

The game also gives the player a short crash-grace window. Taneli Armanto
[described the original idea][history-of-snake] as a tiny delay that made fast edge turns less
frustrating. In [another interview][oral-history], he described that delay as a few extra
milliseconds right before a crash where the player could still turn and continue. That same
interview describes Snake speed as the delay between position steps, not as a fixed 60 FPS render
loop. This example models the crash delay as a short wall-clock timer in `game_loop.rs`, separate
from both the render frame and the normal movement tick. A late turn must actually move to safety;
turning into another collision ends the round instead of granting another grace window.

The repeating movement timer reports how many logical intervals elapsed during each render frame.
The loop processes up to four of those steps, preserving normal game speed through short scheduler
stalls while preventing a debugger pause or suspended terminal from producing an unbounded burst of
moves. Pause and terminal-size suspension do not reset the timer, so the partial head and tail stay
at their current visual positions until play resumes. Collision completes the previous interpolation
before showing crash grace, avoiding a one-cell visual rewind when a failed step does not update the
snake path.

`game_loop.rs` contains Bevy systems, but only the systems that drive the game loop; input
translation and rendering stay near their own concepts. Module docs explain ownership and the
overall model. Item docs keep policy, invariants, and edge-case rationale close to the constants,
resources, fields, and helpers that implement those decisions so readers can distinguish reusable
Bevy patterns from choices made specifically for Snake.

## Validation

The example has unit tests for the framework-independent rules, key-repeat policy, timing
boundaries, terminal-size suspension, interpolation underlays, and terminal-size policy:

```sh
cargo test --example snake
```

The example also participates in workspace target checks:

```sh
cargo check --workspace --all-targets
```

[history-of-snake]: https://www.itsnicethat.com/features/taneli-armanto-the-history-of-snake-design-legacies-230221
[oral-history]: https://melmagazine.com/en-us/story/snake-nokia-6110-oral-history-taneli-armanto
