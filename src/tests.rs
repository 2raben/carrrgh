//! Deterministic checks through the actual fixed-step simulation.
use crate::difficulty::{Difficulty, RAMP_START};
use crate::level::{
    CAR_SIZE, EVASION_SIGNAL_SECONDS, LANES, LaneChange, ONCOMING_SPEED, PLAYER_START,
    ROAD_HALF_WIDTH, SAME_DIRECTION_SPEED, SIGNAL_SECONDS, TRAFFIC_SPACING, TRUCK_SIZE, Traffic,
    TrafficFlow, VehicleKind, blocks_all_lanes, traffic_is_safe,
};
use crate::pickups::{PICKUP_CLEARANCE, PICKUP_PASSED_Y, Pickup, PickupFlow, keeps_clear};
use crate::player::{Controls, Player, PlayerTuning, Position, PreviousPosition};
use crate::profile::{Account, NameDraft, ProfilePhase, ProfilePlugin};
use crate::run::{Run, swept_hit};
use crate::simulation::SimulationPlugin;
use crate::storage::memory::MemoryStore;
use crate::storage::sqlite::{SqliteStore, TempDir};
use crate::storage::{ProfileStore, StoreError};
use bevy::{prelude::*, time::TimeUpdateStrategy};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Sim(App);
impl Sim {
    fn new() -> Self {
        Self::build(App::new())
    }
    /// Adds profiles and storage; without them the drive starts immediately.
    fn with_store(store: impl ProfileStore) -> Self {
        let mut app = App::new();
        app.add_plugins(ProfilePlugin {
            store: Arc::new(store),
        });
        Self::build(app)
    }
    fn build(mut app: App) -> Self {
        app.add_plugins((MinimalPlugins, SimulationPlugin))
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
            .insert_resource(TrafficFlow {
                cooldown: 0.0,
                rng: fastrand::Rng::with_seed(7),
                ..default()
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
    fn phase(&self) -> ProfilePhase {
        *self.0.world().resource::<State<ProfilePhase>>().get()
    }
    fn account(&self) -> &Account {
        self.0.world().resource::<Account>()
    }
    fn balance(&self) -> Option<u64> {
        self.account().profile.as_ref().map(|profile| profile.coins)
    }
    fn draft(&mut self) -> Mut<'_, NameDraft> {
        self.0.world_mut().resource_mut::<NameDraft>()
    }
    fn submit_name(&mut self, name: &str) {
        let mut draft = self.draft();
        draft.text = name.into();
        draft.submitted = true;
    }
    /// Ends the current drive at the given distance by restarting.
    fn abandon_at(&mut self, distance: f64) {
        self.run_mut().distance = distance;
        self.controls().restart = true;
        self.steps(1);
        self.quiet();
    }
    /// Steps until background storage work is observed, with a hard deadline.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done(self) {
            assert!(Instant::now() < deadline, "{what} did not happen in time");
            std::thread::yield_now();
            self.0.update();
        }
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
    // The same random draw, mapped onto each level's delay range.
    let fraction = (base_interval - 1.2) / 0.8;
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
        (10_000.0, 10),
        (150_000.0, 150),
    ] {
        for boost in [false, true] {
            let mut sim = Sim::new();
            sim.0.world_mut().resource_mut::<Run>().distance = metres * 10.0;
            sim.controls().boost = boost;
            sim.steps(1);
            let base_speed = if boost { 540.0 } else { 300.0 };
            // Speed stops rising at level 10; traffic keeps getting denser.
            let expected = base_speed * (1.0 + 0.05 * step.min(9) as f32);
            assert!((sim.run().speed - expected).abs() < 0.001);
            assert_eq!(sim.run().level(), step + 1);
            let world = sim.0.world_mut();
            assert_eq!(world.query::<&Traffic>().iter(world).count(), 1);
            let interval = world.resource::<TrafficFlow>().cooldown;
            if step <= 9 {
                let expected_interval = base_interval / (1.0 + 0.15 * step as f32);
                assert!((interval - expected_interval).abs() < 0.001);
            } else {
                let expected = Difficulty::at(step + 1).spawn_delay(fraction);
                assert!((interval - expected).abs() < 0.001);
                assert!(interval < base_interval / 2.35 && interval >= 0.25);
            }
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
                    evasive: false,
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
        evasive: false,
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
fn difficulty_never_eases_and_ramps_towards_fixed_limits() {
    let mut previous = Difficulty::at(1);
    assert_eq!(previous, Difficulty::at(0), "level 0 is treated as level 1");
    for level in 2..=400 {
        let next = Difficulty::at(level);
        assert!(next.speed_multiplier >= previous.speed_multiplier);
        assert!(next.spawn_delay.start <= previous.spawn_delay.start);
        assert!(next.spawn_delay.end <= previous.spawn_delay.end);
        assert!(next.truck_chance >= previous.truck_chance);
        assert!(next.mix_chance >= previous.mix_chance);
        assert!(next.evasions >= previous.evasions);
        assert!(next.change_cooldown <= previous.change_cooldown);
        assert!(next.change_retry.start <= previous.change_retry.start);
        assert!(next.change_retry.end <= previous.change_retry.end);
        assert!(next.simultaneous_changes >= previous.simultaneous_changes);
        // Each level beyond the stepped phase is strictly denser for a long time.
        if level > RAMP_START && level <= 60 {
            assert!(next.spawn_delay.end < previous.spawn_delay.end);
            assert!(next.mix_chance > previous.mix_chance);
        }
        assert!(next.speed_multiplier <= 1.45 + 1e-6);
        assert!(next.spawn_delay.start >= 0.25 && next.spawn_delay.end >= 0.40);
        assert!(next.truck_chance <= 0.5 && next.mix_chance <= 0.8);
        assert!(next.change_cooldown >= 1.0);
        assert!(next.change_retry.start >= 0.2 && next.change_retry.end >= 0.6);
        assert!(next.simultaneous_changes <= 2);
        previous = next;
    }
    for level in 1..=RAMP_START {
        let d = Difficulty::at(level);
        let step = (level - 1) as f32;
        assert!((d.speed_multiplier - (1.0 + 0.05 * step)).abs() < 1e-6);
        assert!((d.spawn_delay.end - 2.0 / (1.0 + 0.15 * step)).abs() < 1e-6);
        let trucks = [0.0, 0.0, 0.0, 0.0, 0.2, 0.2, 0.2, 0.35, 0.35, 0.35];
        assert_eq!(d.truck_chance, trucks[level as usize - 1]);
        assert_eq!(d.mix_chance == 0.0, level < 3);
        assert_eq!(d.evasions, level >= 6);
        assert_eq!((d.change_cooldown, d.simultaneous_changes), (2.0, 1));
    }
    assert!((Difficulty::at(10).mix_chance - 0.5).abs() < 1e-6);
    assert_eq!(Difficulty::at(11).simultaneous_changes, 2);
    let run = Run {
        distance: 1e30,
        ..default()
    };
    assert_eq!(
        run.level(),
        u32::MAX,
        "huge distances saturate instead of overflowing"
    );
}

#[test]
fn head_on_lane_sharing_requires_a_reserved_evasion() {
    let tuning = PlayerTuning::default();
    let mixed = Difficulty::at(6);
    // Before level 3, lanes keep a single direction.
    let separate = Difficulty::at(2);
    let diverging = (
        Traffic::new(1, SAME_DIRECTION_SPEED, VehicleKind::Car),
        Vec2::new(LANES[1], 480.0),
    );
    let below = Vec2::new(LANES[1], -100.0);
    let oncoming_below = Traffic::new(1, ONCOMING_SPEED, VehicleKind::Car);
    assert!(!traffic_is_safe(
        (oncoming_below, below),
        &[diverging],
        &tuning,
        &separate
    ));
    let same = |y| {
        (
            Traffic::new(1, SAME_DIRECTION_SPEED, VehicleKind::Car),
            Vec2::new(LANES[1], y),
        )
    };
    let oncoming = Traffic::new(1, ONCOMING_SPEED, VehicleKind::Car);
    let at = Vec2::new(LANES[1], 480.0);
    let evading = |target: usize| Traffic {
        change: Some(LaneChange {
            target_lane: target,
            start_x: at.x,
            target_x: LANES[target],
            elapsed: 0.0,
            evasive: true,
        }),
        ..oncoming
    };
    // Closing in on screen without an evasion is never admitted.
    assert!(!traffic_is_safe(
        (oncoming, at),
        &[same(-100.0)],
        &tuning,
        &mixed
    ));
    // An evasion that finishes before the gap reaches 180 units is admitted.
    assert!(traffic_is_safe(
        (evading(0), at),
        &[same(-100.0)],
        &tuning,
        &mixed
    ));
    assert!(traffic_is_safe(
        (evading(2), at),
        &[same(-100.0)],
        &tuning,
        &mixed
    ));
    // Too close for the 1.1-second evasion.
    assert!(!traffic_is_safe(
        (evading(0), at),
        &[same(100.0)],
        &tuning,
        &mixed
    ));
    // A slower truck evasion needs a longer gap.
    let truck = Traffic {
        kind: VehicleKind::Truck,
        ..evading(0)
    };
    assert!(traffic_is_safe(
        (evading(0), at),
        &[same(-50.0)],
        &tuning,
        &mixed
    ));
    assert!(!traffic_is_safe(
        (truck, at),
        &[same(-50.0)],
        &tuning,
        &mixed
    ));
    // No evasion chains: never evade into the lane of the vehicle given way to.
    let mut changing = same(-100.0);
    changing.0.change = Some(LaneChange {
        target_lane: 0,
        start_x: LANES[1],
        target_x: LANES[0],
        elapsed: 0.0,
        evasive: false,
    });
    assert!(!traffic_is_safe(
        (evading(0), at),
        &[changing],
        &tuning,
        &mixed
    ));
    // Nor head-on into same-direction traffic in the evasion's target lane.
    let other_lane = (
        Traffic::new(0, SAME_DIRECTION_SPEED, VehicleKind::Car),
        Vec2::new(LANES[0], -50.0),
    );
    assert!(!traffic_is_safe(
        (evading(0), at),
        &[other_lane],
        &tuning,
        &mixed
    ));
    // Meetings after the same-direction vehicle has left the road do not count.
    assert!(traffic_is_safe(
        (oncoming, at),
        &[same(-300.0)],
        &tuning,
        &mixed
    ));
    // Diverging traffic shares a lane once the usual spacing is kept.
    let behind = Vec2::new(LANES[1], -100.0);
    let ahead = (
        Traffic::new(1, SAME_DIRECTION_SPEED, VehicleKind::Car),
        Vec2::new(LANES[1], 480.0),
    );
    assert!(traffic_is_safe(
        (oncoming, behind),
        &[ahead],
        &tuning,
        &mixed
    ));
    assert!(!traffic_is_safe(
        (oncoming, Vec2::new(LANES[1], 400.0)),
        &[ahead],
        &tuning,
        &mixed
    ));
}

#[test]
fn evasions_signal_briefly_and_release_the_lane_before_the_gap_closes() {
    for kind in [VehicleKind::Car, VehicleKind::Truck] {
        let mut sim = Sim::new();
        sim.quiet();
        sim.0.world_mut().resource_mut::<Run>().protection = 1000.0;
        let same = sim.car(Vec2::new(LANES[1], -120.0), SAME_DIRECTION_SPEED);
        let oncoming = sim.car(Vec2::new(LANES[1], 480.0), ONCOMING_SPEED);
        {
            let mut car = sim.0.world_mut().get_mut::<Traffic>(oncoming).unwrap();
            car.kind = kind;
            car.change = Some(LaneChange {
                target_lane: 2,
                start_x: LANES[1],
                target_x: LANES[2],
                elapsed: 0.0,
                evasive: true,
            });
        }
        let mut signalled = false;
        let mut closest_shared = f32::INFINITY;
        for tick in 1..=128 {
            sim.steps(1);
            let world = sim.0.world();
            let car = world.get::<Traffic>(oncoming).unwrap();
            let x = world.get::<Position>(oncoming).unwrap().0.x;
            let gap = world.get::<Position>(oncoming).unwrap().0.y
                - world.get::<Position>(same).unwrap().0.y;
            if (tick as f32) < EVASION_SIGNAL_SECONDS * 64.0 {
                assert_eq!(x, LANES[1], "no movement during the warning");
                signalled |= car.signal_side() == Some(1.0);
            }
            if car.occupies(1) {
                closest_shared = closest_shared.min(gap);
            }
        }
        let car = sim.0.world().get::<Traffic>(oncoming).unwrap();
        assert!(signalled);
        assert_eq!((car.lane, car.change.is_none()), (2, true));
        assert!(
            closest_shared >= TRAFFIC_SPACING,
            "{kind:?}: {closest_shared}"
        );
    }
}

#[test]
fn from_level_11_two_voluntary_lane_changes_may_run_at_once() {
    for (level, expected) in [(10, 1), (11, 2)] {
        let mut sim = Sim::new();
        sim.quiet();
        sim.run_mut().distance = f64::from(level - 1) * 10_000.0;
        for at in [Vec2::new(LANES[0], 170.0), Vec2::new(LANES[2], 355.0)] {
            let id = sim.car(at, SAME_DIRECTION_SPEED);
            sim.0
                .world_mut()
                .get_mut::<Traffic>(id)
                .unwrap()
                .change_cooldown = 0.0;
        }
        sim.steps(2);
        let world = sim.0.world_mut();
        let changing = world
            .query::<&Traffic>()
            .iter(world)
            .filter(|car| car.change.is_some())
            .count();
        assert_eq!(changing, expected, "level {level}");
    }
}

#[test]
fn random_traffic_is_continuous_spaced_and_never_forms_a_three_lane_wall() {
    // Head-on evasions need a narrow on-screen gap, so they are counted overall.
    let mut evasions = 0;
    for level in (1..=10).chain([15, 20, 30, 50]) {
        let mut mixed_lanes = 0;
        for seed in [1, 7, 42] {
            let mut sim = Sim::new();
            sim.0.world_mut().resource_mut::<TrafficFlow>().rng = fastrand::Rng::with_seed(seed);
            sim.0.world_mut().resource_mut::<Run>().distance = f64::from(level - 1) * 10_000.0;
            // Observe traffic independently of a stationary player's damage.
            sim.0.world_mut().resource_mut::<Run>().protection = 1000.0;
            let mut previous = Vec::new();
            let mut overlapping_spawns = 0;
            let mut used_lanes = [false; 3];
            let mut directions = [false; 2];
            for tick in 0..3840 {
                // Include braking, boost precedence, releases and recharges.
                sim.controls().boost = (tick / 160) % 2 == 0;
                sim.controls().brake = (tick / 96) % 3 == 0;
                sim.steps(1);
                // Driving raises the level during the run.
                let current = Difficulty::at(sim.run().level());
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
                    if car.change.is_some_and(|change| change.evasive) {
                        assert!(current.evasions && car.speed < 0.0);
                        evasions += 1;
                    }
                }
                for (i, (_, a, car_a)) in cars.iter().enumerate() {
                    for (j, (_, b, car_b)) in cars.iter().enumerate().skip(i + 1) {
                        if (0..3).any(|lane| car_a.occupies(lane) && car_b.occupies(lane)) {
                            // Head-on pairs never close in: evasions release the lane first.
                            assert!(
                                (a.y - b.y).abs() >= TRAFFIC_SPACING - 0.01,
                                "lane spacing at level {level}, seed {seed}, tick {tick}"
                            );
                            if car_a.speed != car_b.speed {
                                assert!(current.mix_chance > 0.0, "mixed before level 3");
                                mixed_lanes += 1;
                            }
                        }
                        let half_size = (car_a.kind.size() + car_b.kind.size()) * 0.5;
                        assert!(!(*a - *b).abs().cmple(half_size).all(), "traffic overlap");
                        for (_, c, car_c) in cars.iter().skip(j + 1) {
                            assert!(
                                !blocks_all_lanes(
                                    [(*car_a, *a), (*car_b, *b), (*car_c, *c)],
                                    300.0
                                ),
                                "wall at level {level}, seed {seed}, tick {tick}"
                            );
                        }
                    }
                }
                previous = cars.iter().map(|(id, _, _)| *id).collect();
            }
            assert!(overlapping_spawns > 10);
            assert!(used_lanes.into_iter().all(|used| used));
            assert!(
                directions.into_iter().all(|used| used),
                "directions {directions:?} at level {level}, seed {seed}"
            );
        }
        if level >= 3 {
            assert!(mixed_lanes > 0, "mixed lanes from level {level}");
        }
    }
    assert!(evasions > 0);
}

#[test]
fn oncoming_spawns_evade_same_direction_traffic_only_from_level_6() {
    for (level, expect) in [(5, false), (6, true), (30, true)] {
        let mut evasions = 0;
        for seed in 0..100 {
            let mut sim = Sim::new();
            sim.run_mut().distance = f64::from(level - 1) * 10_000.0;
            sim.0.world_mut().resource_mut::<TrafficFlow>().rng = fastrand::Rng::with_seed(seed);
            sim.0.world_mut().resource_mut::<PickupFlow>().remaining = f32::INFINITY;
            sim.car(Vec2::new(LANES[1], -100.0), SAME_DIRECTION_SPEED);
            sim.steps(1);
            let world = sim.0.world_mut();
            for car in world.query::<&Traffic>().iter(world) {
                if let Some(change) = car.change.filter(|change| change.evasive) {
                    assert_eq!((car.lane, car.speed), (1, ONCOMING_SPEED));
                    assert_ne!(change.target_lane, 1);
                    evasions += 1;
                }
            }
        }
        assert_eq!(evasions > 0, expect, "level {level}: {evasions}");
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
    // A gap safe at cruise is not enough when the player brakes.
    assert!(!keeps_clear(480.0, 1180.0, -150.0, cruise));
    assert!(keeps_clear(480.0, 1300.0, -150.0, cruise));
    // The pickup catches a slower same-direction car ahead of it.
    assert!(!keeps_clear(480.0, 180.0, 140.0, cruise));
    assert!(!keeps_clear(480.0, -200.0, 140.0, cruise));
    assert!(keeps_clear(480.0, -300.0, 140.0, cruise));
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

#[test]
fn coins_count_whole_tens_of_metres_regardless_of_boost_or_level() {
    let coins = |distance, boosting, lives| {
        Run {
            distance,
            boosting,
            lives,
            ..default()
        }
        .coins()
    };
    assert_eq!(coins(0.0, false, 3), 0);
    assert_eq!(coins(99.9, false, 3), 0);
    assert_eq!(coins(100.0, false, 3), 1);
    assert_eq!(coins(12_345.0, false, 3), 123);
    assert_eq!(coins(95_000.0, true, 1), 950);
}

#[test]
fn a_saved_profile_starts_the_drive_directly() {
    let mut sim = Sim::with_store(MemoryStore::with_profile("Ada", 40));
    sim.steps(1);
    assert_eq!(sim.phase(), ProfilePhase::Playing);
    assert_eq!(sim.balance(), Some(40));
    assert!(sim.run().distance > 0.0);
}

#[test]
fn without_a_profile_no_drive_starts_until_a_valid_name_is_saved() {
    let store = MemoryStore::default();
    let mut sim = Sim::with_store(store.clone());
    sim.steps(20);
    assert_eq!(sim.phase(), ProfilePhase::Creating);
    assert_eq!(sim.run().distance, 0.0);

    sim.submit_name("   ");
    sim.steps(2);
    assert_eq!(sim.phase(), ProfilePhase::Creating);
    assert!(sim.draft().error.is_some());
    assert!(store.profiles().is_empty());

    sim.submit_name("  Ada ");
    sim.steps(2);
    assert_eq!(sim.phase(), ProfilePhase::Playing);
    assert_eq!(store.profiles()[0].name.as_str(), "Ada");
    assert_eq!(sim.balance(), Some(0));
    sim.steps(1);
    assert!(sim.run().distance > 0.0);
}

#[test]
fn every_drive_is_credited_exactly_once() {
    let store = MemoryStore::with_profile("Ada", 5);
    let mut sim = Sim::with_store(store.clone());
    sim.steps(1);
    sim.quiet();

    sim.abandon_at(1_234.0);
    assert_eq!(store.coins(), 5 + 12);

    sim.run_mut().lives = 1;
    sim.run_mut().distance = 5_000.0;
    sim.car(PLAYER_START, 300.0);
    sim.steps(1);
    assert!(sim.run().finished);
    let earned = sim.run().coins();
    assert_eq!(earned, 50);
    assert_eq!(store.coins(), 17 + earned);
    assert_eq!(sim.balance(), Some(17 + earned));

    // Restarting after game over and quitting after that add nothing.
    sim.controls().restart = true;
    sim.steps(3);
    sim.0.world_mut().write_message(AppExit::Success);
    sim.steps(1);
    assert_eq!(store.coins(), 17 + earned);
    assert!(!sim.account().saving());
}

#[test]
fn quitting_mid_drive_credits_it_before_exit() {
    let store = MemoryStore::with_profile("Ada", 0);
    let mut sim = Sim::with_store(store.clone());
    sim.steps(1);
    sim.quiet();
    sim.run_mut().distance = 2_000.0;
    sim.0.world_mut().write_message(AppExit::Success);
    sim.steps(1);
    assert_eq!(store.coins(), 20);
}

#[test]
fn failed_credits_are_kept_and_retried_at_the_next_drive_end() {
    let store = MemoryStore::with_profile("Ada", 0);
    let mut sim = Sim::with_store(store.clone());
    sim.steps(1);
    sim.quiet();
    store.fail_with(Some(StoreError::Unavailable("disk full".into())));
    sim.abandon_at(1_000.0);
    assert!(sim.account().saving());
    assert!(sim.account().problem.is_some());
    assert_eq!(store.coins(), 0);

    store.fail_with(None);
    sim.steps(10);
    assert!(sim.account().saving());
    sim.abandon_at(500.0);
    // One storage operation per frame: the retried credit, then the new one.
    assert_eq!(store.coins(), 10);
    sim.steps(1);
    assert_eq!(store.coins(), 15);
    assert!(!sim.account().saving());
    assert!(sim.account().problem.is_none());
}

#[test]
fn unreadable_storage_leaves_the_game_playable_without_saving() {
    let store = MemoryStore::default();
    store.fail_with(Some(StoreError::Invalid("corrupt".into())));
    let mut sim = Sim::with_store(store);
    sim.steps(1);
    assert_eq!(sim.phase(), ProfilePhase::Playing);
    assert!(sim.account().profile.is_none());
    assert!(sim.account().problem.is_some());
    assert!(sim.run().distance > 0.0);
    sim.abandon_at(3_000.0);
    assert!(!sim.account().saving());
}

#[test]
fn sqlite_profile_and_coins_survive_restarting_the_game() {
    let dir = TempDir::new();
    let store = || SqliteStore::new(Some(dir.file()));

    let mut sim = Sim::with_store(store());
    sim.until("loading", |sim| sim.phase() == ProfilePhase::Creating);
    sim.submit_name("Ada");
    sim.until("creating", |sim| sim.phase() == ProfilePhase::Playing);
    sim.quiet();
    sim.abandon_at(4_321.0);
    sim.until("crediting", |sim| !sim.account().saving());
    assert_eq!(sim.balance(), Some(43));
    drop(sim);

    let mut sim = Sim::with_store(store());
    sim.until("reloading", |sim| sim.phase() == ProfilePhase::Playing);
    let profile = sim.account().profile.clone().unwrap();
    assert_eq!(profile.name.as_str(), "Ada");
    assert_eq!(profile.coins, 43);
}

#[test]
fn every_vehicle_uses_its_actual_collision_box_and_road_limits() {
    use crate::vehicles::{MODELS, Vehicle};
    for model in MODELS {
        let mut sim = Sim::new();
        sim.quiet();
        let tuning = Vehicle::new("test".into(), model).tuning();
        sim.0.world_mut().insert_resource(tuning);
        sim.controls().move_x = 1.;
        sim.steps(100);
        assert_eq!(
            sim.position().x,
            ROAD_HALF_WIDTH - tuning.size.x * 0.5,
            "{}",
            model.name()
        );
        sim.controls().move_x = 0.;
        let at = sim.position();
        let half = (tuning.size.x + CAR_SIZE.x) * 0.5;
        sim.car(at - Vec2::new(half - 1., 0.), 300.);
        sim.steps(1);
        assert_eq!(sim.run().lives, 2, "{}", model.name());
    }
}

#[test]
fn garage_pauses_simulation_and_applies_only_confirmed_vehicle_changes() {
    use crate::garage::{Garage, GaragePlugin, Intent};
    use crate::vehicles::{GarageAction, Model};
    let dir = TempDir::new();
    let store = SqliteStore::new(Some(dir.file()));
    let p = bevy::tasks::block_on(store.create(crate::storage::PlayerName::parse("Ada").unwrap()))
        .unwrap();
    bevy::tasks::block_on(store.credit(crate::storage::RunCredit {
        run: crate::storage::RunId::random(),
        profile: p.id,
        coins: 100,
        proof: None,
    }))
    .unwrap();
    let mut app = App::new();
    app.add_plugins((
        ProfilePlugin {
            store: Arc::new(store),
        },
        GaragePlugin,
    ));
    let mut sim = Sim::build(app);
    sim.until("garage", |s| {
        s.0.world().resource::<Garage>().data.is_some()
    });
    sim.steps(10);
    assert_eq!(sim.run().distance, 0.);
    sim.0
        .world_mut()
        .resource_mut::<Garage>()
        .enqueue(Intent::Action(GarageAction::Buy(Model::BobbyCar)));
    sim.steps(1);
    assert_eq!(sim.balance(), Some(100));
    sim.0
        .world_mut()
        .resource_mut::<Garage>()
        .enqueue(Intent::Confirm);
    sim.until("purchase", |s| s.balance() == Some(45));
    let data = sim.0.world().resource::<Garage>().data.as_ref().unwrap();
    let car = data
        .vehicles
        .iter()
        .find(|v| v.model == Model::BobbyCar)
        .unwrap()
        .id
        .clone();
    sim.0
        .world_mut()
        .resource_mut::<Garage>()
        .enqueue(Intent::Action(GarageAction::Select(car.clone())));
    sim.steps(1);
    sim.0
        .world_mut()
        .resource_mut::<Garage>()
        .enqueue(Intent::Confirm);
    sim.until("selection", |s| {
        s.0.world()
            .resource::<Garage>()
            .data
            .as_ref()
            .unwrap()
            .active
            == car
    });
    assert_eq!(
        sim.0.world().resource::<PlayerTuning>().size,
        Model::BobbyCar.size()
    );
    sim.0
        .world_mut()
        .resource_mut::<Garage>()
        .enqueue(Intent::Start);
    sim.until("start", |s| s.run().distance > 0.);
    assert_eq!(sim.balance(), Some(45));
    sim.0
        .world_mut()
        .resource_mut::<Garage>()
        .enqueue(Intent::Action(GarageAction::Buy(Model::BobbyCar)));
    sim.steps(3);
    assert!(sim.0.world().resource::<Garage>().confirmation.is_none());
}

#[test]
fn online_garage_start_does_not_credit_an_empty_or_previous_ticket() {
    use crate::drive_proof::{DriveTicket, verify};
    use crate::garage::{Garage, GaragePlugin, Intent};
    use crate::storage::{PlayerName, Profile, ProfileId, RunCredit, StoreResult};
    use crate::vehicles::{GarageRequest, GarageSnapshot, Model, Vehicle};
    use bevy::tasks::{BoxedFuture, block_on};
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[derive(Clone)]
    struct ProofStore {
        inner: SqliteStore,
        credited: Arc<AtomicUsize>,
    }
    impl ProfileStore for ProofStore {
        fn online(&self) -> bool {
            true
        }
        fn load(&self) -> BoxedFuture<'static, StoreResult<Option<Profile>>> {
            self.inner.load()
        }
        fn create(&self, n: PlayerName) -> BoxedFuture<'static, StoreResult<Profile>> {
            self.inner.create(n)
        }
        fn garage(
            &self,
            p: ProfileId,
            r: GarageRequest,
        ) -> BoxedFuture<'static, StoreResult<GarageSnapshot>> {
            self.inner.garage(p, r)
        }
        fn begin_drive(
            &self,
            id: String,
        ) -> BoxedFuture<'static, StoreResult<Option<DriveTicket>>> {
            Box::pin(async move {
                Ok(Some(DriveTicket {
                    id,
                    seed: 54,
                    vehicle: Vehicle::new("test".into(), Model::Corsa),
                }))
            })
        }
        fn credit(&self, c: RunCredit) -> BoxedFuture<'static, StoreResult<Profile>> {
            let proof = c.proof.as_ref().expect("online credit must include inputs");
            assert!(
                !proof.frames.is_empty(),
                "starting a ticket must not immediately submit it"
            );
            let ticket = DriveTicket {
                id: proof.ticket.clone(),
                seed: 54,
                vehicle: Vehicle::new("test".into(), Model::Corsa),
            };
            assert_eq!(verify(&ticket, proof).unwrap(), c.coins);
            self.credited.fetch_add(1, Ordering::SeqCst);
            self.inner.credit(c)
        }
    }
    let dir = TempDir::new();
    let inner = SqliteStore::new(Some(dir.file()));
    block_on(inner.create(PlayerName::parse("Online").unwrap())).unwrap();
    let credited = Arc::new(AtomicUsize::new(0));
    let mut app = App::new();
    app.add_plugins((
        ProfilePlugin {
            store: Arc::new(ProofStore {
                inner,
                credited: credited.clone(),
            }),
        },
        GaragePlugin,
    ));
    let mut sim = Sim::build(app);
    sim.until("garage", |s| {
        s.0.world().resource::<Garage>().data.is_some()
    });
    for expected in 1..=2 {
        sim.0
            .world_mut()
            .resource_mut::<Garage>()
            .enqueue(Intent::Start);
        sim.until("drive started", |s| {
            !s.0.world().resource::<Garage>().open && s.run().distance > 0. && !s.run().finished
        });
        assert_eq!(credited.load(Ordering::SeqCst), expected - 1);
        sim.steps(64);
        sim.controls().end_drive = true;
        sim.until("verified credit", |s| {
            s.run().finished && !s.account().saving() && credited.load(Ordering::SeqCst) == expected
        });
        sim.0
            .world_mut()
            .resource_mut::<Garage>()
            .enqueue(Intent::Open);
        sim.until("garage reopened", |s| {
            s.0.world().resource::<Garage>().open && !s.0.world().resource::<Garage>().busy()
        });
    }
}

