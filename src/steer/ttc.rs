use nannou::{glam::Vec2, rand::rngs::SmallRng};
use slotmap::{DefaultKey, SlotMap};

use crate::{
    DebugThing, Settings,
    steer::basic::target_velocity,
    world::{World, bot::Bot},
};

// https://www.gameaipro.com/GameAIPro2/GameAIPro2_Chapter19_Guide_to_Anticipatory_Collision_Avoidance.pdf
// pretty garb

pub fn ttc_velocity(
    world: &World,
    i: DefaultKey,
    settings: &Settings,
    rng: &mut SmallRng,
    selection: &[DefaultKey],
    debug_things: &mut Vec<DebugThing>,
) -> Vec2 {
    let target_vel = target_velocity(world, i, settings, rng, selection, debug_things);
    let bot = &world.bots[i];

    let mut f = 2.0 * (target_vel - bot.velocity);

    let query_radius = 2.0 * bot.radius + 12.0 * settings.ttc_time_horizon;
    for j in world.grid.query(bot.position, query_radius) {
        let t = ttc(&world.bots, i, j);
        let other = &world.bots[j];

        // force direction
        let f_avoid = (bot.position + bot.velocity * t - other.position - other.velocity * t)
            .normalize_or_zero();

        // force mag
        let mag = if t >= 0.0 && t <= settings.ttc_time_horizon {
            (settings.ttc_time_horizon - t) / (t + 0.001)
        } else {
            0.0
        }
        .min(bot.max_accel);

        f += f_avoid * mag * 10.0;
    }

    target_vel + f * settings.timestep
}

fn ttc(bots: &SlotMap<DefaultKey, Bot>, i: DefaultKey, j: DefaultKey) -> f32 {
    let i = &bots[i];
    let j = &bots[j];
    let r = i.radius + j.radius;
    let w = j.position - i.position;
    let c = w.dot(w) - r.powi(2);
    if c < 0.0 {
        return 0.0;
    }
    let v = i.velocity - j.velocity;
    let a = v.dot(v);
    let b = w.dot(v);
    let discr = b.powi(2) - a * c;
    if discr <= 0.0 {
        return f32::INFINITY;
    }
    let tau = (b - discr.sqrt()) / a;
    if tau < 0.0 {
        return f32::INFINITY;
    }
    tau
}
