//! Framework-independent snake rules and state.
//!
//! This module deliberately avoids terminal and Bevy system concerns. The rest of the example can
//! resize the board, send commands, and render frames, but this module owns the rules that decide
//! whether the snake moved, ate food, collided, paused, or restarted.
//!
//! A collision is reported to [`crate::game_loop`], which owns the real-time correction window.
//! Keeping that timer out of this module lets rule tests advance one logical cell at a time without
//! constructing a Bevy app or Ratatui buffer.

use std::time::Duration;

#[cfg(test)]
use std::collections::VecDeque;

use rand::Rng;

use bevy::prelude::*;

use crate::{
    geometry::{Board, Direction, Point},
    snake::Snake,
};

/// Fallback board width before the first terminal-size sync.
pub const DEFAULT_BOARD_WIDTH: i16 = 48;
/// Fallback board height before the first terminal-size sync.
pub const DEFAULT_BOARD_HEIGHT: i16 = 48;
/// Initial delay between logical movement steps.
pub const BASE_STEP_DURATION: Duration = Duration::from_millis(140);
/// Per-level reduction in delay between movement steps.
const LEVEL_STEP_REDUCTION: Duration = Duration::from_millis(15);
/// Fastest movement cadence, preventing high levels from becoming unrenderable.
const MIN_STEP_DURATION: Duration = Duration::from_millis(60);
/// Base score awarded for food before applying the current level multiplier.
const POINTS_PER_FOOD: u32 = 10;
/// Score interval at which movement advances to the next level.
const POINTS_PER_LEVEL: u32 = 50;

/// Current round state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameStatus {
    /// The movement timer can advance the snake.
    Playing,

    /// The round is suspended until the player resumes.
    Paused,

    /// The round ended through wall or self collision.
    GameOver,
}

/// Result of one whole-cell movement attempt.
///
/// The Bevy loop uses this result to apply real-time policy without exposing timers to the
/// framework-independent rule model. In particular, [`Self::Collision`] lets the runtime decide
/// whether to offer a short correction window before ending the round.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StepOutcome {
    /// The snake moved without eating.
    Moved,

    /// The snake ate food, grew, and scored.
    AteFood,

    /// The proposed head position intersects a wall or surviving body segment.
    Collision,

    /// The game was paused or already over.
    Noop,
}

/// Complete mutable state for one snake session.
///
/// The high score is session state and survives restarts. The score, snake, food, and status belong
/// to the current round. Keeping both in one Bevy resource makes the example easy to inspect while
/// still keeping rule updates local to this type. `bevy_ratatui` does not require one state
/// resource; this example uses one because input, timing, and rendering systems all need a shared
/// model and these methods keep Bevy scheduling out of the rules.
#[derive(Debug, Resource)]
pub struct Game {
    /// Logical bounds used by movement, collision, and food placement.
    board: Board,

    /// Logical snake geometry, latest movement snapshot, and accepted future turns.
    snake: Snake,

    /// Unoccupied board cell containing food, or `None` after filling the board.
    food: Option<Point>,

    /// Score accumulated during the current round.
    score: u32,

    /// Highest score reached by any round in this process.
    high_score: u32,

    /// Lifecycle state controlling whether logical movement can advance.
    status: GameStatus,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    /// Creates a game using the fallback board size.
    pub fn new() -> Self {
        Self::new_with_board_and_high_score(
            Board::new(DEFAULT_BOARD_WIDTH, DEFAULT_BOARD_HEIGHT),
            0,
        )
    }

    /// Creates a fresh round while carrying process-level score state across restarts.
    fn new_with_board_and_high_score(board: Board, high_score: u32) -> Self {
        let mut game = Self {
            board,
            snake: Snake::new(board),
            food: None,
            score: 0,
            high_score,
            status: GameStatus::Playing,
        };
        game.spawn_food();
        game
    }

    /// Starts a new round on the current board and keeps the session high score.
    pub fn restart(&mut self) {
        self.restart_on_board(self.board);
    }

    /// Starts a new round when the terminal-sized board changes.
    ///
    /// The example chooses restart-on-resize instead of scaling active coordinates. That keeps the
    /// game rules simple enough to study and avoids surprising partial snakes after a resize.
    ///
    /// Boards too narrow for [`Snake::new`] are ignored, as are unchanged boards. Returns whether
    /// the current round was restarted on a different board.
    pub fn resize_board(&mut self, board: Board) -> bool {
        if self.board != board && Snake::can_start_on(board) {
            self.restart_on_board(board);
            return true;
        }

        false
    }

