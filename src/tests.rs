//! Deterministic checks through the actual fixed-step simulation.
use crate::level::{
    CAR_SIZE, LANES, LaneChange, PLAYER_START, ROAD_HALF_WIDTH, SIGNAL_SECONDS, TRAFFIC_SPACING,
    TRUCK_SIZE, Traffic, TrafficFlow, VehicleKind, blocks_all_lanes,
};
use crate::pickups::{PICKUP_CLEARANCE, PICKUP_PASSED_Y, Pickup, PickupFlow, keeps_clear};
use crate::player::{Controls, Player, PlayerTuning, Position, PreviousPosition};
use crate::run::{Run, swept_hit};
use crate::simulation::SimulationPlugin;
use bevy::{prelude::*, time::TimeUpdateStrategy};

struct Sim(App);
impl Sim {
    fn new() -> Self {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimulationPlugin))
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
            .insert_resource(TrafficFlow {
                cooldown: 0.0,
                rng: fastrand::Rng::with_seed(7),
            })
            .insert_resource(PickupFlow::with_seed(3));
        app.finish();
        app.cleanup();
        app.update();
        Self(app)
    }
    fn steps(&mut self, n: usize) {
        for _ in 0..n {
            self.0.update();
        }
    }
    fn controls(&mut self) -> Mut<'_, Controls> {
        self.0.world_mut().resource_mut::<Controls>()
    }
    fn run(&self) -> Run {
        self.0.world().resource::<Run>().clone()
    }
    fn position(&mut self) -> Vec2 {
        let world = self.0.world_mut();
        world
            .query_filtered::<&Position, With<Player>>()
            .single(world)
            .unwrap()
            .0
    }
    fn car(&mut self, at: Vec2, speed: f32) -> Entity {
        self.0
            .world_mut()
            .spawn((
                Traffic {
                    change_cooldown: f32::INFINITY,
                    ..Traffic::new(
                        LANES
                            .iter()
                            .position(|x| (*x - at.x).abs() <= 70.0)
                            .unwrap_or(1),
                        speed,
                        VehicleKind::Car,
                    )
                },
                Position(at),
                PreviousPosition(at),
            ))
            .id()
    }
    fn pickup(&mut self, at: Vec2, kind: Pickup) -> Entity {
        self.0
            .world_mut()
            .spawn((kind, Position(at), PreviousPosition(at)))
            .id()
    }
    fn exists(&self, entity: Entity) -> bool {
        self.0.world().get_entity(entity).is_ok()
    }
    fn run_mut(&mut self) -> Mut<'_, Run> {
        self.0.world_mut().resource_mut::<Run>()
    }
    fn quiet(&mut self) {
        self.0.world_mut().resource_mut::<TrafficFlow>().cooldown = 10000.0;
        self.0.world_mut().resource_mut::<PickupFlow>().remaining = f32::INFINITY;
    }
}

#[test]
fn steering_stops_on_release_and_stays_inside_both_edges() {
    let mut sim = Sim::new();
    sim.quiet();
    sim.controls().move_x = -1.0;
    sim.steps(128);
    assert_eq!(sim.position().x, -ROAD_HALF_WIDTH + CAR_SIZE.x * 0.5);
    sim.controls().move_x = 1.0;
    sim.steps(128);
    assert_eq!(sim.position().x, ROAD_HALF_WIDTH - CAR_SIZE.x * 0.5);
    sim.controls().move_x = -0.5;
    sim.steps(10);
    sim.controls().move_x = 0.0;
    let stopped = sim.position();
    sim.steps(10);
    assert_eq!(sim.position(), stopped);
    assert_eq!(stopped.y, PLAYER_START.y);
    assert!(sim.run().distance > 0.0);
}

#[test]
fn boost_increases_distance_depletes_and_recharges_without_pulsing() {
    let mut cruise = Sim::new();
    cruise.quiet();
    cruise.steps(64);
    let mut sim = Sim::new();
    sim.quiet();
    sim.controls().boost = true;
    sim.steps(64);
    assert!(sim.run().distance > cruise.run().distance * 1.5);
    assert!(sim.run().energy < 0.7);
    sim.steps(110);
    assert!(sim.run().exhausted);
    assert!(!sim.run().boosting);
    let distance = sim.run().distance;
    sim.steps(64);
    assert!((sim.run().distance - distance - 300.0).abs() < 0.01);
    sim.controls().boost = false;
    sim.steps(400);
    assert_eq!(sim.run().energy, 1.0);
    assert!(!sim.run().exhausted);
    sim.controls().boost = true;
    sim.steps(1);
    assert!(sim.run().boosting);
}

