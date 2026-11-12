//! Procedural car and pickup sprites, scrolling lane marks, and render interpolation.
use crate::level::{ROAD_HALF_WIDTH, Traffic, VehicleKind};
use crate::pickups::{PICKUP_SIZE, Pickup};
use crate::player::{Player, Position, PreviousPosition};
use crate::run::Run;
use crate::vehicles::ActiveVehicle;
use bevy::prelude::*;

pub mod palette {
    use bevy::prelude::Color;
    pub const PANEL: Color = Color::srgba(0.035, 0.055, 0.08, 0.95);
    pub const TEXT: Color = Color::srgb(0.93, 0.95, 0.96);
    pub const BLUE: Color = Color::srgb(0.22, 0.72, 0.95);
    pub const HEART: Color = Color::srgb(1.0, 0.4, 0.38);
    pub const SHIELD: Color = Color::srgb(0.55, 0.95, 1.0);
    pub const PROTECTION: Color = Color::srgb(1.0, 0.82, 0.3);
    pub const DIM: Color = Color::srgb(0.25, 0.29, 0.33);
}

#[derive(Component)]
struct RoadMark(f32);
#[derive(Component)]
enum Effect {
    Protection,
    Exhaust,
    Shield,
}

#[derive(Component)]
struct TurnSignal {
    vehicle: Entity,
    /// Side in road coordinates; the parent reverses local x for oncoming cars.
    side: f32,
}

pub struct VisualsPlugin;
impl Plugin for VisualsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.09, 0.16, 0.14)))
            .add_systems(Startup, draw_road)
            .add_systems(Update, (decorate_cars, decorate_pickups, decorate_player))
            .add_systems(
                PostUpdate,
                (
                    interpolate_positions,
                    scroll_road,
                    show_effects,
                    show_turn_signals,
                )
                    .before(TransformSystems::Propagate),
            );
    }
}

fn sprite(color: Color, size: Vec2, center: Vec2, z: f32) -> impl Bundle {
    (
        Sprite::from_color(color, size),
        Transform::from_translation(center.extend(z)),
    )
}

/// A square turned by 45 degrees, used to build hearts and shields.
fn diamond(color: Color, side: f32, center: Vec2, z: f32) -> impl Bundle {
    (
        Sprite::from_color(color, Vec2::splat(side)),
        Transform::from_translation(center.extend(z))
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
    )
}

fn draw_road(mut commands: Commands) {
    commands.spawn(sprite(
        Color::srgb(0.16, 0.19, 0.23),
        Vec2::new(ROAD_HALF_WIDTH * 2.0, 1000.0),
        Vec2::ZERO,
        0.0,
    ));
    for x in [-ROAD_HALF_WIDTH, ROAD_HALF_WIDTH] {
        commands.spawn(sprite(
            Color::srgb(0.85, 0.87, 0.80),
            Vec2::new(5.0, 1000.0),
            Vec2::new(x, 0.0),
            0.1,
        ));
    }
    for x in [-70.0, 70.0] {
        for i in -6..=6 {
            let y = i as f32 * 90.0;
            commands.spawn((
                RoadMark(y),
                sprite(
                    Color::srgb(0.55, 0.60, 0.61),
                    Vec2::new(4.0, 38.0),
                    Vec2::new(x, y),
                    0.1,
                ),
            ));
        }
    }
}

type NewCars<'w, 's> = Query<'w, 's, (Entity, &'static Position, &'static Traffic), Added<Traffic>>;

