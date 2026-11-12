//! Device input: keyboard and gamepad state becomes [`Controls`] for the
//! fixed-step simulation, or edits the [`NameDraft`] while creating a player.
//! This is the only module that reads devices.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonState, InputSystems};
use bevy::prelude::*;

use crate::player::Controls;
use crate::profile::{NameDraft, ProfilePhase};
use crate::run::Run;
use crate::storage::NAME_MAX_CHARS;

/// Stick deflection below this is ignored.
const STICK_DEAD_ZONE: f32 = 0.25;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        // Sample before the fixed-step loop so this frame's presses reach this
        // frame's fixed steps.
        let creating = in_state(ProfilePhase::Creating);
        app.add_systems(
            PreUpdate,
            (
                sample_controls.run_if(not(creating.clone())),
                edit_name.run_if(creating),
                quit,
            )
                .after(InputSystems),
        );
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

/// Typed text uses the keyboard layout; Enter or a controller button confirms,
/// so controller players can accept the suggested name.
fn edit_name(
    mut typed: MessageReader<KeyboardInput>,
    gamepads: Query<&Gamepad>,
    mut draft: ResMut<NameDraft>,
) {
    for key in typed.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
        match &key.logical_key {
            Key::Enter => draft.submitted = true,
            Key::Backspace => {
                draft.text.pop();
                draft.error = None;
            }
            _ => {
                for c in key.text.iter().flat_map(|text| text.chars()) {
                    if !c.is_control() && draft.text.chars().count() < NAME_MAX_CHARS {
                        draft.text.push(c);
                        draft.error = None;
                    }
                }
            }
        }
    }
    let confirm = [GamepadButton::South, GamepadButton::Start];
    if gamepads.iter().any(|pad| pad.any_just_pressed(confirm)) {
        draft.submitted = true;
    }
}

fn quit(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