#[test]
fn every_1000_metres_increases_spawn_rate_and_speed_up_to_the_limit() {
    let mut baseline = Sim::new();
    baseline.steps(1);
    let base_interval = baseline.0.world().resource::<TrafficFlow>().cooldown;
    for (metres, step) in [
        (0.0, 0),
        (999.0, 0),
        (1000.0, 1),
        (1999.0, 1),
        (2000.0, 2),
        (3000.0, 3),
        (3999.0, 3),
        (4000.0, 4),
        (5000.0, 5),
        (6000.0, 6),
        (7000.0, 7),
        (8000.0, 8),
        (9000.0, 9),
        (150_000.0, 9),
    ] {
        for boost in [false, true] {
            let mut sim = Sim::new();
            sim.0.world_mut().resource_mut::<Run>().distance = metres * 10.0;
            sim.controls().boost = boost;
            sim.steps(1);
            let base_speed = if boost { 540.0 } else { 300.0 };
            let expected = base_speed * (1.0 + 0.05 * step as f32);
            assert!((sim.run().speed - expected).abs() < 0.001);
            let world = sim.0.world_mut();
            assert_eq!(world.query::<&Traffic>().iter(world).count(), 1);
            let expected_interval = base_interval / (1.0 + 0.15 * step as f32);
            assert!((world.resource::<TrafficFlow>().cooldown - expected_interval).abs() < 0.001);
        }
    }
}

#[test]
fn crossing_a_distance_threshold_accelerates_on_the_next_tick() {
    let mut sim = Sim::new();
    sim.quiet();
    sim.0.world_mut().resource_mut::<Run>().distance = 9_999.0;
    sim.steps(1);
    assert_eq!(sim.run().speed, 300.0);
    assert!(sim.run().distance >= 10_000.0);
    sim.steps(1);
    assert!((sim.run().speed - 315.0).abs() < 0.001);
}

#[test]
fn trucks_unlock_at_4000_metres_and_offsets_stay_within_the_lane() {
    for metres in [0.0, 3999.0, 4000.0, 7000.0] {
        let mut trucks = 0;
        let mut offset_sides = [false; 2];
        for seed in 0..100 {
            let mut sim = Sim::new();
            sim.run_mut().distance = metres * 10.0;
            sim.0.world_mut().resource_mut::<TrafficFlow>().rng = fastrand::Rng::with_seed(seed);
            sim.steps(1);
            let world = sim.0.world_mut();
            let (car, p) = world
                .query::<(&Traffic, &Position)>()
                .single(world)
                .unwrap();
            trucks += usize::from(car.kind == VehicleKind::Truck);
            let offset = p.0.x - LANES[car.lane];
            offset_sides[usize::from(offset > 0.0)] = true;
            assert!(offset.abs() <= car.kind.max_offset());
            assert!(offset.abs() + car.kind.size().x * 0.5 < 70.0);
        }
        assert!(offset_sides.into_iter().all(|seen| seen));
        if metres < 4000.0 {
            assert_eq!(trucks, 0);
        } else {
            assert!(
                (1..100).contains(&trucks),
                "trucks mix with cars at {metres} m"
            );
        }
    }
}

#[test]
fn trucks_collide_across_their_full_width_and_length() {
    assert!(TRUCK_SIZE.cmpgt(CAR_SIZE).all());
    for offset in [Vec2::new(52.0, 0.0), Vec2::new(0.0, 100.0)] {
        for kind in [VehicleKind::Car, VehicleKind::Truck] {
            let mut sim = Sim::new();
            sim.quiet();
            let id = sim.car(PLAYER_START + offset, 300.0);
            sim.0.world_mut().get_mut::<Traffic>(id).unwrap().kind = kind;
            sim.steps(1);
            assert_eq!(
                sim.run().lives,
                if kind == VehicleKind::Truck { 2 } else { 3 }
            );
        }
    }
}