fn decorate_cars(mut commands: Commands, cars: NewCars) {
    for (entity, position, traffic) in &cars {
        let oncoming = traffic.speed < 0.0;
        let truck = traffic.kind == VehicleKind::Truck;
        let size = traffic.kind.size();
        let color = if oncoming {
            Color::srgb(0.94, 0.38, 0.30)
        } else {
            Color::srgb(0.82, 0.85, 0.78)
        };
        commands
            .entity(entity)
            .insert((
                Transform::from_translation(position.0.extend(2.0)).with_rotation(
                    Quat::from_rotation_z(if oncoming { std::f32::consts::PI } else { 0.0 }),
                ),
                Visibility::default(),
            ))
            .with_children(|body| {
                body.spawn(sprite(
                    Color::srgba(0.0, 0.0, 0.0, 0.3),
                    size + Vec2::splat(6.0),
                    Vec2::new(4.0, -4.0),
                    -0.2,
                ));
                for side in [-1.0, 1.0] {
                    for end in [-1.0, 1.0] {
                        body.spawn(sprite(
                            Color::srgb(0.035, 0.045, 0.06),
                            Vec2::new(8.0, 20.0),
                            Vec2::new(side * (size.x * 0.5 + 2.0), end * size.y * 0.31),
                            0.0,
                        ));
                    }
                }
                body.spawn(sprite(color, size, Vec2::ZERO, 0.1));
                body.spawn(sprite(
                    Color::srgb(0.06, 0.14, 0.21),
                    Vec2::new(size.x - 10.0, 17.0),
                    Vec2::new(0.0, if truck { 49.0 } else { 15.0 }),
                    0.2,
                ));
                if truck {
                    body.spawn(sprite(
                        Color::srgb(0.46, 0.55, 0.61),
                        Vec2::new(size.x - 10.0, 94.0),
                        Vec2::new(0.0, -17.0),
                        0.2,
                    ));
                    for y in [-48.0, -18.0, 12.0] {
                        body.spawn(sprite(
                            Color::srgb(0.63, 0.70, 0.73),
                            Vec2::new(size.x - 18.0, 3.0),
                            Vec2::new(0.0, y),
                            0.3,
                        ));
                    }
                } else {
                    body.spawn(sprite(
                        Color::srgb(0.1, 0.22, 0.29),
                        Vec2::new(30.0, 11.0),
                        Vec2::new(0.0, -24.0),
                        0.2,
                    ));
                }
                for side in [-1.0, 1.0] {
                    let x = side * (size.x * 0.5 - 7.0);
                    body.spawn(sprite(
                        Color::srgb(1.0, 0.94, 0.65),
                        Vec2::new(9.0, 6.0),
                        Vec2::new(x, size.y * 0.5 - 5.0),
                        0.2,
                    ));
                    body.spawn(sprite(
                        Color::srgb(0.8, 0.12, 0.14),
                        Vec2::new(8.0, 4.0),
                        Vec2::new(x, -size.y * 0.5 + 3.0),
                        0.2,
                    ));

                    for end in [-1.0, 1.0] {
                        body.spawn((
                            TurnSignal {
                                vehicle: entity,
                                side: if oncoming { -side } else { side },
                            },
                            sprite(
                                Color::srgb(1.0, 0.62, 0.02),
                                Vec2::new(10.0, 10.0),
                                Vec2::new(side * (size.x * 0.5 - 2.0), end * (size.y * 0.5 - 11.0)),
                                0.4,
                            ),
                            Visibility::Hidden,
                        ));
                    }
                }
            });
    }
}

