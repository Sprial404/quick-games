use ggez;
use rand;

use ggez::event::{KeyCode, KeyMods};
use ggez::graphics::{DrawMode, Scale, Text, TextFragment};
use ggez::{event, graphics, Context, GameResult};

use std::collections::LinkedList;
use std::time::{Duration, Instant};

use rand::Rng;

const GRID_SIZE: (i16, i16) = (30, 20);
const GRID_CELL_SIZE: (i16, i16) = (32, 32);

const SCREEN_SIZE: (f32, f32) = (
    GRID_SIZE.0 as f32 * GRID_CELL_SIZE.0 as f32,
    GRID_SIZE.1 as f32 * GRID_CELL_SIZE.1 as f32,
);

const UPDATES_PER_SECOND: f32 = 8.0;
const MILLIS_PER_UPDATE: u64 = (1.0 / UPDATES_PER_SECOND * 1000.0) as u64;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct GridPosition {
    x: i16,
    y: i16,
}

impl GridPosition {
    /// Creates a new grid position.
    pub fn new(x: i16, y: i16) -> Self {
        GridPosition { x, y }
    }

    /// Creates a new random grid position from the range of `(0, 0)` to `(max_x, max_y)`.
    pub fn random(max_x: i16, max_y: i16) -> Self {
        let mut rng = rand::thread_rng();

        (
            rng.gen_range::<i16, i16, i16>(0, max_x),
            rng.gen_range::<i16, i16, i16>(0, max_y),
        )
            .into()
    }

    /// Move grid position by the given direction and wrap arround the board.
    pub fn wrapped_move(pos: GridPosition, dir: Direction) -> Self {
        match dir {
            Direction::Up => GridPosition::new(pos.x, (pos.y - 1).rem_euclid(GRID_SIZE.1)),
            Direction::Down => GridPosition::new(pos.x, (pos.y + 1).rem_euclid(GRID_SIZE.1)),
            Direction::Left => GridPosition::new((pos.x - 1).rem_euclid(GRID_SIZE.0), pos.y),
            Direction::Right => GridPosition::new((pos.x + 1).rem_euclid(GRID_SIZE.0), pos.y),
        }
    }
}

/// Implement `From` trait for `graphics::Rect` so it easily converts a grid position
/// into a grid cell.
impl From<GridPosition> for graphics::Rect {
    fn from(pos: GridPosition) -> Self {
        graphics::Rect::new_i32(
            pos.x as i32 * GRID_CELL_SIZE.0 as i32,
            pos.y as i32 * GRID_CELL_SIZE.1 as i32,
            GRID_CELL_SIZE.0 as i32,
            GRID_CELL_SIZE.1 as i32,
        )
    }
}

impl From<(i16, i16)> for GridPosition {
    fn from(pos: (i16, i16)) -> Self {
        GridPosition { x: pos.0, y: pos.1 }
    }
}

/// Represents all possible directions that our snake can move.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    /// Returns the inverse `Direction` of the current.
    pub fn inverse(&self) -> Self {
        match *self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }

    /// Converts from `ggez::Keycode` to a `Direction` that it represents, or it returns `None`.
    pub fn from_keycode(key: KeyCode) -> Option<Self> {
        match key {
            KeyCode::Up => Some(Direction::Up),
            KeyCode::Down => Some(Direction::Down),
            KeyCode::Left => Some(Direction::Left),
            KeyCode::Right => Some(Direction::Right),
            _ => None,
        }
    }
}

/// A segment of the snake.
#[derive(Debug, Copy, Clone)]
struct Segment {
    pos: GridPosition,
}

impl Segment {
    /// Creates a new `Segment` with the `col` and at the `pos`.
    pub fn new(pos: GridPosition) -> Self {
        Segment { pos }
    }
}

/// A piece of food the snake can eat.
#[derive(Debug, Copy, Clone)]
struct Food {
    pos: GridPosition,
}

impl Food {
    /// Creates a new `Food` at the given `pos`.
    pub fn new(pos: GridPosition) -> Self {
        Food { pos }
    }

    fn draw(&self, ctx: &mut Context) -> GameResult<()> {
        let color = [1.0, 0.0, 0.0, 1.0].into();

        let rect = graphics::Mesh::new_rectangle(ctx, DrawMode::fill(), self.pos.into(), color)?;
        graphics::draw(ctx, &rect, (ggez::mint::Point2 { x: 0.0, y: 0.0 },))
    }
}

/// Represents all possible things the snake could have "eaten" during an update. Either being a
/// piece of `Food`, or it may have eaten `Itself` if it ran into its body.
#[derive(Debug, Copy, Clone)]
enum Ate {
    Itself,
    Food,
}

/// The snake entity that the player controls to direct it to the food to grow the snake and avoid
/// hitting into itself and dying.
#[derive(Debug)]
struct Snake {
    /// The head of the snake.
    head: Segment,
    /// The current direction the snake will move in the next `update`.
    dir: Direction,
    /// The body of the snake.
    body: LinkedList<Segment>,
    /// The last update of whether the snake ate Itself (`Some(Ate::Itself)`), Food
    /// (`Some(Ate::Food)`), or nothing (`None`).
    ate: Option<Ate>,
    /// The direction the snake previously travelled in the last `update`. Used to determine the
    /// possible valid directions of the next move.
    last_update_dir: Direction,
    /// Stores the next direction that the snake will travel in the next `update` after. Used to
    /// allow the user to choose two directions (e.g., left than up).
    next_dir: Option<Direction>,
}