#[test]
fn lane_changes_signal_before_moving_and_finish_at_the_target_offset() {
    for (lane, target, speed) in [(0, 1, 140.0), (2, 1, -150.0)] {
        for kind in [VehicleKind::Car, VehicleKind::Truck] {
            let mut sim = Sim::new();
            sim.quiet();
            let start = LANES[lane] + 15.0;
            let target_x = LANES[target] - 20.0;
            let id = sim.car(Vec2::new(start, 480.0), speed);
            {
                let mut car = sim.0.world_mut().get_mut::<Traffic>(id).unwrap();
                car.kind = kind;
                car.change = Some(LaneChange {
                    target_lane: target,
                    start_x: start,
                    target_x,
                    elapsed: 0.0,
                });
            }
            let side = (target_x - start).signum();
            let mut saw_on = false;
            let mut saw_off = false;
            for _ in 0..(SIGNAL_SECONDS * 64.0) as usize {
                sim.steps(1);
                let car = sim.0.world().get::<Traffic>(id).unwrap();
                assert_eq!(sim.0.world().get::<Position>(id).unwrap().0.x, start);
                assert!(car.occupies(lane) && car.occupies(target));
                match car.signal_side() {
                    Some(lit) => {
                        assert_eq!(lit, side);
                        saw_on = true;
                    }
                    None => saw_off = true,
                }
            }
            assert!(saw_on && saw_off, "the warning must blink");
            // Keep the vehicle on screen while checking the entire manoeuvre.
            sim.0.world_mut().get_mut::<Traffic>(id).unwrap().speed = 300.0;
            sim.steps(32);
            let halfway = sim.0.world().get::<Position>(id).unwrap().0.x;
            assert!((halfway - start) * side > 0.0);
            assert!((target_x - halfway) * side > 0.0);
            sim.steps(64);
            let car = sim.0.world().get::<Traffic>(id).unwrap();
            assert!(car.change.is_none());
            assert_eq!(car.lane, target);
            assert_eq!(car.signal_side(), None);
            assert_eq!(sim.0.world().get::<Position>(id).unwrap().0.x, target_x);
        }
    }
}

#[test]
fn lane_change_planner_respects_traffic_pickups_and_adjacent_lanes() {
    for obstacle in [None, Some(0), Some(1), Some(2)] {
        let mut sim = Sim::new();
        sim.quiet();
        let id = sim.car(Vec2::new(LANES[0] + 20.0, 320.0), 140.0);
        sim.0
            .world_mut()
            .get_mut::<Traffic>(id)
            .unwrap()
            .change_cooldown = 0.0;
        match obstacle {
            Some(0) => {
                sim.car(Vec2::new(LANES[1] - 30.0, 300.0), 140.0);
            }
            Some(1) => {
                sim.car(Vec2::new(LANES[1], -100.0), -150.0);
            }
            Some(2) => {
                sim.pickup(Vec2::new(LANES[1], 320.0), Pickup::Shield);
            }
            _ => {}
        }
        sim.steps(1);
        let car = sim.0.world().get::<Traffic>(id).unwrap();
        if obstacle.is_some() {
            assert!(
                car.change.is_none(),
                "unsafe reservation for obstacle {obstacle:?}"
            );
        } else {
            assert_eq!(car.change.unwrap().target_lane, 1);
            assert_eq!(car.signal_side(), Some(1.0));
        }
    }
}

#[test]
fn changing_lanes_hits_a_player_waiting_on_the_lane_marking() {
    let mut sim = Sim::new();
    sim.quiet();
    let world = sim.0.world_mut();
    world
        .query_filtered::<&mut Position, With<Player>>()
        .single_mut(world)
        .unwrap()
        .0
        .x = 70.0;
    let id = sim.car(Vec2::new(0.0, PLAYER_START.y), 300.0);
    sim.0.world_mut().get_mut::<Traffic>(id).unwrap().change = Some(LaneChange {
        target_lane: 2,
        start_x: 0.0,
        target_x: 140.0,
        elapsed: 0.0,
    });
    sim.steps(51);
    assert_eq!(sim.run().lives, 3, "no movement during the warning");
    sim.steps(32);
    assert_eq!(sim.run().lives, 2, "crossing a marking must collide");
}

