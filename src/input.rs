//! Device input: keyboard and gamepad state becomes [`Controls`] for the
//! fixed-step simulation. This is the only module that reads devices.

use bevy::input::InputSystems;
use bevy::prelude::*;

use crate::player::Controls;
use crate::run::Run;

/// Stick deflection below this is ignored.
const STICK_DEAD_ZONE: f32 = 0.25;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        // Sample before the fixed-step loop so this frame's presses reach this
        // frame's fixed steps.
        app.add_systems(PreUpdate, (sample_controls, quit).after(InputSystems));
    }
}

/// Keys are physical positions, so WASD stays in place on other layouts.
fn sample_controls(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    run: Res<Run>,
    mut controls: ResMut<Controls>,
) {
    let any_key = |codes: &[KeyCode]| keys.any_pressed(codes.iter().copied());
    let any_key_just = |codes: &[KeyCode]| keys.any_just_pressed(codes.iter().copied());
    const LEFT: &[KeyCode] = &[KeyCode::KeyA, KeyCode::ArrowLeft];
    const RIGHT: &[KeyCode] = &[KeyCode::KeyD, KeyCode::ArrowRight];
    const BOOST: &[KeyCode] = &[KeyCode::Space];
    const RESTART: &[KeyCode] = &[KeyCode::KeyR, KeyCode::Backspace];

    let mut move_x = f32::from(u8::from(any_key(RIGHT))) - f32::from(u8::from(any_key(LEFT)));
    let mut boost = any_key(BOOST);
    let mut restart = any_key_just(RESTART);
    for pad in &gamepads {
        let stick = pad.left_stick().x;
        if stick.abs() > STICK_DEAD_ZONE {
            move_x += stick;
        }
        move_x += pad.dpad().x;
        boost |= pad.pressed(GamepadButton::South);
        restart |= pad.just_pressed(GamepadButton::Select);
    }

    // After the drive ends, only Restart is accepted.
    let playing = !run.finished;
    controls.move_x = if playing {
        move_x.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    controls.boost = playing && boost;
    // Latch presses until a fixed step consumes them.
    controls.restart |= restart;
}

fn quit(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
