//! Bevy systems that drive the snake game loop.
//!
//! The domain rules live in [`crate::game`]. These ordinary Bevy systems show how a
//! `bevy_ratatui` application can connect terminal conditions to domain resources without putting
//! behavior in the draw closure. [`apply_game_commands`] consumes translated input,
//! [`sync_playfield`] derives state from [`bevy_ratatui::RatatuiContext`], and
//! [`advance_game`] updates the model from Bevy time.
//!
//! The chained order in `main.rs` is meaningful: resize policy runs before commands, commands run
//! before movement, and drawing happens later in [`PostUpdate`]. The movement timer itself is a
//! Snake policy rather than a `bevy_ratatui` requirement. It preserves partial interpolation while
//! paused, catches up a bounded number of logical steps after ordinary stalls, and stops while the
//! terminal cannot display a valid board.
//!
//! Input forgiveness is intentional. Taneli Armanto, who created Nokia's 1997 Snake, described
//! adding a tiny crash delay after testing showed that edge turns were frustratingly hard at speed.
//! This example applies that idea when [`StepOutcome::Collision`] starts a short correction window.
//! [`crate::snake::Snake`] separately buffers quick chained turns such as `Up, Left` so they are
//! applied across consecutive movement steps instead of being consumed by one rendered frame.
//!
//! [Nokia's 1997 Snake]: https://www.itsnicethat.com/features/taneli-armanto-the-history-of-snake-design-legacies-230221

use std::time::Duration;

use bevy::{app::AppExit, prelude::*};

use crate::{
    controls::GameCommand,
    game::{BASE_STEP_DURATION, Game, StepOutcome},
    playfield::Playfield,
};

/// Input-forgiveness window after a movement step discovers a collision.
///
/// This duration is wall-clock time rather than a fraction of a movement step. Keeping it separate
/// lets faster levels retain the same small opportunity to correct a near-edge turn. It is a game
/// design choice inspired by [Nokia's 1997 Snake][snake-history], not timing required by Bevy or
/// `bevy_ratatui`.
///
/// [snake-history]: https://www.itsnicethat.com/features/taneli-armanto-the-history-of-snake-design-legacies-230221
const CRASH_GRACE_DURATION: Duration = Duration::from_millis(80);
/// Maximum logical steps processed after one delayed render frame.
///
/// Repeating timers report every interval crossed by a frame. Processing a few of those intervals
/// keeps movement speed stable through ordinary scheduling stalls, while the cap prevents a long
/// terminal suspension or debugger stop from causing an unbounded burst of invisible moves.
const MAX_CATCH_UP_STEPS: u32 = 4;

/// Runtime timing for movement, interpolation, and collision forgiveness.
///
/// Rendering still happens every frame. The repeating movement timer gates whole-cell rule updates
/// and supplies interpolation progress between them. The optional one-shot timer is also the single
/// definition of whether collision grace is active; [`Game`] only reports the
/// collision that starts it.
#[derive(Debug, Resource)]
pub struct GameTiming {
    /// Repeating cadence for whole-cell movement.
    movement: Timer,

    /// Active wall-clock opportunity to correct the latest collision.
    crash_grace: Option<Timer>,
}

impl GameTiming {
    /// Returns the visible fraction of the current movement interval.
    pub fn movement_progress(&self) -> f32 {
        self.movement.fraction()
    }

    /// Returns whether a collision is waiting for correction or expiry.
    pub fn crash_grace_active(&self) -> bool {
        self.crash_grace.is_some()
    }

    /// Marks the latest logical movement as fully rendered.
    ///
    /// Collision does not update the snake's movement snapshot, so leaving a repeating timer at its
    /// wrapped fraction would visually replay the preceding move during crash grace. Setting elapsed
    /// time to the duration keeps the snake at the position where the collision was detected.
    fn finish_interpolation(&mut self) {
        let duration = self.movement.duration();
        self.movement.set_elapsed(duration);
    }

    /// Advances movement time and returns a bounded number of completed logical steps.
    fn tick_movement(&mut self, delta: Duration) -> u32 {
        self.movement.tick(delta);
        self.movement
            .times_finished_this_tick()
            .min(MAX_CATCH_UP_STEPS)
    }

    /// Starts the Nokia-inspired wall-clock opportunity to correct a collision.
    fn start_crash_grace(&mut self) {
        self.crash_grace = Some(Timer::new(CRASH_GRACE_DURATION, TimerMode::Once));
    }

    /// Advances active collision grace and consumes it when it expires this frame.
    fn tick_crash_grace(&mut self, delta: Duration) -> bool {
        let Some(timer) = self.crash_grace.as_mut() else {
            return false;
        };

        timer.tick(delta);
        let expired = timer.just_finished();
        if expired {
            self.finish_crash_grace();
        }
        expired
    }

    /// Ends the active collision-correction opportunity without changing game rules.
    fn finish_crash_grace(&mut self) {
        self.crash_grace = None;
    }

    /// Restarts all runtime timing at the cadence of a new or corrected round.
    fn reset(&mut self, movement_duration: Duration) {
        self.movement.set_duration(movement_duration);
        self.movement.reset();
        self.crash_grace = None;
    }