    /// Replaces all round state while preserving the session high score.
    fn restart_on_board(&mut self, board: Board) {
        let high_score = self.high_score;
        *self = Self::new_with_board_and_high_score(board, high_score);
    }

    /// Toggles a live round between playing and paused.
    ///
    /// Game-over is intentionally stable: only an explicit restart creates another round.
    pub fn toggle_pause(&mut self) {
        self.status = match self.status {
            GameStatus::Playing => GameStatus::Paused,
            GameStatus::Paused => GameStatus::Playing,
            GameStatus::GameOver => GameStatus::GameOver,
        };
    }

    /// Queues a direction for the next movement step.
    ///
    /// The queue prevents dropped quick turns, while the reversal checks prevent the snake from
    /// turning directly into itself between ticks. Reversal is checked against the last queued turn
    /// when one exists, so `Right -> Up -> Left` is accepted but `Right -> Up -> Down` is rejected.
    pub fn queue_turn(&mut self, direction: Direction) -> bool {
        if self.status != GameStatus::Playing {
            return false;
        }

        self.snake.queue_turn(direction)
    }

    /// Advances the snake by one logical step.
    ///
    /// This method is the framework-independent rule boundary used by the Bevy game loop. It
    /// updates the current round and returns a small outcome that tests can assert without
    /// inspecting rendering state.
    pub fn step(&mut self) -> StepOutcome {
        if self.status != GameStatus::Playing {
            return StepOutcome::Noop;
        }

        self.snake.apply_next_turn();
        let next_head = self.snake.next_head();

        let ate_food = self.food == Some(next_head);
        if !self.board.contains(next_head) || self.snake.would_collide(next_head, ate_food) {
            return StepOutcome::Collision;
        }

        self.snake.advance(next_head, ate_food);
        if ate_food {
            self.score += POINTS_PER_FOOD * self.level();
            self.high_score = self.high_score.max(self.score);
            self.spawn_food();
            StepOutcome::AteFood
        } else {
            StepOutcome::Moved
        }
    }

    /// Returns the current movement cadence.
    ///
    /// Higher levels shorten the timer duration down to a fixed floor.
    pub fn step_duration(&self) -> Duration {
        let reduction = LEVEL_STEP_REDUCTION * self.level().saturating_sub(1);
        BASE_STEP_DURATION
            .saturating_sub(reduction)
            .max(MIN_STEP_DURATION)
    }

    /// Returns the one-based speed and scoring multiplier derived from the current score.
    pub fn level(&self) -> u32 {
        self.score / POINTS_PER_LEVEL + 1
    }

    /// Returns whether the game is currently accepting movement steps.
    pub fn is_playing(&self) -> bool {
        self.status == GameStatus::Playing
    }

    /// Returns the logical board used by the current round.
    pub fn board(&self) -> Board {
        self.board
    }

    /// Returns the logical snake and its latest movement snapshot.
    pub fn snake(&self) -> &Snake {
        &self.snake
    }

    /// Returns the current food cell, or `None` after filling the board.
    pub fn food(&self) -> Option<Point> {
        self.food
    }

    /// Returns the score accumulated during the current round.
    pub fn score(&self) -> u32 {
        self.score
    }

    /// Returns the highest score reached during this process.
    pub fn high_score(&self) -> u32 {
        self.high_score
    }

    /// Returns the current round lifecycle state.
    pub fn status(&self) -> GameStatus {
        self.status
    }

    /// Places food with process randomness after construction or growth.
    fn spawn_food(&mut self) {
        let mut rng = rand::rng();
        self.spawn_food_with_rng(&mut rng);
    }

    /// Places food using an injected random source so placement policy remains testable.
    fn spawn_food_with_rng(&mut self, rng: &mut impl Rng) {
        let free_cells = self.free_cells();
        if free_cells.is_empty() {
            self.food = None;
            self.end_game();
            return;
        }

        let index = rng.random_range(0..free_cells.len());
        self.food = Some(free_cells[index]);
    }

    /// Collects legal food positions in stable board order before random selection.
    ///
    /// Allocating here is acceptable for this example because food placement is infrequent and the
    /// explicit list makes the occupied-cell rule easier to inspect than rejection sampling.
    fn free_cells(&self) -> Vec<Point> {
        let mut cells = Vec::new();
        for y in 0..self.board.height() {
            for x in 0..self.board.width() {
                let point = Point { x, y };
                if !self.snake.contains(point) {
                    cells.push(point);
                }
            }
        }
        cells
    }

    /// Ends the current round and commits its score to session state.
    fn end_game(&mut self) {
        self.status = GameStatus::GameOver;
        self.high_score = self.high_score.max(self.score);
    }