impl Snake {
    /// Creates a new snake from the pos with one head and body segment moving to the right.
    pub fn new(pos: GridPosition) -> Self {
        let mut body = LinkedList::new();

        body.push_back(Segment::new((pos.x - 1, pos.y).into()));
        Snake {
            head: Segment::new((pos.x, pos.y).into()),
            dir: Direction::Right,
            last_update_dir: Direction::Right,
            body,
            ate: None,
            next_dir: None,
        }
    }

    fn eats(&self, food: &Food) -> bool {
        self.head.pos == food.pos
    }

    fn eats_self(&self) -> bool {
        for seg in self.body.iter() {
            if self.head.pos == seg.pos {
                return true;
            }
        }
        false
    }

    fn update(&mut self, food: &Food) {
        if self.last_update_dir == self.dir && self.next_dir.is_some() {
            self.dir = self.next_dir.unwrap();
            self.next_dir = None;
        }

        let new_head_pos = GridPosition::wrapped_move(self.head.pos, self.dir);
        let new_head = Segment::new(new_head_pos);

        // Grow the snake by pushing the current head `Segment` to the front of our body.
        self.body.push_front(self.head);
        self.head = new_head;

        self.ate = if self.eats_self() {
            Some(Ate::Itself)
        } else if self.eats(food) {
            Some(Ate::Food)
        } else {
            None
        };

        // If we didn't eat anything this `update`, we remove the last segment from our body, which
        // gives the illusion that the snake is moving.
        if self.ate.is_none() {
            self.body.pop_back();
        }

        self.last_update_dir = self.dir;
    }

    fn draw(&self, ctx: &mut Context) -> GameResult<()> {
        for seg in self.body.iter() {
            let rect = graphics::Mesh::new_rectangle(
                ctx,
                DrawMode::fill(),
                seg.pos.into(),
                [1.0, 1.0, 1.0, 1.0].into(),
            )?;
            graphics::draw(ctx, &rect, (ggez::mint::Point2 { x: 0.0, y: 0.0 },))?;
        }

        let rect = graphics::Mesh::new_rectangle(
            ctx,
            DrawMode::stroke(5.0),
            self.head.pos.into(),
            [1.0, 1.0, 1.0, 1.0].into(),
        )?;
        graphics::draw(ctx, &rect, (ggez::mint::Point2 { x: 0.0, y: 0.0 },))
    }
}

/// The state for the game.
struct GameState {
    snake: Snake,
    food: Food,
    gameover: bool,
    last_update: Instant,
}

impl GameState {
    /// Creates a new game state.
    pub fn new() -> Self {
        let snake_pos = (GRID_SIZE.0 / 4, GRID_SIZE.1 / 2).into();
        let food_pos = GridPosition::random(GRID_SIZE.0, GRID_SIZE.1);

        GameState {
            snake: Snake::new(snake_pos),
            food: Food::new(food_pos),
            gameover: false,
            last_update: Instant::now(),
        }
    }
}

impl event::EventHandler for GameState {
    fn update(&mut self, _ctx: &mut Context) -> GameResult<()> {
        // Check if enough time has elapsed since the last update.
        if Instant::now() - self.last_update >= Duration::from_millis(MILLIS_PER_UPDATE) {
            if !self.gameover {
                self.snake.update(&self.food);

                if let Some(ate) = self.snake.ate {
                    match ate {
                        Ate::Food => {
                            let new_food_pos = GridPosition::random(GRID_SIZE.0, GRID_SIZE.1);
                            self.food.pos = new_food_pos;
                        }
                        Ate::Itself => {
                            self.gameover = true;
                        }
                    }
                }
            }

            self.last_update = Instant::now();
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx, [0.0, 0.0, 0.0, 1.0].into());
        self.snake.draw(ctx)?;
        self.food.draw(ctx)?;

        if self.gameover {
            let game_over = Text::new(
                TextFragment::new("GAME OVER!")
                    .color([1.0, 0.0, 0.0, 1.0].into())
                    .scale(Scale::uniform(40.0)),
            );

            graphics::draw(ctx, &game_over, (ggez::mint::Point2 { x: 0.0, y: 0.0 },))?;
        }

        graphics::present(ctx)?;
        ggez::timer::yield_now();
        Ok(())
    }

    fn key_down_event(
        &mut self,
        _ctx: &mut Context,
        keycode: KeyCode,
        _keymod: KeyMods,
        _repeat: bool,
    ) {
        if let Some(dir) = Direction::from_keycode(keycode) {
            if self.snake.dir != self.snake.last_update_dir && dir.inverse() != self.snake.dir {
                self.snake.next_dir = Some(dir);
            } else if dir.inverse() != self.snake.last_update_dir {
                self.snake.dir = dir;
            }
        }

        if self.gameover {
            let snake_pos = (GRID_SIZE.0 / 4, GRID_SIZE.1 / 2).into();
            let food_pos = GridPosition::random(GRID_SIZE.0, GRID_SIZE.1);
            self.snake = Snake::new(snake_pos);
            self.food = Food::new(food_pos);
            self.gameover = false;
        }
    }
}

fn main() -> GameResult {
    let (ctx, events_loop) = &mut ggez::ContextBuilder::new("snake", "Sprial404")
        .window_setup(ggez::conf::WindowSetup::default().title("Snake"))
        .window_mode(ggez::conf::WindowMode::default().dimensions(SCREEN_SIZE.0, SCREEN_SIZE.1))
        .build()?;
    let state = &mut GameState::new();
    event::run(ctx, events_loop, state)
}