#[test]
fn stationary_lane_markings_are_no_longer_safe_in_real_traffic() {
    for x in [-70.0, 70.0] {
        for seed in [1, 7, 42] {
            let mut sim = Sim::new();
            sim.0.world_mut().resource_mut::<TrafficFlow>().rng = fastrand::Rng::with_seed(seed);
            sim.0.world_mut().resource_mut::<PickupFlow>().remaining = f32::INFINITY;
            let world = sim.0.world_mut();
            world
                .query_filtered::<&mut Position, With<Player>>()
                .single_mut(world)
                .unwrap()
                .0
                .x = x;
            sim.steps(64 * 120);
            assert!(
                sim.run().finished,
                "marking x={x} stayed safe for seed {seed}"
            );
        }
    }
}

#[test]
fn collision_removes_one_heart_and_protection_prevents_multiple_hits() {
    let mut sim = Sim::new();
    sim.quiet();
    sim.car(PLAYER_START, 300.0);
    sim.car(PLAYER_START, 300.0);
    sim.steps(1);
    assert_eq!(sim.run().lives, 2);
    sim.steps(60);
    assert_eq!(sim.run().lives, 2);
    sim.steps(40);
    assert_eq!(sim.run().lives, 1);
    // Both cars are now marked as hit, so lingering overlap cannot hit again.
    sim.steps(200);
    assert_eq!(sim.run().lives, 1);
}

#[test]
fn high_speed_oncoming_car_cannot_tunnel_through_player() {
    let mut sim = Sim::new();
    sim.quiet();
    sim.car(PLAYER_START + Vec2::Y * 180.0, -30000.0);
    sim.steps(1);
    assert_eq!(sim.run().lives, 2);
}

#[test]
fn swept_collision_accounts_for_both_axes_and_near_misses() {
    assert!(swept_hit(
        Vec2::new(-100.0, 150.0),
        Vec2::new(100.0, -150.0),
        CAR_SIZE
    ));
    assert!(!swept_hit(
        Vec2::new(100.0, 150.0),
        Vec2::new(100.0, -150.0),
        CAR_SIZE
    ));
    assert!(!swept_hit(
        Vec2::new(-100.0, 250.0),
        Vec2::new(100.0, 50.0),
        CAR_SIZE
    ));
}

#[test]
fn game_over_freezes_simulation_and_restart_restores_everything() {
    let mut sim = Sim::new();
    sim.quiet();
    sim.0.world_mut().resource_mut::<Run>().lives = 1;
    sim.0.world_mut().resource_mut::<Run>().distance = 90_000.0;
    let car = sim.car(PLAYER_START, 300.0);
    sim.steps(1);
    assert!(sim.run().finished);
    let distance = sim.run().distance;
    let player = sim.position();
    let traffic = sim.0.world().get::<Position>(car).unwrap().0;
    sim.controls().move_x = -1.0;
    sim.controls().boost = true;
    sim.steps(128);
    assert_eq!(sim.position(), player);
    assert_eq!(sim.run().distance, distance);
    assert_eq!(sim.0.world().get::<Position>(car).unwrap().0, traffic);
    sim.controls().restart = true;
    sim.steps(1);
    assert_eq!(sim.position(), PLAYER_START);
    assert_eq!(sim.run().distance, 0.0);
    assert_eq!(sim.run().previous_distance, 0.0);
    assert_eq!(sim.run().lives, 3);
    assert_eq!(sim.run().energy, 1.0);
    assert_eq!(sim.run().protection, 0.0);
    assert!(!sim.run().finished);
    assert!(!sim.controls().restart);
    let world = sim.0.world_mut();
    assert_eq!(world.query::<&Traffic>().iter(world).count(), 0);
    assert_eq!(world.resource::<TrafficFlow>().cooldown, 0.0);
    sim.steps(1);
    assert!(sim.run().distance > 0.0);
    assert_eq!(sim.run().speed, 300.0);
    let world = sim.0.world_mut();
    assert_eq!(world.query::<&Traffic>().iter(world).count(), 1);
}

