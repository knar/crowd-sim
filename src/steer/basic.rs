use nannou::{
    glam::{Vec2, vec2},
    rand::{Rng, rngs::SmallRng},
};
use slotmap::DefaultKey;

use crate::{Settings, world::World};

pub fn target_velocity(
    world: &World,
    k: DefaultKey,
    settings: &Settings,
    rng: &mut SmallRng,
) -> Vec2 {
    let dt = settings.timestep;
    let bot = &world.bots[k];

    let ideal_vel = if let Some(target) = bot.waypoints.last()
        && *target != bot.position
    {
        let offset = *target - bot.position;
        let d = offset.length();
        let dir = offset / d;
        let mut v_target_mag = bot.max_speed;
        if bot.tasks.len() <= 1 {
            let braking_threshold =
                (bot.max_speed * bot.max_speed) / (2.0 * bot.max_accel) + (bot.max_speed * dt);
            if d <= braking_threshold {
                let a_dt_sq = bot.max_accel * dt * dt;
                let p_val = 0.5 * (1.0 + (8.0 * d) / a_dt_sq).sqrt();
                let k = (p_val - 0.5).ceil().max(1.0);
                let d_prev = (k * (k - 1.0) / 2.0) * a_dt_sq;

                v_target_mag = (k - 1.0) * bot.max_accel * dt + (d - d_prev) / (k * dt);
                if v_target_mag > bot.max_speed {
                    v_target_mag = bot.max_speed;
                }
            }
        }

        let noise_x = rng.gen_range(-0.001..=0.001);
        let noise_y = rng.gen_range(-0.001..=0.001);
        dir * v_target_mag + vec2(noise_x, noise_y)
    } else {
        Vec2::ZERO
    };

    let dv = ideal_vel - bot.velocity;
    bot.velocity + dv.clamp_length_max(bot.max_accel * dt)
}
