//! Board geometry and movement directions.
//!
//! These types are deliberately small value types. The rule layer stores snake segments as
//! [`Point`] values on a [`Board`], controls produce [`Direction`] values, and the terminal layer
//! decides how those logical cells map to terminal pixels.

/// Cardinal movement direction for the snake.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    /// Decreases the board row.
    Up,
    /// Increases the board row.
    Down,
    /// Decreases the board column.
    Left,
    /// Increases the board column.
    Right,
}

impl Direction {
    /// Returns whether two directions would immediately reverse the snake.
    pub fn is_opposite(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Up, Self::Down)
                | (Self::Down, Self::Up)
                | (Self::Left, Self::Right)
                | (Self::Right, Self::Left)
        )
    }
}

/// Logical board dimensions in snake cells.
///
/// The terminal renderer decides how these logical cells map onto terminal cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Board {
    /// Number of logical snake cells from left to right.
    width: i16,

    /// Number of logical snake cells from top to bottom.
    height: i16,
}

impl Board {
    /// Creates a board with positive logical dimensions.
    ///
    /// Use [`Self::try_new`] when dimensions come from an external source rather than constants or
    /// other already-validated application state.
    ///
    /// # Panics
    ///
    /// Panics when either dimension is zero or negative.
    pub fn new(width: i16, height: i16) -> Self {
        Self::try_new(width, height).expect("board dimensions must be positive")
    }

    /// Creates a board when both logical dimensions are positive.
    pub fn try_new(width: i16, height: i16) -> Option<Self> {
        (width > 0 && height > 0).then_some(Self { width, height })
    }

    /// Returns the number of logical cells from left to right.
    pub fn width(self) -> i16 {
        self.width
    }

    /// Returns the number of logical cells from top to bottom.
    pub fn height(self) -> i16 {
        self.height
    }

    /// Returns whether `point` is inside the board bounds.
    pub fn contains(self, point: Point) -> bool {
        point.x >= 0 && point.y >= 0 && point.x < self.width && point.y < self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_constructor_rejects_non_positive_dimensions() {
        assert_eq!(Board::try_new(0, 20), None);
        assert_eq!(Board::try_new(20, 0), None);
        assert_eq!(Board::try_new(-1, 20), None);
    }
}

/// Coordinate in logical board cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Point {
    /// Zero-based logical column, increasing to the right.
    pub x: i16,
    /// Zero-based logical row, increasing downward with terminal coordinates.
    pub y: i16,
}

impl Point {
    /// Creates a point at `x, y`.
    pub fn new(x: i16, y: i16) -> Self {
        Self { x, y }
    }

    /// Returns the neighboring point reached by moving one cell in `direction`.
    pub fn offset(self, direction: Direction) -> Self {
        match direction {
            Direction::Up => Self::new(self.x, self.y - 1),
            Direction::Down => Self::new(self.x, self.y + 1),
            Direction::Left => Self::new(self.x - 1, self.y),
            Direction::Right => Self::new(self.x + 1, self.y),
        }
    }
}