#[test]
fn traffic_directions_have_correct_relative_speed_and_cleanup() {
    let mut sim = Sim::new();
    sim.quiet();
    let same = sim.car(Vec2::new(-140.0, 300.0), 140.0);
    let opposite = sim.car(Vec2::new(0.0, 300.0), -150.0);
    sim.steps(64);
    assert!((sim.0.world().get::<Position>(same).unwrap().0.y - 140.0).abs() < 0.01);
    assert!((sim.0.world().get::<Position>(opposite).unwrap().0.y + 150.0).abs() < 0.01);
    sim.steps(300);
    assert!(sim.0.world().get_entity(same).is_err());
    assert!(sim.0.world().get_entity(opposite).is_err());
}

#[test]
fn spawn_safety_detects_oncoming_cars_closing_a_future_gap() {
    let cars = [
        (
            Traffic::new(0, -150.0, VehicleKind::Car),
            Vec2::new(LANES[0], 480.0),
        ),
        (
            Traffic::new(1, 140.0, VehicleKind::Car),
            Vec2::new(LANES[1], 0.0),
        ),
        (
            Traffic::new(2, 140.0, VehicleKind::Car),
            Vec2::new(LANES[2], -20.0),
        ),
    ];
    // Clear now, but the oncoming car would catch the other two before they exit.
    assert!(blocks_all_lanes(cars, 300.0));
    let mut cleared_first = cars;
    cleared_first[2].1.y = -450.0;
    assert!(!blocks_all_lanes(cleared_first, 300.0));
}

#[test]
fn random_traffic_is_continuous_spaced_and_never_forms_a_three_lane_wall() {
    for difficulty in 0..=9 {
        for seed in [1, 7, 42] {
            let mut sim = Sim::new();
            sim.0.world_mut().resource_mut::<TrafficFlow>().rng = fastrand::Rng::with_seed(seed);
            sim.0.world_mut().resource_mut::<Run>().distance = difficulty as f64 * 10_000.0;
            // Observe traffic independently of a stationary player's damage.
            sim.0.world_mut().resource_mut::<Run>().protection = 1000.0;
            let mut previous = Vec::new();
            let mut overlapping_spawns = 0;
            let mut used_lanes = [false; 3];
            let mut directions = [false; 2];
            for tick in 0..3840 {
                // Include releases and recharges, not just constant cruising.
                sim.controls().boost = (tick / 160) % 2 == 0;
                sim.steps(1);
                let world = sim.0.world_mut();
                let cars: Vec<_> = world
                    .query::<(Entity, &Position, &Traffic)>()
                    .iter(world)
                    .map(|(id, p, car)| (id, p.0, *car))
                    .collect();
                let added: Vec<_> = cars
                    .iter()
                    .filter(|(id, _, _)| !previous.contains(id))
                    .collect();
                assert!(added.len() <= 1, "traffic must spawn individually");
                if !added.is_empty() && cars.iter().any(|(id, _, _)| previous.contains(id)) {
                    overlapping_spawns += 1;
                }
                for (_, _, car) in &added {
                    used_lanes[car.lane] = true;
                    directions[usize::from(car.speed > 0.0)] = true;
                }
                for (i, (_, a, car_a)) in cars.iter().enumerate() {
                    for (j, (_, b, car_b)) in cars.iter().enumerate().skip(i + 1) {
                        if (0..3).any(|lane| car_a.occupies(lane) && car_b.occupies(lane)) {
                            assert_eq!(car_a.speed, car_b.speed);
                            assert!((a.y - b.y).abs() >= TRAFFIC_SPACING - 0.01);
                        }
                        let half_size = (car_a.kind.size() + car_b.kind.size()) * 0.5;
                        assert!(!(*a - *b).abs().cmple(half_size).all(), "traffic overlap");
                        for (_, c, car_c) in cars.iter().skip(j + 1) {
                            assert!(
                                !blocks_all_lanes(
                                    [(*car_a, *a), (*car_b, *b), (*car_c, *c)],
                                    300.0
                                ),
                                "wall at difficulty {difficulty}, seed {seed}, tick {tick}"
                            );
                        }
                    }
                }
                previous = cars.iter().map(|(id, _, _)| *id).collect();
            }
            assert!(overlapping_spawns > 10);
            assert!(used_lanes.into_iter().all(|used| used));
            assert!(directions.into_iter().all(|used| used));
        }
    }
}

