use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::WindowResized,
};

use crate::RatatuiContext;

/// A plugin that, rather than drawing to a terminal buffer, uses software rendering to build a 2D
/// texture from the ratatui buffer, and displays the result in a window.
pub struct WindowedPlugin;

impl Plugin for WindowedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, terminal_render_setup)
            .add_systems(PreUpdate, handle_resize_messages)
            .add_systems(Update, render_terminal_to_handle);
    }
}

#[derive(Resource)]
struct TerminalRender(Handle<Image>);

/// A startup system that sets up the terminal
pub fn terminal_render_setup(
    mut commands: Commands,
    softatui: ResMut<RatatuiContext>,
    mut images: ResMut<Assets<Image>>,
) -> Result {
    commands.spawn(Camera2d);
    // Create an image that we are going to draw into
    let width = softatui.backend().get_pixmap_width() as u32;
    let height = softatui.backend().get_pixmap_height() as u32;
    let data = softatui.backend().get_pixmap_data_as_rgba();

    let image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    let handle = images.add(image);
    commands.spawn((
        ImageNode::new(handle.clone()),
        Node {
            justify_self: JustifySelf::Center,
            align_self: AlignSelf::Center,
            ..default()
        },
    ));

    commands.insert_resource(TerminalRender(handle));

    Ok(())
}

/// System that updates the terminal texture each frame
fn render_terminal_to_handle(
    softatui: ResMut<RatatuiContext>,
    mut images: ResMut<Assets<Image>>,
    my_handle: Res<TerminalRender>,
) {
    let width = softatui.backend().get_pixmap_width() as u32;
    let height = softatui.backend().get_pixmap_height() as u32;

    let image = images.get_mut(&my_handle.0).expect("Image not found");
    if image.width() != width || image.height() != height {
        image.resize(Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        });
    }

    let data = softatui.backend().get_pixmap_data();
    let image_data = image.data.as_mut().expect("Image data missing");
    let (pixels, _) = data.as_chunks::<3>();
    for (i, [r, g, b]) in pixels.into_iter().copied().enumerate() {
        image_data[i * 4] = r;
        image_data[i * 4 + 1] = g;
        image_data[i * 4 + 2] = b;
        image_data[i * 4 + 3] = 255;
    }
}

/// System that reacts to window resize
fn handle_resize_messages(
    mut resize_reader: MessageReader<WindowResized>,
    mut softatui: ResMut<RatatuiContext>,
) {
    for message in resize_reader.read() {
        let cur_pix_width = softatui.backend().char_width;
        let cur_pix_height = softatui.backend().char_height;
        let av_wid = (message.width / cur_pix_width as f32) as u16;
        let av_hei = (message.height / cur_pix_height as f32) as u16;
        softatui.backend_mut().resize(av_wid, av_hei);
    }
}