    /// Updates movement cadence after score progression changes the level.
    fn set_movement_duration(&mut self, duration: Duration) {
        self.movement.set_duration(duration);
    }
}

impl Default for GameTiming {
    fn default() -> Self {
        Self {
            movement: Timer::new(BASE_STEP_DURATION, TimerMode::Repeating),
            crash_grace: None,
        }
    }
}

/// Applies high-level commands produced by the controls module in message order.
///
/// Restart resets runtime timing because the new snake has no previous movement to interpolate.
/// Turns only update rule state; [`advance_game`] decides when queued turns become logical
/// movement.
pub fn apply_game_commands(
    mut game: ResMut<Game>,
    mut game_commands: MessageReader<GameCommand>,
    mut exit: MessageWriter<AppExit>,
    mut timing: ResMut<GameTiming>,
) {
    for command in game_commands.read() {
        match command {
            GameCommand::Turn(direction) => {
                game.queue_turn(*direction);
            }
            GameCommand::TogglePause => game.toggle_pause(),
            GameCommand::Restart => {
                game.restart();
                timing.reset(game.step_duration());
            }
            GameCommand::Quit => {
                exit.write_default();
            }
        }
    }
}

/// Keeps the logical board matched to the available terminal playfield.
///
/// Resizing restarts the current round because snake segments and food are board coordinates. The
/// high score remains on the [`Game`] resource so resizing does not erase the session record.
/// Invalid or temporarily unavailable terminal sizes are recorded as unavailable instead of
/// changing [`Game::status`](crate::game::Game::status), allowing an explicit player pause to
/// remain distinct from an environmental suspension.
///
/// `RatatuiContext::size` can fail while terminal access is unavailable, so the system converts
/// both an error and an undersized terminal into `None`. Keeping that case in resource state lets
/// later systems use a normal Bevy guard instead of attempting terminal I/O themselves.
pub fn sync_playfield(
    context: Res<bevy_ratatui::RatatuiContext>,
    mut playfield: ResMut<Playfield>,
    mut game: ResMut<Game>,
    mut timing: ResMut<GameTiming>,
) {
    let board = playfield.update_for_size(context.size().ok());

    if board.is_some_and(|board| game.resize_board(board)) {
        timing.reset(game.step_duration());
    }
}

