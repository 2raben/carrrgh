//! Versioned vehicle catalogue and pure garage rules. No storage or drawing.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::player::PlayerTuning;

pub const CATALOG_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Model {
    BobbyCar,
    #[default]
    Corsa,
    Rs6,
    F150,
    Jesko,
    M1,
    Mazda5,
    Fendt1050,
    Ateca,
    Xc60,
}

pub const MODELS: [Model; 10] = [
    Model::BobbyCar,
    Model::Corsa,
    Model::Rs6,
    Model::F150,
    Model::Jesko,
    Model::M1,
    Model::Mazda5,
    Model::Fendt1050,
    Model::Ateca,
    Model::Xc60,
];

impl Model {
    pub fn name(self) -> &'static str {
        match self {
            Self::BobbyCar => "Bobby-Car",
            Self::Corsa => "Opel Corsa",
            Self::Rs6 => "Audi RS6",
            Self::F150 => "Ford F150",
            Self::Jesko => "Koenigsegg Jesko",
            Self::M1 => "BMW M1",
            Self::Mazda5 => "Mazda5",
            Self::Fendt1050 => "Fendt 1050",
            Self::Ateca => "Seat Ateca",
            Self::Xc60 => "Volvo XC60",
        }
    }

    pub fn price(self) -> u64 {
        match self {
            Self::BobbyCar => 55,
            Self::Corsa | Self::Mazda5 => 23_000,
            Self::Rs6 => 140_000,
            Self::F150 => 40_000,
            Self::Jesko => 2_800_000,
            Self::M1 => 500_000,
            Self::Fendt1050 => 425_000,
            Self::Ateca => 33_000,
            Self::Xc60 => 60_000,
        }
    }

    pub fn size(self) -> Vec2 {
        let (w, h) = match self {
            Self::BobbyCar => (30., 42.),
            Self::Corsa => (44., 78.),
            Self::Rs6 => (50., 94.),
            Self::F150 => (60., 108.),
            Self::Jesko => (54., 88.),
            Self::M1 => (48., 82.),
            Self::Mazda5 => (48., 98.),
            Self::Fendt1050 => (76., 110.),
            Self::Ateca => (50., 84.),
            Self::Xc60 => (56., 92.),
        };
        Vec2::new(w, h)
    }

    fn tuning(self) -> PlayerTuning {
        // Even the slowest model overtakes 140-unit/s traffic. Maximum upgraded
        // boost stays below 1,000 units/s at difficulty ten.
        let (cruise, boost, steer, drain, recharge) = match self {
            Self::BobbyCar => (220., 350., 500., 0.55, 0.25),
            Self::Corsa => (300., 540., 420., 0.4, 0.2),
            Self::Rs6 => (340., 590., 400., 0.42, 0.2),
            Self::F150 => (280., 480., 330., 0.28, 0.18),
            Self::Jesko => (380., 620., 430., 0.6, 0.18),
            Self::M1 => (330., 570., 470., 0.48, 0.22),
            Self::Mazda5 => (275., 470., 390., 0.38, 0.3),
            Self::Fendt1050 => (210., 340., 300., 0.24, 0.2),
            Self::Ateca => (310., 530., 395., 0.38, 0.22),
            Self::Xc60 => (305., 520., 365., 0.3, 0.22),
        };
        PlayerTuning {
            cruise_speed: cruise,
            boost_speed: boost,
            steer_speed: steer,
            boost_drain: drain,
            boost_recharge: recharge,
            size: self.size(),
            ..default()
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Paint {
    #[default]
    Blue,
    Red,
    Yellow,
    Green,
    White,
    Black,
    Purple,
    Orange,
}
pub const PAINTS: [Paint; 8] = [
    Paint::Blue,
    Paint::Red,
    Paint::Yellow,
    Paint::Green,
    Paint::White,
    Paint::Black,
    Paint::Purple,
    Paint::Orange,
];
impl Paint {
    pub fn name(self) -> &'static str {
        match self {
            Self::Blue => "Blue",
            Self::Red => "Red",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::White => "White",
            Self::Black => "Black",
            Self::Purple => "Purple",
            Self::Orange => "Orange",
        }
    }
    pub fn rgb(self) -> [f32; 3] {
        match self {
            Self::Blue => [0.22, 0.72, 0.95],
            Self::Red => [0.92, 0.22, 0.23],
            Self::Yellow => [0.98, 0.8, 0.18],
            Self::Green => [0.26, 0.69, 0.36],
            Self::White => [0.9, 0.92, 0.94],
            Self::Black => [0.18, 0.21, 0.25],
            Self::Purple => [0.67, 0.4, 0.89],
            Self::Orange => [1.0, 0.48, 0.15],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tires {
    #[default]
    Standard,
    Sport,
    Efficiency,
}
impl Tires {
    pub fn name(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::Sport => "Sport",
            Self::Efficiency => "Efficiency",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Upgrade {
    SportTires,
    EfficiencyTires,
    Motor1,
    Motor2,
    Carbon,
}
pub const UPGRADES: [Upgrade; 5] = [
    Upgrade::SportTires,
    Upgrade::EfficiencyTires,
    Upgrade::Motor1,
    Upgrade::Motor2,
    Upgrade::Carbon,
];
impl Upgrade {
    pub fn name(self) -> &'static str {
        match self {
            Self::SportTires => "Sport tires",
            Self::EfficiencyTires => "Efficiency tires",
            Self::Motor1 => "Motor stage 1",
            Self::Motor2 => "Motor stage 2",
            Self::Carbon => "Carbon body kit",
        }
    }
    pub fn price(self, model: Model) -> u64 {
        let percent = match self {
            Self::SportTires | Self::EfficiencyTires => 2,
            Self::Motor1 => 5,
            Self::Motor2 => 10,
            Self::Carbon => 8,
        };
        (model.price() * percent).div_ceil(100)
    }
    fn bit(self) -> u8 {
        match self {
            Self::SportTires => 1,
            Self::EfficiencyTires => 2,
            Self::Motor1 => 4,
            Self::Motor2 => 8,
            Self::Carbon => 16,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vehicle {
    pub id: String,
    pub version: u32,
    pub model: Model,
    pub paint: Paint,
    pub tires: Tires,
    pub motor: u8,
    pub carbon: bool,
    pub owned_upgrades: u8,
}
impl Vehicle {
    pub fn new(id: String, model: Model) -> Self {
        Self {
            id,
            version: CATALOG_VERSION,
            model,
            paint: Paint::Blue,
            tires: Tires::Standard,
            motor: 0,
            carbon: false,
            owned_upgrades: 0,
        }
    }
    pub fn owns(&self, upgrade: Upgrade) -> bool {
        self.owned_upgrades & upgrade.bit() != 0
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != CATALOG_VERSION
            || self.id.is_empty()
            || self.id.len() > 64
            || self.owned_upgrades & !31 != 0
            || self.motor > 2
            || (self.owns(Upgrade::Motor2) && !self.owns(Upgrade::Motor1))
            || (self.motor >= 1 && !self.owns(Upgrade::Motor1))
            || (self.motor == 2 && !self.owns(Upgrade::Motor2))
            || (self.carbon && !self.owns(Upgrade::Carbon))
            || (self.tires == Tires::Sport && !self.owns(Upgrade::SportTires))
            || (self.tires == Tires::Efficiency && !self.owns(Upgrade::EfficiencyTires))
        {
            return Err("Invalid or newer vehicle configuration".into());
        }
        Ok(())
    }
    pub fn upgrade_cost(&self) -> u64 {
        UPGRADES
            .into_iter()
            .filter(|u| self.owns(*u))
            .map(|u| u.price(self.model))
            .sum()
    }
    pub fn sale_price(&self) -> u64 {
        (70 * self.model.price() + 50 * self.upgrade_cost()) / 100
    }
    pub fn market_price(&self) -> u64 {
        (85 * self.model.price() + 75 * self.upgrade_cost()).div_ceil(100)
    }
    pub fn tuning(&self) -> PlayerTuning {
        let mut t = self.model.tuning();
        let (steer, duration) = match self.tires {
            Tires::Standard => (0., 1.),
            Tires::Sport => (0.1, 0.95),
            Tires::Efficiency => (-0.05, 1.1),
        };
        t.steer_speed =
            (t.steer_speed * (1. + steer + if self.carbon { 0.05 } else { 0. })).min(560.);
        let motor = 1. + f32::from(self.motor) * 0.05;
        t.cruise_speed *= motor;
        t.boost_speed *= motor;
        t.boost_drain /= duration;
        t
    }
    /// Applies a workshop choice and returns its price; callers transact the coin debit.
    pub fn configure(&mut self, change: &Change) -> Result<u64, String> {
        let mut cost = 0;
        match *change {
            Change::Paint(paint) => self.paint = paint,
            Change::Tires(tires) => self.tires = tires,
            Change::Motor(motor) => self.motor = motor,
            Change::Carbon(active) => self.carbon = active,
            Change::Buy(upgrade) => {
                if !self.owns(upgrade) {
                    if upgrade == Upgrade::Motor2 && !self.owns(Upgrade::Motor1) {
                        return Err("Buy motor stage 1 first".into());
                    }
                    cost = upgrade.price(self.model);
                    self.owned_upgrades |= upgrade.bit();
                }
                match upgrade {
                    Upgrade::SportTires => self.tires = Tires::Sport,
                    Upgrade::EfficiencyTires => self.tires = Tires::Efficiency,
                    Upgrade::Motor1 => self.motor = 1,
                    Upgrade::Motor2 => self.motor = 2,
                    Upgrade::Carbon => self.carbon = true,
                }
            }
        }
        self.validate()?;
        Ok(cost)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Change {
    Paint(Paint),
    Tires(Tires),
    Motor(u8),
    Carbon(bool),
    Buy(Upgrade),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GarageAction {
    Refresh { page: u32 },
    Buy(Model),
    Select(String),
    Configure { vehicle: String, change: Change },
    Sell(String),
    BuyListing(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GarageRequest {
    pub transaction: String,
    pub version: u32,
    pub action: GarageAction,
}
impl GarageRequest {
    pub fn new(action: GarageAction) -> Self {
        Self {
            transaction: crate::storage::RunId::random().0,
            version: CATALOG_VERSION,
            action,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GarageSnapshot {
    pub coins: u64,
    pub active: String,
    pub vehicles: Vec<Vehicle>,
    pub listings: Vec<Vehicle>,
    pub page: u32,
    pub more: bool,
}

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct ActiveVehicle(pub Vehicle);
impl Default for ActiveVehicle {
    fn default() -> Self {
        Self(Vehicle::new("preview".into(), Model::Corsa))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn price_example_and_all_upgrade_resale_cycles() {
        for model in MODELS {
            let mut car = Vehicle::new("test".into(), model);
            for upgrade in UPGRADES {
                let old_sale = car.sale_price();
                let cost = car.configure(&Change::Buy(upgrade)).unwrap();
                assert!(car.sale_price() <= old_sale + cost);
                assert!(car.sale_price() < car.market_price());
                assert!(car.market_price() < model.price() + car.upgrade_cost());
                assert_eq!(car.configure(&Change::Buy(upgrade)).unwrap(), 0);
            }
            let t = car.tuning();
            assert!(t.cruise_speed > 140. && t.boost_speed * 1.45 < 1000.);
            assert!(t.size.x < 140.);
        }
        let mut car = Vehicle::new("corsa".into(), Model::Corsa);
        car.configure(&Change::Buy(Upgrade::Motor1)).unwrap();
        car.configure(&Change::Buy(Upgrade::Carbon)).unwrap();
        assert_eq!((car.sale_price(), car.market_price()), (17595, 21793));
    }
    #[test]
    fn equipment_cannot_be_enabled_without_purchase() {
        let car = Vehicle::new("corsa".into(), Model::Corsa);
        for change in [
            Change::Motor(2),
            Change::Carbon(true),
            Change::Tires(Tires::Sport),
            Change::Buy(Upgrade::Motor2),
        ] {
            assert!(car.clone().configure(&change).is_err());
        }
        let mut car = car;
        car.configure(&Change::Buy(Upgrade::SportTires)).unwrap();
        let speed = car.tuning().steer_speed;
        for _ in 0..10 {
            car.configure(&Change::Buy(Upgrade::SportTires)).unwrap();
        }
        assert_eq!(car.tuning().steer_speed, speed);
    }
}
