//! Logical snake state and movement.
//!
//! A snake is more than its occupied cells: its direction, accepted future turns, and latest
//! movement must change together. Keeping those invariants on [`Snake`] lets [`crate::game::Game`]
//! coordinate board, food, and scoring rules without also managing the representation of movement.
//!
//! Rendering reads [`MovementSnapshot`] to interpolate the moving ends. Collision rules only read
//! the current segments, preserving a whole-cell simulation regardless of frame rate.

use std::collections::VecDeque;

use crate::geometry::{Board, Direction, Point};

/// Number of contiguous cells in a new snake.
pub const STARTING_LENGTH: usize = 4;

/// Number of future turns retained between logical movement steps.
///
/// Two entries preserve a quick corner such as `Up, Left`. A longer queue would make the snake
/// execute old input after the player can no longer see why it was accepted.
const MAX_QUEUED_TURNS: usize = 2;

/// Snake geometry and the input needed to advance it.
///
/// Segments are ordered from head to tail. [`MovementSnapshot`] records only the endpoints needed
/// for interpolation; movement and collision always use `segments`. At most one queued turn is
/// applied per logical step so quick corners remain responsive without collapsing several inputs
/// into one move.
#[derive(Debug)]
pub struct Snake {
    /// Cells occupied after the latest successful logical step.
    segments: VecDeque<Point>,

    /// Endpoints occupied before the latest successful logical step.
    movement: MovementSnapshot,

    /// Direction applied by the latest logical step.
    direction: Direction,

    /// Accepted future turns, ordered by the steps on which they will be applied.
    queued_turns: VecDeque<Direction>,
}

/// Previous endpoints needed to interpolate one successful logical movement.
///
/// The head moves on every successful step. `previous_tail` is present only when the tail also
/// moved; growth leaves the tail stationary and therefore needs no tail interpolation. This compact
/// snapshot avoids cloning the complete body for rendering.
#[derive(Clone, Copy, Debug)]
pub struct MovementSnapshot {
    /// Head position before the latest successful step.
    previous_head: Point,

    /// Tail position before the latest step, when that step moved the tail.
    previous_tail: Option<Point>,
}

impl MovementSnapshot {
    /// Returns the head position before the latest successful step.
    pub fn previous_head(self) -> Point {
        self.previous_head
    }

    /// Returns the prior tail position when the latest step moved the tail.
    pub fn previous_tail(self) -> Option<Point> {
        self.previous_tail
    }
}

impl Snake {
    /// Returns whether `board` can contain the initial horizontal snake.
    pub fn can_start_on(board: Board) -> bool {
        let head_x = board.width() / 2;
        head_x >= STARTING_LENGTH as i16 - 1
    }

    /// Creates a right-facing snake centered on `board`.
    ///
    /// # Panics
    ///
    /// Panics when [`Self::can_start_on`] is false. Terminal-derived boards are substantially wider
    /// than this minimum; callers accepting arbitrary boards should check before restarting.
    pub fn new(board: Board) -> Self {
        assert!(
            Self::can_start_on(board),
            "board must fit the starting snake"
        );
        let head = Point::new(board.width() / 2, board.height() / 2);
        let segments = (0..STARTING_LENGTH as i16)
            .map(|offset| Point::new(head.x - offset, head.y))
            .collect::<VecDeque<_>>();
        let tail = segments.back().copied();

        Self {
            movement: MovementSnapshot {
                previous_head: head,
                previous_tail: tail,
            },
            segments,
            direction: Direction::Right,
            queued_turns: VecDeque::new(),
        }
    }

    /// Queues a non-reversing turn for a future logical step.
    ///
    /// Validation uses the newest queued turn when one exists. This accepts
    /// `Right -> Up -> Left` but rejects `Right -> Up -> Down`, which would reverse the second turn
    /// before it had been rendered.
    pub fn queue_turn(&mut self, direction: Direction) -> bool {
        if self.queued_turns.len() >= MAX_QUEUED_TURNS {
            return false;
        }

        let previous_direction = self.queued_turns.back().copied().unwrap_or(self.direction);
        if direction == previous_direction || direction.is_opposite(previous_direction) {
            return false;
        }

        self.queued_turns.push_back(direction);
        true
    }

    /// Applies at most one queued turn before the next logical movement.
    pub fn apply_next_turn(&mut self) {
        if let Some(direction) = self.queued_turns.pop_front() {
            self.direction = direction;
        }
    }

    /// Returns the head position proposed by the current direction.
    pub fn next_head(&self) -> Point {
        self.head().offset(self.direction)
    }

