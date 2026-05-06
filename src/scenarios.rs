use nannou::{
    glam::{Vec2, vec2},
    rand::{Rng, SeedableRng, rngs::SmallRng},
};

use crate::{Model, world::bot::Task};

pub fn scenario_simple_avoid(m: &mut Model) {
    m.reset_world();
    let k = m.world.add_bot(
        vec2(-3.0, 0.0),
        Vec2::ZERO,
        Some(Task::Move(vec2(3.0, 0.0))),
    );
    m.world.add_bot(vec2(0.0, 0.2), Vec2::ZERO, None);

    m.client.selection = vec![k];
}

pub fn scenario_symmetry_avoid(m: &mut Model) {
    m.reset_world();
    let pos = vec2(-3.0, 0.0);
    m.world.add_bot(pos, Vec2::ZERO, Some(Task::Move(-pos)));
    m.world.add_bot(-pos, Vec2::ZERO, Some(Task::Move(pos)));
}

pub fn scenario_symmetry_avoid2(m: &mut Model) {
    m.reset_world();
    let pos = vec2(-3.0, 0.0);
    let dy = vec2(0.0, 0.4);
    let k = m.world.add_bot(pos, Vec2::ZERO, Some(Task::Move(-pos)));
    m.world
        .add_bot(-pos + dy, Vec2::ZERO, Some(Task::Move(pos)));
    m.world
        .add_bot(-pos - dy, Vec2::ZERO, Some(Task::Move(pos)));

    m.client.selection = vec![k];
}

pub fn scenario_origin_swap_n(m: &mut Model, n: usize, r: f32) {
    m.reset_world();
    let mut rng = SmallRng::seed_from_u64(0);
    for _ in 0..n {
        let pos = vec2(rng.gen_range(-r..r), rng.gen_range(-r..r));
        m.world.add_bot(pos, Vec2::ZERO, Some(Task::Move(-pos)));
    }
}

pub fn scenario_lines_swap_n(m: &mut Model, n: usize) {
    m.reset_world();
    let mut rng = SmallRng::seed_from_u64(0);
    for _ in 0..n / 2 {
        let a = vec2(rng.gen_range(-10.0..-8.0), rng.gen_range(-10.0..10.0));
        let b = vec2(rng.gen_range(8.0..10.0), rng.gen_range(-10.0..10.0));
        m.world
            .add_bot(a, Vec2::ZERO, Some(Task::Move(vec2(-a.x, a.y))));
        m.world
            .add_bot(b, Vec2::ZERO, Some(Task::Move(vec2(-b.x, b.y))));
    }
}