#[test]
fn random_layouts_are_reproducible_with_a_seed_and_vary_between_seeds() {
    let trace = |seed| {
        let mut sim = Sim::new();
        sim.0.world_mut().resource_mut::<TrafficFlow>().rng = fastrand::Rng::with_seed(seed);
        sim.0.world_mut().resource_mut::<Run>().protection = 1000.0;
        let mut seen = Vec::new();
        let mut spawns = Vec::new();
        for tick in 0..1280 {
            sim.steps(1);
            let world = sim.0.world_mut();
            for (id, at, car) in world.query::<(Entity, &Position, &Traffic)>().iter(world) {
                if !seen.contains(&id) {
                    seen.push(id);
                    spawns.push((tick, at.0.x, car.speed));
                }
            }
        }
        spawns
    };
    let first = trace(1);
    assert_eq!(first, trace(1));
    assert_ne!(first, trace(2));
    let intervals: std::collections::HashSet<_> =
        first.windows(2).map(|pair| pair[1].0 - pair[0].0).collect();
    assert!(intervals.len() > 3, "spawn intervals should vary");
}

#[test]
fn heart_restores_one_life_but_never_more_than_three() {
    let mut sim = Sim::new();
    sim.quiet();
    sim.run_mut().lives = 1;
    let heart = sim.pickup(PLAYER_START, Pickup::Heart);
    sim.steps(1);
    assert_eq!(sim.run().lives, 2);
    assert!(!sim.exists(heart));
    sim.run_mut().lives = 3;
    let unused = sim.pickup(PLAYER_START, Pickup::Heart);
    sim.steps(1);
    assert_eq!(sim.run().lives, 3);
    assert!(
        sim.exists(unused),
        "a heart without effect stays on the road"
    );
}

#[test]
fn shield_absorbs_exactly_one_hit_and_still_grants_protection() {
    let mut sim = Sim::new();
    sim.quiet();
    let shield = sim.pickup(PLAYER_START, Pickup::Shield);
    sim.car(PLAYER_START, 300.0);
    sim.car(PLAYER_START, 300.0);
    sim.steps(1);
    assert!(!sim.exists(shield));
    assert_eq!(sim.run().lives, 3);
    assert!(!sim.run().shield);
    assert!(sim.run().shield_absorbed);
    assert!(sim.run().protection > 0.0);
    sim.steps(100);
    assert_eq!(sim.run().lives, 2);
    assert!(!sim.run().shield_absorbed);
}

#[test]
fn shield_also_saves_the_last_heart() {
    let mut sim = Sim::new();
    sim.quiet();
    sim.run_mut().lives = 1;
    sim.run_mut().shield = true;
    sim.car(PLAYER_START, 300.0);
    sim.steps(1);
    assert_eq!(sim.run().lives, 1);
    assert!(!sim.run().finished);
}

#[test]
fn only_one_shield_is_held_and_protection_does_not_consume_it() {
    let mut sim = Sim::new();
    sim.quiet();
    sim.run_mut().shield = true;
    let second = sim.pickup(PLAYER_START, Pickup::Shield);
    sim.steps(1);
    assert!(sim.run().shield);
    assert!(sim.exists(second), "a second shield stays on the road");
    sim.run_mut().protection = 1.0;
    sim.car(PLAYER_START, 300.0);
    sim.steps(1);
    assert!(sim.run().shield);
    assert_eq!(sim.run().lives, 3);
}

#[test]
fn pickups_are_collected_even_at_extreme_speed() {
    let mut sim = Sim::new();
    sim.quiet();
    sim.0.world_mut().insert_resource(PlayerTuning {
        cruise_speed: 30_000.0,
        ..default()
    });
    sim.pickup(PLAYER_START + Vec2::Y * 180.0, Pickup::Shield);
    sim.steps(1);
    assert!(sim.run().shield);
}

