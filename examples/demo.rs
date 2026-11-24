//! This app demonstrates:
//!
//! - Using ScheduleRunnerPlugin to run the bevy app loop without a window.
//! - Using the RatatuiContext resource to draw widgets to the terminal.
//! - Using Events to read input and communicate between systems.
//!
//! Keys:
//! - Left & Right: modify the counter
//! - Q or Esc: quit
//! - P: panic (tests the color_eyre panic hooks)

use core::panic;
#[cfg(not(feature = "windowed"))]
use std::time::Duration;

use bevy::{app::AppExit, diagnostic::FrameCount, prelude::*};
#[cfg(not(feature = "windowed"))]
use bevy::{app::ScheduleRunnerPlugin, state::app::StatesPlugin};
#[cfg(not(feature = "windowed"))]
use bevy_ratatui::event::KeyMessage;
use bevy_ratatui::{RatatuiContext, RatatuiPlugins};
#[cfg(not(feature = "windowed"))]
use ratatui::crossterm::event::KeyEventKind;
use ratatui::widgets::{FrameExt, Widget};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::WidgetRef,
};

fn main() -> Result<()> {
    color_eyre::install()?;

    let mut app = App::new();

    #[cfg(not(feature = "windowed"))]
    app.add_plugins((
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
            1. / 60.,
        ))),
        StatesPlugin,
        RatatuiPlugins::default(),
    ))
    .add_systems(PreUpdate, keyboard_input_system);

    #[cfg(feature = "windowed")]
    app.add_plugins((
        DefaultPlugins,
        RatatuiPlugins {
            enable_input_forwarding: true,
            ..default()
        },
    ))
    .add_systems(PreUpdate, keyboard_input_system_windowed);

    app.init_resource::<BackgroundColor>()
        .init_resource::<Counter>()
        .init_state::<AppState>()
        .add_message::<CounterMessage>()
        .add_systems(
            Update,
            (ui_system, update_counter_system, background_color_system),
        )
        .add_systems(OnEnter(AppState::Negative), start_background_color_timer)
        .add_systems(OnEnter(AppState::Positive), start_background_color_timer)
        .run();

    Ok(())
}

fn ui_system(
    mut context: ResMut<RatatuiContext>,
    frame_count: Res<FrameCount>,
    counter: Res<Counter>,
    app_state: Res<State<AppState>>,
    bg_color: Res<BackgroundColor>,
) -> Result {
    context.draw(|frame| {
        let area = frame.area();
        let frame_count = Line::from(format!("Frame Count: {}", frame_count.0)).right_aligned();
        frame.render_widget_ref(bg_color.as_ref(), area);
        frame.render_widget(frame_count, area);
        frame.render_widget_ref(counter.as_ref(), area);
        frame.render_widget_ref(app_state.get(), area)
    })?;

    Ok(())
}

#[cfg(not(feature = "windowed"))]
fn keyboard_input_system(
    mut key_messages: MessageReader<KeyMessage>,
    mut app_exit: MessageWriter<AppExit>,
    mut counter_messages: MessageWriter<CounterMessage>,
) {
    use ratatui::crossterm::event::KeyCode;
    for message in key_messages.read() {
        if let KeyEventKind::Release = message.kind {
            continue;
        }

        match message.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                app_exit.write_default();
            }
            KeyCode::Char('p') => {
                panic!("Panic!");
            }
            KeyCode::Left => {
                counter_messages.write(CounterMessage::Decrement);
            }
            KeyCode::Right => {
                counter_messages.write(CounterMessage::Increment);
            }
            _ => {}
        }
    }
}

#[cfg(feature = "windowed")]
fn keyboard_input_system_windowed(
    keys: Res<ButtonInput<KeyCode>>,
    mut app_exit: MessageWriter<AppExit>,
    mut counter_messages: MessageWriter<CounterMessage>,
) {
    if keys.just_pressed(KeyCode::KeyQ) {
        app_exit.write_default();
    }
    if keys.just_pressed(KeyCode::KeyP) {
        panic!("Panic!");
    }
    if keys.pressed(KeyCode::ArrowLeft) {
        counter_messages.write(CounterMessage::Decrement);
    }
    if keys.pressed(KeyCode::ArrowRight) {
        counter_messages.write(CounterMessage::Increment);
    }
}

#[derive(Default, Resource, Debug, Deref, DerefMut)]
struct Counter(i32);

impl Counter {
    fn decrement(&mut self) {
        self.0 -= 1;
    }

    fn increment(&mut self) {
        self.0 += 1;
    }
}

#[derive(Message, Clone, Copy, PartialEq, Eq, Debug)]
enum CounterMessage {
    Increment,
    Decrement,
}

fn update_counter_system(
    mut counter: ResMut<Counter>,
    mut counter_messages: MessageReader<CounterMessage>,
    mut app_state: ResMut<NextState<AppState>>,
) {
    for message in counter_messages.read() {
        match message {
            CounterMessage::Increment => counter.increment(),
            CounterMessage::Decrement => counter.decrement(),
        }
        // demonstrates changing something in the app state based on the counter value
        if counter.0 < 0 {
            app_state.set(AppState::Negative);
        } else {
            app_state.set(AppState::Positive);
        }
    }
}

impl WidgetRef for &Counter {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let counter = format!("Counter: {}", self.0);
        Line::from(counter).render(area, buf);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, States)]
enum AppState {
    Negative,
    #[default]
    Positive,
}