fn show_turn_signals(traffic: Query<&Traffic>, mut signals: Query<(&TurnSignal, &mut Visibility)>) {
    for (signal, mut visibility) in &mut signals {
        *visibility = if traffic
            .get(signal.vehicle)
            .is_ok_and(|car| car.signal_side() == Some(signal.side))
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn decorate_pickups(
    mut commands: Commands,
    pickups: Query<(Entity, &Pickup, &Position), Added<Pickup>>,
) {
    for (entity, pickup, position) in &pickups {
        let color = match pickup {
            Pickup::Heart => palette::HEART,
            Pickup::Shield => palette::SHIELD,
        };
        commands
            .entity(entity)
            .insert((
                Transform::from_translation(position.0.extend(1.0)),
                Visibility::default(),
            ))
            .with_children(|body| {
                body.spawn(sprite(color, PICKUP_SIZE, Vec2::ZERO, 0.0));
                body.spawn(sprite(
                    Color::srgb(0.07, 0.09, 0.12),
                    PICKUP_SIZE - Vec2::splat(6.0),
                    Vec2::ZERO,
                    0.1,
                ));
                match pickup {
                    Pickup::Heart => {
                        body.spawn(diamond(color, 10.0, Vec2::new(-4.5, 3.5), 0.2));
                        body.spawn(diamond(color, 10.0, Vec2::new(4.5, 3.5), 0.2));
                        body.spawn(diamond(color, 13.0, Vec2::new(0.0, -2.0), 0.2));
                    }
                    Pickup::Shield => {
                        body.spawn(sprite(
                            color,
                            Vec2::new(18.0, 10.0),
                            Vec2::new(0.0, 5.0),
                            0.2,
                        ));
                        body.spawn(diamond(color, 12.7, Vec2::new(0.0, -0.5), 0.2));
                    }
                }
            });
    }
}

fn interpolate_positions(
    fixed: Res<Time<Fixed>>,
    run: Res<Run>,
    mut bodies: Query<(&Position, &PreviousPosition, &mut Transform)>,
) {
    let blend = if run.finished {
        1.0
    } else {
        fixed.overstep_fraction()
    };
    for (position, previous, mut transform) in &mut bodies {
        transform.translation = previous
            .0
            .lerp(position.0, blend)
            .extend(transform.translation.z);
    }
}

fn scroll_road(
    fixed: Res<Time<Fixed>>,
    run: Res<Run>,
    mut marks: Query<(&RoadMark, &mut Transform)>,
) {
    let blend = if run.finished {
        1.0
    } else {
        f64::from(fixed.overstep_fraction())
    };
    let distance = run.previous_distance + (run.distance - run.previous_distance) * blend;
    for (mark, mut transform) in &mut marks {
        transform.translation.y = mark.0 - (distance % 90.0) as f32;
    }
}

fn show_effects(run: Res<Run>, mut effects: Query<(&Effect, &mut Visibility, &mut Sprite)>) {
    for (effect, mut visibility, mut sprite) in &mut effects {
        let visible = match effect {
            Effect::Protection => {
                // A shield-absorbed hit flashes white-blue instead of gold.
                sprite.color = if run.shield_absorbed {
                    Color::WHITE.mix(&palette::SHIELD, 0.4)
                } else {
                    palette::PROTECTION
                };
                run.protection > 0.0 && !run.finished
            }
            Effect::Exhaust => run.boosting,
            Effect::Shield => run.shield && !run.finished,
        };
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn decorate_player(
    mut commands: Commands,
    active: Res<ActiveVehicle>,
    players: Query<(Entity, &Position), With<Player>>,
    mut drawn: Local<bool>,
) {
    if *drawn && !active.is_changed() {
        return;
    }
    for (entity, at) in &players {
        let size = active.0.model.size();
        commands
            .entity(entity)
            .despawn_children()
            .insert((
                Transform::from_translation(at.0.extend(2.)),
                Visibility::default(),
            ))
            .with_children(|body| {
                body.spawn(sprite(
                    Color::srgba(0., 0., 0., 0.3),
                    size + Vec2::splat(5.),
                    Vec2::new(4., -4.),
                    -0.2,
                ));
                for (effect, color, dimensions, offset) in [
                    (
                        Effect::Shield,
                        palette::SHIELD.with_alpha(0.35),
                        size + Vec2::splat(30.),
                        Vec2::ZERO,
                    ),
                    (
                        Effect::Protection,
                        palette::PROTECTION,
                        size + Vec2::splat(12.),
                        Vec2::ZERO,
                    ),
                    (
                        Effect::Exhaust,
                        palette::BLUE,
                        Vec2::new(18., 35.),
                        Vec2::new(0., -size.y * 0.5 - 12.),
                    ),
                ] {
                    body.spawn((
                        effect,
                        sprite(color, dimensions, offset, -0.1),
                        Visibility::Hidden,
                    ));
                }
                for (i, part) in crate::vehicle_art::parts(&active.0).into_iter().enumerate() {
                    body.spawn(sprite(part.color, part.size, part.at, 0.01 * i as f32));
                }
            });
        *drawn = true;
    }
}
