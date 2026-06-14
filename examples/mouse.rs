use bevy::{
    app::{AppExit, ScheduleRunnerPlugin},
    prelude::*,
};
use bevy_ratatui::{RatatuiContext, RatatuiPlugins, event::KeyMessage, event::MouseMessage};
use rand::prelude::*;
use ratatui::crossterm::event::MouseEventKind;

fn main() -> Result<()> {
    color_eyre::install()?;

    let wait_duration = std::time::Duration::from_secs_f64(1. / 60.); // 60 FPS
    App::new()
        .add_plugins(RatatuiPlugins {
            enable_mouse_capture: true,
            ..default()
        })
        .add_plugins(ScheduleRunnerPlugin::run_loop(wait_duration))
        .add_systems(PreUpdate, keyboard_input_system)
        .add_systems(Update, mouse_input_system)
        .add_systems(Update, (move_balls, bounce_balls.chain()))
        .add_systems(PostUpdate, draw_balls)
        .run();

    Ok(())
}

fn keyboard_input_system(
    mut key_messages: MessageReader<KeyMessage>,
    mut exit: MessageWriter<AppExit>,
) {
    use ratatui::crossterm::event::KeyCode;
    for message in key_messages.read() {
        match message.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                exit.write_default();
            }
            _ => {}
        }
    }
}

#[derive(Debug, Component)]
struct Ball;

#[derive(Debug, Component, Deref, DerefMut)]
struct Color(ratatui::style::Color);

impl Color {
    fn random() -> Self {
        let mut rng = rand::rng();
        Self(ratatui::style::Color::Rgb(
            rng.random_range(0..255),
            rng.random_range(0..255),
            rng.random_range(0..255),
        ))
    }
}

#[derive(Debug, Component)]
struct Position {
    x: f32,
    y: f32,
}

impl Position {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Component)]
struct Velocity {
    x: f32,
    y: f32,
}

impl Velocity {
    fn random() -> Self {
        let mut rng = rand::rng();
        Self {
            x: rng.random_range(-1.0..1.0),
            y: rng.random_range(-1.0..1.0),
        }
    }
}

fn move_balls(mut query: Query<(&Ball, &mut Position, &Velocity)>) {
    for (_, mut position, velocity) in query.iter_mut() {
        position.x += velocity.x * 0.01;
        position.y += velocity.y * 0.01;
    }
}

fn bounce_balls(mut query: Query<(&Ball, &mut Position, &mut Velocity)>) {
    for (_, mut position, mut velocity) in query.iter_mut() {
        if position.x < 0.0 || position.x > 1.0 {
            velocity.x *= -1.0;
        }
        if position.y < 0.0 || position.y > 1.0 {
            velocity.y *= -1.0;
        }
        if position.x < 0.0 {
            position.x = -position.x;
        } else if position.x > 1.0 {
            position.x = 2.0 - position.x;
        }
        if position.y < 0.0 {
            position.y = -position.y;
        } else if position.y > 1.0 {
            position.y = 2.0 - position.y;
        }
    }
}

fn draw_balls(mut context: ResMut<RatatuiContext>, query: Query<(&Ball, &Position, &Color)>) {
    let _ = context.draw(|frame| {
        let area = frame.area();
        let buf = frame.buffer_mut();
        let count = query.iter().count();
        for (_, position, color) in query.iter() {
            let x = ((position.x * area.width as f32) as u16).min(area.width - 1);
            let y = ((position.y * area.height as f32) as u16).min(area.height - 1);
            buf[(x, y)].set_symbol("●").set_fg(**color);
        }
        frame.render_widget(format!("count: {count}"), area);
    });
}

fn mouse_input_system(
    mut messages: MessageReader<MouseMessage>,
    mut commands: Commands,
    context: Res<RatatuiContext>,
) {
    for message in messages.read() {
        let ratatui::crossterm::event::MouseEvent {
            kind, column, row, ..
        } = message.0;
        let size = context.size().unwrap(); // TODO: handle error properly
        let column = column as f32 / size.width as f32;
        let row = row as f32 / size.height as f32;
        if let MouseEventKind::Moved = kind {
            commands.spawn((
                Ball,
                Position::new(column, row),
                Velocity::random(),
                Color::random(),
            ));
        }
    }
}
