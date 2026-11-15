//! Device input: keyboard and gamepad state becomes [`Controls`] for the
//! fixed-step simulation, or edits the [`NameDraft`] while creating a player.
//! This is the only module that reads devices.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonState, InputSystems};
use bevy::prelude::*;

use crate::bindings::{Action, BindingIntent, Bindings};
use crate::garage::{Garage, Intent, Tab};
use crate::garage_ui::MenuActions;
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
                capture_binding,
                menu_controls.run_if(in_state(ProfilePhase::Playing)),
                sample_controls.run_if(not(creating.clone())),
                edit_name.run_if(creating),
                quit.run_if(|| CAN_QUIT),
            )
                .chain()
                .after(InputSystems),
        );
    }
}

/// Keys are physical positions, so WASD stays in place on other layouts.
fn sample_controls(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    run: Res<Run>,
    garage: Res<Garage>,
    storage: Res<crate::profile::Storage>,
    bindings: Res<Bindings>,
    mut controls: ResMut<Controls>,
) {
    if garage.open {
        return;
    }
    let held = |action| bindings.map.pressed(action, &keys, false);
    let mut move_x =
        f32::from(u8::from(held(Action::Right))) - f32::from(u8::from(held(Action::Left)));
    let mut boost = held(Action::Boost);
    let mut restart = bindings.map.pressed(Action::Restart, &keys, true);
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
    controls.brake = playing && held(Action::Brake);
    // Latch presses until a fixed step consumes them.
    if storage.0.online() {
        controls.end_drive |= restart;
    } else {
        controls.restart |= restart;
    }
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

/// Whether Escape outside the garage quits. In the browser, quitting would only
/// freeze the page, so the player closes the tab instead.
pub const CAN_QUIT: bool = cfg!(not(target_arch = "wasm32"));

fn quit(keys: Res<ButtonInput<KeyCode>>, garage: Res<Garage>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) && !garage.open {
        exit.write(AppExit::Success);
    }
}

fn menu_controls(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut garage: ResMut<Garage>,
    menu: Res<MenuActions>,
    mut controls: ResMut<Controls>,
    bindings: Res<Bindings>,
) {
    if bindings.capture.is_some() || bindings.waiting_release || bindings.loading {
        return;
    }
    let pad = |button| pads.iter().any(|p| p.just_pressed(button));
    if !garage.open {
        if keys.just_pressed(KeyCode::KeyG) || pad(GamepadButton::Start) {
            garage.enqueue(Intent::Open);
        }
        return;
    }
    *controls = Controls::default();
    let count = menu.0.len();
    garage.focus = garage.focus.min(count.saturating_sub(1));
    if count > 0 {
        if keys.just_pressed(KeyCode::ArrowDown) || pad(GamepadButton::DPadDown) {
            garage.focus = (garage.focus + 1) % count;
        }
        if keys.just_pressed(KeyCode::ArrowUp) || pad(GamepadButton::DPadUp) {
            garage.focus = (garage.focus + count - 1) % count;
        }
        if (keys.just_pressed(KeyCode::Enter) || pad(GamepadButton::South))
            && let Some(intent) = menu.0.get(garage.focus)
        {
            garage.enqueue(intent.clone());
        }
    }
    if bindings.open {
        if keys.just_pressed(KeyCode::Escape) || pad(GamepadButton::East) {
            garage.enqueue(Intent::Bindings(BindingIntent::Close));
        }
        return;
    }
    if keys.just_pressed(KeyCode::Tab) || pad(GamepadButton::RightTrigger) {
        let tab = match garage.tab {
            Tab::Owned => Tab::Catalog,
            Tab::Catalog => Tab::Market,
            Tab::Market => Tab::Owned,
        };
        garage.enqueue(Intent::Tab(tab));
    }
    if keys.just_pressed(KeyCode::Escape) || pad(GamepadButton::East) {
        garage.enqueue(Intent::Cancel);
    }
    if keys.just_pressed(KeyCode::KeyG) || pad(GamepadButton::Start) {
        garage.enqueue(Intent::Start);
    }
}

/// Runs before all other keyboard consumers. Capture never leaks to menu actions.
fn capture_binding(keys: Res<ButtonInput<KeyCode>>, mut bindings: ResMut<Bindings>) {
    if bindings.waiting_release {
        if keys.get_pressed().next().is_none() {
            bindings.waiting_release = false;
        }
        return;
    }
    if bindings.capture.is_none() {
        return;
    }
    if !bindings.capture_ready {
        if keys.get_pressed().next().is_none() {
            bindings.capture_ready = true;
        }
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        bindings.apply(BindingIntent::Cancel);
    } else if let Some(key) = keys.get_just_pressed().next() {
        bindings.capture_key(*key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Controls>()
            .init_resource::<Run>()
            .init_resource::<Bindings>()
            .init_resource::<MenuActions>()
            .init_resource::<Garage>()
            .insert_resource(crate::profile::Storage(std::sync::Arc::new(
                crate::storage::memory::MemoryStore::default(),
            )))
            .add_systems(
                Update,
                (capture_binding, menu_controls, sample_controls).chain(),
            );
        app.world_mut().resource_mut::<Garage>().open = false;
        app
    }
    #[test]
    fn every_default_boost_and_brake_key_samples_and_releases() {
        let mut app = app();
        for (key, boost, brake) in [
            (KeyCode::Space, true, false),
            (KeyCode::ArrowUp, true, false),
            (KeyCode::KeyW, true, false),
            (KeyCode::ArrowDown, false, true),
            (KeyCode::KeyS, false, true),
        ] {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
            app.update();
            let controls = app.world().resource::<Controls>();
            assert_eq!((controls.boost, controls.brake), (boost, brake));
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            app.update();
            let controls = app.world().resource::<Controls>();
            assert!(!controls.boost && !controls.brake);
        }
    }
    #[test]
    fn capture_waits_for_release_and_does_not_activate_menu_or_drive() {
        let mut app = app();
        app.world_mut().resource_mut::<Garage>().open = true;
        app.world_mut()
            .resource_mut::<MenuActions>()
            .0
            .push(Intent::Start);
        app.world_mut()
            .resource_mut::<Bindings>()
            .apply(BindingIntent::Capture(Some(0)));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert!(app.world().resource::<Bindings>().capture.is_some());
        assert!(app.world().resource::<Garage>().intents.is_empty());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert!(app.world().resource::<Bindings>().capture.is_none());
        assert_eq!(
            app.world().resource::<Bindings>().map.keys(Action::Boost)[0],
            "Enter"
        );
        assert!(app.world().resource::<Garage>().intents.is_empty());
        assert!(!app.world().resource::<Controls>().boost);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.update();
        app.world_mut().resource_mut::<Garage>().open = false;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert!(app.world().resource::<Controls>().boost);
    }
    #[test]
    fn capture_cancel_preserves_keys_and_does_not_start_or_close_the_garage() {
        let mut app = app();
        app.world_mut().resource_mut::<Garage>().open = true;
        app.world_mut()
            .resource_mut::<Bindings>()
            .apply(BindingIntent::Capture(Some(0)));
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(app.world().resource::<Bindings>().capture.is_none());
        assert_eq!(
            app.world().resource::<Bindings>().map.keys(Action::Boost)[0],
            "Space"
        );
        assert!(app.world().resource::<Garage>().intents.is_empty());
    }
}