    /// Ends the current round after the runtime rejects or expires a collision correction.
    pub fn end_after_collision(&mut self) {
        self.end_game();
    }
}

#[cfg(test)]
impl Game {
    /// Places deterministic food without invoking the random production policy.
    pub fn place_food_for_test(&mut self, point: Point) {
        self.food = Some(point);
    }

    /// Replaces snake geometry while preserving the invariants expected by movement and rendering.
    pub fn replace_snake_for_test(&mut self, snake: VecDeque<Point>, direction: Direction) {
        self.snake.replace_segments_for_test(snake, direction);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use crate::{
        geometry::{Board, Direction, Point},
        snake::STARTING_LENGTH,
    };

    /// Establishes score-dependent test scenarios without weakening production visibility.
    fn set_score(game: &mut Game, score: u32) {
        game.score = score;
        game.high_score = game.high_score.max(score);
    }

    #[test]
    fn initial_board_and_snake_are_valid() {
        let game = Game::new();

        assert_eq!(game.status(), GameStatus::Playing);
        assert_eq!(game.snake().segment_count(), STARTING_LENGTH);
        assert!(
            game.snake()
                .segments()
                .iter()
                .all(|point| game.board().contains(*point))
        );
        assert!(game.food().is_some_and(|food| !game.snake().contains(food)));
    }

    #[test]
    fn food_placement_avoids_the_snake() {
        let game = Game::new();

        assert!(game.food().is_some_and(|food| !game.snake().contains(food)));
    }

    #[test]
    fn movement_advances_the_head_and_preserves_length() {
        let mut game = Game::new();
        let original_head = game.snake().head();
        let original_len = game.snake().segment_count();
        game.place_food_for_test(Point::new(0, 0));

        assert_eq!(game.step(), StepOutcome::Moved);

        assert_eq!(game.snake().head(), original_head.offset(Direction::Right));
        assert_eq!(game.snake().segment_count(), original_len);
    }

    #[test]
    fn eating_food_grows_the_snake_and_scores() {
        let mut game = Game::new();
        let original_len = game.snake().segment_count();
        let food = game.snake().head().offset(Direction::Right);
        game.place_food_for_test(food);

        assert_eq!(game.step(), StepOutcome::AteFood);

        assert_eq!(game.snake().head(), food);
        assert_eq!(game.snake().segment_count(), original_len + 1);
        assert_eq!(game.score(), 10);
        assert_eq!(game.high_score(), 10);
    }

    #[test]
    fn reversing_direction_in_one_tick_is_rejected() {
        let mut game = Game::new();

        assert!(!game.queue_turn(Direction::Left));
        assert_eq!(game.snake().direction(), Direction::Right);

        let head = game.snake().head();
        game.step();

        assert_eq!(game.snake().head(), head.offset(Direction::Right));
    }

    #[test]
    fn quick_chained_turns_are_applied_on_consecutive_steps() {
        let mut game = Game::new();
        let head = game.snake().head();
        game.place_food_for_test(Point::new(0, 0));

        assert!(game.queue_turn(Direction::Up));
        assert!(game.queue_turn(Direction::Left));

        assert_eq!(game.step(), StepOutcome::Moved);
        assert_eq!(game.snake().head(), head.offset(Direction::Up));

        assert_eq!(game.step(), StepOutcome::Moved);
        assert_eq!(
            game.snake().head(),
            head.offset(Direction::Up).offset(Direction::Left)
        );
    }

    #[test]
    fn queued_turns_are_validated_against_the_previous_queued_turn() {
        let mut game = Game::new();

        assert!(game.queue_turn(Direction::Up));
        assert!(!game.queue_turn(Direction::Down));
        assert!(game.queue_turn(Direction::Left));
        assert!(!game.queue_turn(Direction::Down));
    }

    #[test]
    fn wall_collision_ends_the_game() {
        let mut game = Game::new();
        let snake = VecDeque::from([Point::new(game.board().width() - 1, 0)]);
        game.replace_snake_for_test(snake, Direction::Right);
        game.place_food_for_test(Point::new(0, 0));

        assert_eq!(game.step(), StepOutcome::Collision);
        assert_eq!(game.status(), GameStatus::Playing);

        game.end_after_collision();
        assert_eq!(game.status(), GameStatus::GameOver);
    }

    #[test]
    fn collision_can_be_corrected_by_a_late_valid_turn() {
        let mut game = Game::new();
        let snake = VecDeque::from([Point::new(game.board().width() - 1, 2)]);
        game.replace_snake_for_test(snake, Direction::Right);
        game.place_food_for_test(Point::new(0, 0));

        assert_eq!(game.step(), StepOutcome::Collision);

        assert!(game.queue_turn(Direction::Up));
        assert_eq!(game.step(), StepOutcome::Moved);
        assert_eq!(game.snake().head(), Point::new(game.board().width() - 1, 1));
        assert_eq!(game.status(), GameStatus::Playing);
    }

    #[test]
    fn self_collision_ends_the_game() {
        let mut game = Game::new();
        let snake = VecDeque::from([
            Point::new(5, 5),
            Point::new(5, 6),
            Point::new(4, 6),
            Point::new(4, 5),
            Point::new(5, 5),
        ]);
        game.replace_snake_for_test(snake, Direction::Down);
        game.place_food_for_test(Point::new(0, 0));

        assert_eq!(game.step(), StepOutcome::Collision);
        game.end_after_collision();
        assert_eq!(game.status(), GameStatus::GameOver);
    }

    #[test]
    fn restart_resets_game_and_preserves_high_score() {
        let mut game = Game::new();
        set_score(&mut game, 120);
        game.end_game();

        game.restart();

        assert_eq!(game.status(), GameStatus::Playing);
        assert_eq!(game.score(), 0);
        assert_eq!(game.high_score(), 120);
        assert_eq!(game.snake().segment_count(), STARTING_LENGTH);
    }

    #[test]
    fn resizing_restarts_on_the_new_board_and_preserves_high_score() {
        let mut game = Game::new();
        set_score(&mut game, 120);

        assert!(game.resize_board(Board::new(60, 40)));

        assert_eq!(game.board(), Board::new(60, 40));
        assert_eq!(game.status(), GameStatus::Playing);
        assert_eq!(game.score(), 0);
        assert_eq!(game.high_score(), 120);
        assert!(
            game.snake()
                .segments()
                .iter()
                .all(|point| game.board().contains(*point))
        );
    }

    #[test]
    fn resizing_to_the_same_board_does_not_restart() {
        let mut game = Game::new();
        set_score(&mut game, 120);
        let board = game.board();

        assert!(!game.resize_board(board));

        assert_eq!(game.score(), 120);
    }

    #[test]
    fn resizing_to_a_board_that_cannot_fit_the_snake_does_not_restart() {
        let mut game = Game::new();
        set_score(&mut game, 120);

        assert!(!game.resize_board(Board::new(5, 20)));

        assert_eq!(
            game.board(),
            Board::new(DEFAULT_BOARD_WIDTH, DEFAULT_BOARD_HEIGHT)
        );
        assert_eq!(game.score(), 120);
    }

    #[test]
    fn initial_step_duration_uses_the_base_speed() {
        let game = Game::new();

        assert_eq!(game.step_duration(), BASE_STEP_DURATION);
    }

    #[test]
    fn level_is_derived_from_score_thresholds() {
        let mut game = Game::new();
        assert_eq!(game.level(), 1);

        set_score(&mut game, POINTS_PER_LEVEL - 1);
        assert_eq!(game.level(), 1);

        set_score(&mut game, POINTS_PER_LEVEL);
        assert_eq!(game.level(), 2);
    }

    #[test]
    fn scoring_uses_the_level_before_food_is_added() {
        let mut game = Game::new();
        set_score(&mut game, POINTS_PER_LEVEL);
        let food = game.snake().head().offset(Direction::Right);
        game.place_food_for_test(food);

        assert_eq!(game.step(), StepOutcome::AteFood);
        assert_eq!(game.score(), POINTS_PER_LEVEL + POINTS_PER_FOOD * 2);
        assert_eq!(game.level(), 2);
    }

    #[test]
    fn movement_cadence_stops_at_the_minimum_duration() {
        let mut game = Game::new();
        set_score(&mut game, POINTS_PER_LEVEL * 100);

        assert_eq!(game.step_duration(), MIN_STEP_DURATION);
    }

    #[test]
    fn filling_the_board_ends_the_round_without_food() {
        let mut game = Game::new_with_board_and_high_score(Board::new(6, 1), 0);
        game.replace_snake_for_test(
            VecDeque::from([
                Point::new(4, 0),
                Point::new(3, 0),
                Point::new(2, 0),
                Point::new(1, 0),
                Point::new(0, 0),
            ]),
            Direction::Right,
        );
        game.place_food_for_test(Point::new(5, 0));

        assert_eq!(game.step(), StepOutcome::AteFood);
        assert_eq!(game.food(), None);
        assert_eq!(game.status(), GameStatus::GameOver);
    }
}