    /// Commits a collision-free move to `next_head` and optionally keeps the tail for growth.
    pub fn advance(&mut self, next_head: Point, grows: bool) {
        let previous_head = self.head();
        let previous_tail = if !grows && self.segments.len() > 1 {
            self.segments.back().copied()
        } else {
            None
        };
        self.movement = MovementSnapshot {
            previous_head,
            previous_tail,
        };

        self.segments.push_front(next_head);
        if !grows {
            self.segments.pop_back();
        }
    }

    /// Returns whether `point` intersects body that would survive the proposed move.
    ///
    /// Entering the current tail cell is legal when the tail advances. Growth keeps the tail in
    /// place, so every current segment remains collidable on that step.
    pub fn would_collide(&self, point: Point, grows: bool) -> bool {
        let collidable_segments = if grows {
            self.segments.len()
        } else {
            self.segments.len().saturating_sub(1)
        };

        self.segments
            .iter()
            .take(collidable_segments)
            .any(|segment| *segment == point)
    }

    /// Returns the current segments from head to tail.
    pub fn segments(&self) -> &VecDeque<Point> {
        &self.segments
    }

    /// Returns the endpoints before the latest successful logical step.
    pub fn movement(&self) -> MovementSnapshot {
        self.movement
    }

    /// Returns the current head.
    pub fn head(&self) -> Point {
        self.segments
            .front()
            .copied()
            .expect("snake always has a head")
    }

    /// Returns the direction used by the latest logical step.
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// Returns the number of occupied body cells, including the head.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Returns whether any segment occupies `point`.
    pub fn contains(&self, point: Point) -> bool {
        self.segments.iter().any(|segment| *segment == point)
    }

    /// Returns whether a queued turn is waiting for a logical step.
    pub fn has_queued_turn(&self) -> bool {
        !self.queued_turns.is_empty()
    }
}

#[cfg(test)]
impl Snake {
    /// Replaces the path and direction while restoring movement invariants for focused tests.
    pub fn replace_segments_for_test(&mut self, segments: VecDeque<Point>, direction: Direction) {
        let previous_head = segments
            .front()
            .copied()
            .expect("test snake must have a head");
        let previous_tail = if segments.len() > 1 {
            segments.back().copied()
        } else {
            None
        };
        self.movement = MovementSnapshot {
            previous_head,
            previous_tail,
        };
        self.segments = segments;
        self.direction = direction;
        self.queued_turns.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_snake_is_contiguous_and_inside_the_board() {
        let board = Board::new(24, 20);
        let snake = Snake::new(board);

        assert_eq!(snake.segment_count(), STARTING_LENGTH);
        assert!(snake.segments().iter().all(|point| board.contains(*point)));
        assert!(
            snake
                .segments()
                .iter()
                .zip(snake.segments().iter().skip(1))
                .all(|(left, right)| left.x == right.x + 1 && left.y == right.y)
        );
    }

    #[test]
    fn starting_snake_requires_enough_space_left_of_center() {
        assert!(!Snake::can_start_on(Board::new(5, 20)));
        assert!(Snake::can_start_on(Board::new(6, 20)));
    }

    #[test]
    fn queued_turns_are_applied_one_step_at_a_time() {
        let mut snake = Snake::new(Board::new(24, 20));
        assert!(snake.queue_turn(Direction::Up));
        assert!(snake.queue_turn(Direction::Left));

        snake.apply_next_turn();
        let first_head = snake.next_head();
        snake.advance(first_head, false);
        assert_eq!(snake.direction(), Direction::Up);

        snake.apply_next_turn();
        let second_head = snake.next_head();
        snake.advance(second_head, false);
        assert_eq!(snake.direction(), Direction::Left);
    }

    #[test]
    fn ordinary_movement_records_both_previous_endpoints() {
        let mut snake = Snake::new(Board::new(24, 20));
        let previous_head = snake.head();
        let previous_tail = snake.segments().back().copied();

        snake.advance(snake.next_head(), false);

        assert_eq!(snake.movement().previous_head(), previous_head);
        assert_eq!(snake.movement().previous_tail(), previous_tail);
    }

    #[test]
    fn growth_records_that_the_tail_did_not_move() {
        let mut snake = Snake::new(Board::new(24, 20));

        snake.advance(snake.next_head(), true);

        assert_eq!(snake.movement().previous_tail(), None);
    }

    #[test]
    fn queue_rejects_duplicates_reversals_and_excess_turns() {
        let mut snake = Snake::new(Board::new(24, 20));

        assert!(!snake.queue_turn(Direction::Right));
        assert!(!snake.queue_turn(Direction::Left));
        assert!(snake.queue_turn(Direction::Up));
        assert!(!snake.queue_turn(Direction::Down));
        assert!(snake.queue_turn(Direction::Left));
        assert!(!snake.queue_turn(Direction::Down));
    }
}