/// Advances the game for movement intervals completed since the previous render frame.
///
/// Paused rounds and unavailable terminals leave runtime timing untouched, preserving visual
/// progress until play resumes. A delayed frame may advance several cells, bounded by the
/// `MAX_CATCH_UP_STEPS` policy. Collision instead finishes the preceding interpolation and starts
/// the separate crash-grace clock. A buffered turn during grace must produce a valid move; another
/// collision ends the round rather than repeatedly renewing grace.
pub fn advance_game(
    time: Res<Time>,
    playfield: Res<Playfield>,
    mut timing: ResMut<GameTiming>,
    mut game: ResMut<Game>,
) {
    if !playfield.is_available() || !game.is_playing() {
        return;
    }

    if timing.crash_grace_active() {
        if game.snake().has_queued_turn() {
            match game.step() {
                StepOutcome::Moved | StepOutcome::AteFood => {
                    timing.reset(game.step_duration());
                }
                StepOutcome::Collision => {
                    timing.finish_crash_grace();
                    game.end_after_collision();
                }
                StepOutcome::Noop => {}
            }
            return;
        }

        if timing.tick_crash_grace(time.delta()) {
            game.end_after_collision();
        }
        return;
    }

    let completed_steps = timing.tick_movement(time.delta());
    for _ in 0..completed_steps {
        match game.step() {
            StepOutcome::Moved | StepOutcome::AteFood => {}
            StepOutcome::Collision => {
                timing.finish_interpolation();
                timing.start_crash_grace();
                return;
            }
            StepOutcome::Noop => break,
        }

        if !game.is_playing() {
            timing.finish_interpolation();
            break;
        }
    }
    timing.set_movement_duration(game.step_duration());
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use bevy_ratatui::event::KeyMessage;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::{
        controls::translate_keyboard_input,
        game::GameStatus,
        geometry::{Direction, Point},
        playfield::Playfield,
    };

    /// Creates the available terminal condition needed by movement-system tests.
    fn available_playfield() -> Playfield {
        let mut playfield = Playfield::default();
        playfield.update_for_size(Some(ratatui::layout::Size::new(100, 40)));
        playfield
    }

    #[test]
    fn terminal_key_message_reaches_the_game_as_a_buffered_turn() {
        let mut app = App::new();
        app.add_message::<KeyMessage>()
            .add_message::<GameCommand>()
            .add_message::<AppExit>()
            .init_resource::<Game>()
            .init_resource::<GameTiming>()
            .add_systems(PreUpdate, translate_keyboard_input)
            .add_systems(Update, apply_game_commands);
        app.world_mut()
            .write_message(KeyMessage(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)));

        app.update();

        assert!(app.world().resource::<Game>().snake().has_queued_turn());
    }

    #[test]
    fn reset_restores_base_round_cadence_and_clears_grace() {
        let mut game = Game::new();

        let mut timing = GameTiming::default();
        timing.set_movement_duration(Duration::from_millis(95));
        timing.tick_movement(Duration::from_millis(95));
        timing.start_crash_grace();
        timing.tick_crash_grace(Duration::from_millis(40));

        game.restart();
        timing.reset(game.step_duration());

        assert_eq!(timing.movement.duration(), BASE_STEP_DURATION);
        assert_eq!(timing.movement.elapsed(), Duration::ZERO);
        assert!(!timing.crash_grace_active());
    }

    #[test]
    fn catch_up_steps_are_bounded_after_a_long_frame() {
        let mut timing = GameTiming::default();
        let completed_steps = timing.tick_movement(BASE_STEP_DURATION * 20);

        assert_eq!(timing.movement.times_finished_this_tick(), 20);
        assert_eq!(completed_steps, MAX_CATCH_UP_STEPS);
    }

    #[test]
    fn collision_finishes_interpolation_before_grace() {
        let mut game = Game::new();
        let snake = VecDeque::from([Point::new(game.board().width() - 1, 0)]);
        game.replace_snake_for_test(snake, Direction::Right);
        game.place_food_for_test(Point::new(0, 0));

        let mut timing = GameTiming::default();
        let completed_steps = timing.tick_movement(BASE_STEP_DURATION);

        for _ in 0..completed_steps {
            if game.step() == StepOutcome::Collision {
                timing.finish_interpolation();
                timing.start_crash_grace();
                break;
            }
        }

        assert!(timing.crash_grace_active());
        assert_eq!(timing.movement_progress(), 1.0);
    }

    #[test]
    fn late_turn_that_still_collides_ends_the_round() {
        let mut app = App::new();
        let mut game = Game::new();
        let corner = Point::new(game.board().width() - 1, 0);
        game.replace_snake_for_test(VecDeque::from([corner]), Direction::Right);
        game.place_food_for_test(Point::new(0, 1));
        assert_eq!(game.step(), StepOutcome::Collision);
        assert!(game.queue_turn(Direction::Up));

        let mut timing = GameTiming::default();
        timing.start_crash_grace();

        app.insert_resource(game)
            .insert_resource(timing)
            .insert_resource(available_playfield())
            .init_resource::<Time>()
            .add_systems(Update, advance_game);

        app.update();

        assert_eq!(
            app.world().resource::<Game>().status(),
            GameStatus::GameOver
        );
        assert!(!app.world().resource::<GameTiming>().crash_grace_active());
    }

    #[test]
    fn paused_game_preserves_partial_interpolation() {
        let mut app = App::new();
        let mut timing = GameTiming::default();
        timing.tick_movement(Duration::from_millis(70));
        let elapsed = timing.movement.elapsed();

        let mut game = Game::new();
        game.toggle_pause();
        app.insert_resource(game)
            .insert_resource(timing)
            .insert_resource(available_playfield())
            .init_resource::<Time>()
            .add_systems(Update, advance_game);

        app.update();

        assert_eq!(
            app.world().resource::<GameTiming>().movement.elapsed(),
            elapsed
        );
    }

    #[test]
    fn unavailable_terminal_suspends_movement() {
        let mut app = App::new();
        let game = Game::new();
        let head = game.snake().head();
        let mut time = Time::<()>::default();
        time.advance_by(BASE_STEP_DURATION);

        app.insert_resource(game)
            .init_resource::<GameTiming>()
            .init_resource::<Playfield>()
            .insert_resource(time)
            .add_systems(Update, advance_game);

        app.update();

        assert_eq!(app.world().resource::<Game>().snake().head(), head);
        assert_eq!(
            app.world().resource::<GameTiming>().movement.elapsed(),
            Duration::ZERO
        );
    }

    #[test]
    fn delayed_frame_advances_each_completed_interval() {
        let mut app = App::new();
        let mut game = Game::new();
        let head = game.snake().head();
        game.place_food_for_test(Point::new(0, 0));
        let mut time = Time::<()>::default();
        time.advance_by(BASE_STEP_DURATION * 3);

        app.insert_resource(game)
            .init_resource::<GameTiming>()
            .insert_resource(available_playfield())
            .insert_resource(time)
            .add_systems(Update, advance_game);

        app.update();

        assert_eq!(
            app.world().resource::<Game>().snake().head(),
            Point::new(head.x + 3, head.y)
        );
    }

    #[test]
    fn expired_crash_grace_ends_the_round() {
        let mut app = App::new();
        let mut timing = GameTiming::default();
        timing.start_crash_grace();
        let mut time = Time::<()>::default();
        time.advance_by(CRASH_GRACE_DURATION);

        app.init_resource::<Game>()
            .insert_resource(timing)
            .insert_resource(available_playfield())
            .insert_resource(time)
            .add_systems(Update, advance_game);

        app.update();

        assert_eq!(
            app.world().resource::<Game>().status(),
            GameStatus::GameOver
        );
        assert!(!app.world().resource::<GameTiming>().crash_grace_active());
    }
}
