//! Endless levels: one pure mapping from the level to speed and traffic tuning.
use std::ops::Range;

/// Last level of the stepped phase; later levels ramp towards fixed limits.
pub const RAMP_START: u32 = 10;
/// Each level beyond `RAMP_START` closes this share of the remaining distance
/// to the limits: `r(L) = 1 − RAMP_BASE^(L − RAMP_START)`.
const RAMP_BASE: f32 = 0.85;

/// Play-test values for one level. Every value is monotonic in the level and
/// never passes its limit; fairness rules (signals, spacing, wall checks,
/// traffic speeds) are deliberately not part of the profile.
#[derive(Clone, Debug, PartialEq)]
pub struct Difficulty {
    /// Applied to the player's cruise and boost speed.
    pub speed_multiplier: f32,
    /// Random range of seconds between successful traffic spawns.
    pub spawn_delay: Range<f32>,
    pub truck_chance: f32,
    /// Chance that a spawn ignores the direction already used in its lane.
    pub mix_chance: f32,
    /// Whether oncoming traffic may be admitted head-on with a reserved evasion.
    pub evasions: bool,
    /// Seconds before a vehicle may plan another voluntary lane change.
    pub change_cooldown: f32,
    /// Random range of seconds before a blocked lane change is retried.
    pub change_retry: Range<f32>,
    /// Voluntary lane changes that may be active at once; evasions do not count.
    pub simultaneous_changes: usize,
}

impl Difficulty {
    pub fn at(level: u32) -> Self {
        let stepped = level.clamp(1, RAMP_START);
        let step = (stepped - 1) as f32;
        let divisor = 1.0 + 0.15 * step;
        let base = Self {
            speed_multiplier: 1.0 + 0.05 * step,
            spawn_delay: 1.2 / divisor..2.0 / divisor,
            truck_chance: match stepped {
                ..=4 => 0.0,
                5..=7 => 0.2,
                _ => 0.35,
            },
            mix_chance: match stepped {
                ..=2 => 0.0,
                _ => 0.15 + 0.05 * (stepped - 3) as f32,
            },
            evasions: stepped >= 6,
            change_cooldown: 2.0,
            change_retry: 0.4..1.2,
            simultaneous_changes: 1,
        };
        if level <= RAMP_START {
            return base;
        }
        let r = 1.0 - RAMP_BASE.powf((level - RAMP_START) as f32);
        let towards = |from: f32, limit: f32| from + (limit - from) * r;
        Self {
            spawn_delay: towards(base.spawn_delay.start, 0.25)..towards(base.spawn_delay.end, 0.40),
            truck_chance: towards(base.truck_chance, 0.5),
            mix_chance: towards(base.mix_chance, 0.8),
            change_cooldown: towards(base.change_cooldown, 1.0),
            change_retry: towards(base.change_retry.start, 0.2)
                ..towards(base.change_retry.end, 0.6),
            simultaneous_changes: 2,
            ..base
        }
    }

    /// Picks a spawn delay from a uniform random fraction in `0..1`.
    pub fn spawn_delay(&self, fraction: f32) -> f32 {
        lerp(&self.spawn_delay, fraction)
    }

    /// Picks a lane-change retry delay from a uniform random fraction in `0..1`.
    pub fn retry_delay(&self, fraction: f32) -> f32 {
        lerp(&self.change_retry, fraction)
    }
}

fn lerp(range: &Range<f32>, fraction: f32) -> f32 {
    range.start + (range.end - range.start) * fraction
}