#[test]
fn restart_removes_pickups_and_the_held_shield() {
    let mut sim = Sim::new();
    sim.quiet();
    sim.run_mut().shield = true;
    sim.pickup(Vec2::new(LANES[0], 300.0), Pickup::Heart);
    sim.controls().restart = true;
    sim.steps(1);
    assert!(!sim.run().shield);
    assert!(!sim.run().shield_absorbed);
    let world = sim.0.world_mut();
    assert_eq!(world.query::<&Pickup>().iter(world).count(), 0);
}

#[test]
fn pickup_clearance_predicts_cars_closing_in_before_the_player_arrives() {
    let cruise = 300.0;
    // Oncoming car behind the pickup would catch up with it.
    assert!(!keeps_clear(480.0, 880.0, -150.0, cruise));
    assert!(keeps_clear(480.0, 1180.0, -150.0, cruise));
    // The pickup catches a slower same-direction car ahead of it.
    assert!(!keeps_clear(480.0, 180.0, 140.0, cruise));
    assert!(keeps_clear(480.0, -200.0, 140.0, cruise));
    // A same-direction car behind it drives away from the pickup.
    assert!(keeps_clear(480.0, 480.0 + PICKUP_CLEARANCE, 140.0, cruise));
    assert!(!keeps_clear(480.0, 680.0, 140.0, cruise));
}

/// Drives five minutes beside the lanes, so nothing is collected, and records
/// every pickup spawn while checking clearance and one pickup at a time.
fn pickup_trace(lives: u8, shield: bool, seed: u64) -> Vec<(usize, f32, Pickup)> {
    let mut sim = Sim::new();
    sim.0.insert_resource(PickupFlow::with_seed(seed));
    sim.run_mut().lives = lives;
    sim.run_mut().shield = shield;
    sim.run_mut().protection = f32::INFINITY;
    let world = sim.0.world_mut();
    world
        .query_filtered::<&mut Position, With<Player>>()
        .single_mut(world)
        .unwrap()
        .0
        .x = 70.0;
    let mut seen = Vec::new();
    let mut spawns = Vec::new();
    for tick in 0..64 * 300 {
        sim.steps(1);
        let world = sim.0.world_mut();
        let pickups: Vec<_> = world
            .query::<(Entity, &Pickup, &Position)>()
            .iter(world)
            .map(|(id, kind, p)| (id, *kind, p.0))
            .collect();
        assert!(pickups.len() <= 1, "at most one pickup on the road");
        let cars: Vec<_> = world
            .query::<(&Traffic, &Position)>()
            .iter(world)
            .map(|(car, p)| (*car, p.0))
            .collect();
        for (id, kind, at) in pickups {
            if !seen.contains(&id) {
                seen.push(id);
                spawns.push((tick, at.x, kind));
            }
            if at.y > PICKUP_PASSED_Y {
                let lane = LANES.iter().position(|x| *x == at.x).unwrap();
                for (_, car) in cars.iter().filter(|(car, _)| car.occupies(lane)) {
                    assert!(
                        (car.y - at.y).abs() >= PICKUP_CLEARANCE - 0.01,
                        "car too close to pickup at tick {tick}, seed {seed}"
                    );
                }
            }
        }
    }
    spawns
}

#[test]
fn pickups_are_rare_fair_and_only_appear_when_useful() {
    for (lives, shield, kinds) in [
        (3, false, &[Pickup::Shield][..]),
        (2, true, &[Pickup::Heart][..]),
        (2, false, &[Pickup::Heart, Pickup::Shield][..]),
    ] {
        for seed in [1, 2] {
            let spawns = pickup_trace(lives, shield, seed);
            // 400 to 900 metres apart: ten to 23 within five minutes of driving.
            assert!(
                (10..=23).contains(&spawns.len()),
                "{} pickups for seed {seed}",
                spawns.len()
            );
            assert!(spawns.iter().all(|(_, _, kind)| kinds.contains(kind)));
            assert!(kinds.iter().all(|kind| spawns.iter().any(|s| s.2 == *kind)));
        }
    }
    assert!(pickup_trace(3, true, 1).is_empty());
}

#[test]
fn pickup_spawns_are_reproducible_with_a_seed() {
    let first = pickup_trace(2, false, 5);
    assert_eq!(first, pickup_trace(2, false, 5));
    assert_ne!(first, pickup_trace(2, false, 6));
}