impl WidgetRef for &AppState {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let state = match self {
            AppState::Negative => "Negative",
            AppState::Positive => "Positive",
        };
        Line::from(state).centered().render(area, buf);
    }
}

#[derive(Debug, Component, Deref, DerefMut)]
struct ColorChangeTimer {
    #[deref]
    timer: Timer,
    start_color: Color,
}

fn start_background_color_timer(
    mut commands: Commands,
    bg_color: Res<BackgroundColor>,
    existing_timers: Query<Entity, With<ColorChangeTimer>>,
) {
    // Replace the fade that is still in flight rather than stacking a second one onto it.
    // `background_color_system` takes a `Single`, which matches only when exactly one timer
    // exists, so a second one stops it running. It is also the only system that despawns
    // finished timers, so once it stops the color stays frozen for the rest of the session.
    for entity in &existing_timers {
        commands.entity(entity).despawn();
    }
    commands.spawn(ColorChangeTimer {
        timer: Timer::from_seconds(2.0, TimerMode::Once),
        start_color: bg_color.0,
    });
}

#[derive(Debug, Resource, Deref, DerefMut)]
struct BackgroundColor(Color);

impl Default for BackgroundColor {
    fn default() -> Self {
        BackgroundColor(Color::Rgb(0, 0, 0))
    }
}

impl WidgetRef for &BackgroundColor {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, Style::new().bg(self.0));
    }
}

/// Change the background color over time when the app state changes from negative to positive
/// or vice versa.
fn background_color_system(
    time: Res<Time>,
    query: Single<(Entity, &mut ColorChangeTimer)>,
    app_state: Res<State<AppState>>,
    mut commands: Commands,
    mut bg_color: ResMut<BackgroundColor>,
) {
    let (entity, mut timer) = query.into_inner();
    timer.tick(time.delta());
    let end_color = match app_state.get() {
        AppState::Negative => Color::Rgb(191, 0, 0),
        AppState::Positive => Color::Rgb(0, 63, 128),
    };
    bg_color.0 = interpolate(timer.start_color, end_color, timer.fraction())
        .expect("only works for rgb colors");
    if timer.just_finished() {
        commands.entity(entity).despawn();
    }
}

/// Interpolate between two colors based on the fraction
///
/// This is just a simple linear interpolation between the two colors (a real implementation would
/// use a color space that is perceptually uniform).
fn interpolate(start: Color, end: Color, fraction: f32) -> Option<Color> {
    let Color::Rgb(start_red, start_green, start_blue) = start else {
        return None;
    };
    let Color::Rgb(end_red, end_green, end_blue) = end else {
        return None;
    };
    Some(Color::Rgb(
        (start_red as f32 + (end_red as f32 - start_red as f32) * fraction) as u8,
        (start_green as f32 + (end_green as f32 - start_green as f32) * fraction) as u8,
        (start_blue as f32 + (end_blue as f32 - start_blue as f32) * fraction) as u8,
    ))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::state::app::StatesPlugin;

    use super::*;

    /// Builds the counter/state/background-color half of the demo, without the systems that
    /// need a terminal.
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_resource::<Time>()
            .init_resource::<BackgroundColor>()
            .init_resource::<Counter>()
            .init_state::<AppState>()
            .add_message::<CounterMessage>()
            .add_systems(Update, (update_counter_system, background_color_system))
            .add_systems(OnEnter(AppState::Negative), start_background_color_timer)
            .add_systems(OnEnter(AppState::Positive), start_background_color_timer);
        app.update();
        app
    }

    fn advance(app: &mut App, seconds: f32) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(seconds));
        app.update();
    }

    fn flip(app: &mut App, message: CounterMessage) {
        app.world_mut().write_message(message);
        advance(app, 0.0);
    }

    fn background(app: &App) -> Color {
        app.world().resource::<BackgroundColor>().0
    }

    fn timer_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<ColorChangeTimer>>()
            .iter(app.world())
            .count()
    }

    const NEGATIVE: Color = Color::Rgb(191, 0, 0);
    const POSITIVE: Color = Color::Rgb(0, 63, 128);

    #[test]
    fn fade_completes_when_left_alone() {
        let mut app = test_app();
        advance(&mut app, 2.5);
        assert_eq!(background(&app), POSITIVE);
    }

    /// Regression test: flipping the sign before the previous fade finished used to leave two
    /// `ColorChangeTimer` entities alive. `background_color_system` takes a `Single`, which
    /// matches only when exactly one exists, so it stopped running — and because it is also the
    /// only system that despawns finished timers, the color stayed frozen forever.
    #[test]
    fn flipping_sign_mid_fade_still_reaches_the_new_color() {
        let mut app = test_app();
        advance(&mut app, 0.5); // part-way through the initial fade to positive
        flip(&mut app, CounterMessage::Decrement); // -1, queues the switch to Negative
        advance(&mut app, 0.0); // the transition applies and starts the new fade

        assert_eq!(
            timer_count(&mut app),
            1,
            "the in-flight fade's timer was left alive alongside the new one"
        );

        advance(&mut app, 2.5);
        assert_eq!(background(&app), NEGATIVE);
    }

    #[test]
    fn repeated_early_flips_keep_animating() {
        let mut app = test_app();
        for message in [
            CounterMessage::Decrement,
            CounterMessage::Increment,
            CounterMessage::Decrement,
        ] {
            advance(&mut app, 0.2);
            flip(&mut app, message);
        }
        advance(&mut app, 2.5);
        assert_eq!(background(&app), NEGATIVE);
    }
}
