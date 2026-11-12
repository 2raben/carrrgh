//! Fixed road view: resizing preserves the visible road and reaction distance.
use bevy::camera::ScalingMode;
use bevy::prelude::*;

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, |mut commands: Commands| {
            commands.spawn((
                Camera2d,
                Projection::Orthographic(OrthographicProjection {
                    scaling_mode: ScalingMode::Fixed {
                        width: 960.0,
                        height: 800.0,
                    },
                    ..OrthographicProjection::default_2d()
                }),
            ));
        });
    }
}