#[test]
fn braking_slows_distance_and_traffic_while_preserving_steering_and_recharging() {
    let mut sim = Sim::new();
    sim.quiet();
    sim.run_mut().energy = 0.5;
    let start_x = sim.position().x;
    let car = sim.car(Vec2::new(LANES[0], 400.), SAME_DIRECTION_SPEED);
    sim.controls().brake = true;
    sim.controls().boost = true;
    sim.controls().move_x = -1.;
    sim.steps(32);
    let run = sim.run();
    assert_eq!(run.speed, 225.);
    assert!(!run.boosting);
    assert!(run.energy > 0.5);
    assert!((run.distance - 112.5).abs() < 0.01);
    assert!(sim.position().x < start_x);
    assert!((sim.0.world().get::<Position>(car).unwrap().0.y - 357.5).abs() < 0.01);
    sim.controls().boost = false;
    sim.controls().brake = false;
    sim.steps(1);
    assert_eq!(sim.run().speed, 300.);
    sim.controls().brake = true;
    sim.controls().restart = true;
    sim.steps(1);
    assert!(!sim.controls().brake);
    assert_eq!(sim.run().distance, 0.);
    sim.run_mut().finished = true;
    sim.controls().brake = true;
    sim.steps(5);
    assert_eq!(sim.run().distance, 0.);
}

#[test]
fn every_vehicle_keeps_moving_faster_than_traffic_when_braking_at_every_difficulty() {
    for model in crate::vehicles::MODELS {
        let tuning = crate::vehicles::Vehicle::new("test".into(), model).tuning();
        assert!(tuning.brake_speed() > SAME_DIRECTION_SPEED);
        for level in [1, 5, 10, 50] {
            let multiplier = Difficulty::at(level).speed_multiplier;
            assert!(tuning.brake_speed() * multiplier < tuning.cruise_speed * multiplier);
        }
    }
}
