mod basic;
mod orca;
mod ttc;

use std::fmt::Display;

use nannou::{glam::Vec2, rand::rngs::SmallRng};
use slotmap::DefaultKey;

use crate::{
    Settings,
    steer::{basic::target_velocity, orca::orca_velocity, ttc::ttc_velocity},
    world::World,
};

#[derive(PartialEq)]
pub enum SteeringStrategy {
    Basic,
    Orca,
    Ttc,
}

impl SteeringStrategy {
    pub fn steer_fn(
        &self,
        world: &World,
        k: DefaultKey,
        settings: &Settings,
        rng: &mut SmallRng,
    ) -> Vec2 {
        match self {
            SteeringStrategy::Basic => target_velocity(world, k, settings, rng),
            SteeringStrategy::Orca => orca_velocity(world, k, settings, rng),
            SteeringStrategy::Ttc => ttc_velocity(world, k, settings, rng),
        }
    }
}

impl Display for SteeringStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SteeringStrategy::Basic => write!(f, "Basic"),
            SteeringStrategy::Orca => write!(f, "ORCA"),
            SteeringStrategy::Ttc => write!(f, "TTC"),
        }
    }
}
