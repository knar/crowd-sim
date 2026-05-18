use nannou::{
    Draw,
    color::{BLUE, CYAN, GREEN, ORANGE, PINK, RED, WHITE, YELLOW},
    glam::{Vec2, vec2},
    math::{Vec2Angle, Vec2Rotate},
    rand::rngs::SmallRng,
};
use slotmap::DefaultKey;

use crate::{
    DebugThing, Settings,
    steer::basic::target_velocity,
    world::{World, bot::Bot},
};

#[derive(Debug)]
struct Cone {
    origin: Vec2,
    left: Vec2,
    right: Vec2,
}

pub fn vo_velocity(
    world: &World,
    i: DefaultKey,
    settings: &Settings,
    rng: &mut SmallRng,
    selection: &[DefaultKey],
    debug_things: &mut Vec<DebugThing>,
) -> Vec2 {
    let target_vel = target_velocity(world, i, settings, rng, selection, debug_things);
    if target_vel == Vec2::ZERO {
        return target_vel;
    }
    // if !selection.contains(&i) {
    //     return target_vel;
    // }

    let bot = &world.bots[i];

    if selection.contains(&i) {
        debug_things.push(DebugThing::Circle(bot.position, target_vel.length(), GREEN));
    }

    let mut vo_angles = vec![];
    let neighbors = world
        .grid
        .query(bot.position, settings.vo_neighbor_radius)
        .filter(|&j| j != i)
        .filter(|&j| {
            bot.position.distance_squared(world.bots[j].position)
                <= settings.vo_neighbor_radius.powi(2)
        })
        .map(|j| &world.bots[j]);
    let debug_origin = selection.contains(&i).then_some(bot.position);
    for other in neighbors {
        let vo = vo(bot, other, debug_origin, debug_things);
        let intervals = vo_angle_ranges(target_vel, vo, debug_origin, debug_things);
        for (left, right) in [intervals.0, intervals.1].into_iter().flatten() {
            if selection.contains(&i) {
                let theta = target_vel.angle();
                debug_things.push(DebugThing::Arc(
                    bot.position,
                    target_vel.length(),
                    theta + right,
                    theta + left,
                    RED,
                ));
            }
            // right to left since counterclockwise
            vo_angles.push((right, left));
        }
    }

    // debug_things.push(DebugThing::Text(format!("{:?}", &vo_angles)));
    vo_angles.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    for i in (1..vo_angles.len()).rev() {
        if vo_angles[i].0 <= vo_angles[i - 1].1 {
            vo_angles[i - 1].1 = vo_angles[i - 1].1.max(vo_angles[i].1);
            vo_angles.pop();
        }
    }
    if selection.contains(&i) {
        debug_things.push(DebugThing::Text(format!("{:?}", &vo_angles)));
    }

    let mut target_vel = target_vel;
    for (a, b) in vo_angles {
        if a < 0.0 && 0.0 < b {
            if -a < b {
                target_vel = target_vel.rotate(a * settings.vo_response);
            } else {
                target_vel = target_vel.rotate(b * settings.vo_response);
            }
        }
    }

    if selection.contains(&i) {
        debug_things.push(DebugThing::Vec(bot.position, target_vel, WHITE));
    }
    target_vel
}

fn vo(a: &Bot, b: &Bot, debug_origin: Option<Vec2>, debug_things: &mut Vec<DebugThing>) -> Cone {
    let d = b.position - a.position;
    let r = a.radius + b.radius;
    let theta = (r / d.length()).asin();
    let angle = d.angle();
    let [left, right] = [angle + theta, angle - theta].map(|a| vec2(a.cos(), a.sin()));

    if let Some(o) = debug_origin {
        debug_things.push(DebugThing::Vec(o + b.velocity, left * 100.0, CYAN));
        debug_things.push(DebugThing::Vec(o + b.velocity, right * 100.0, PINK));
    }

    Cone {
        origin: b.velocity,
        left,
        right,
    }
}

fn vo_angle_ranges(
    target_vel: Vec2,
    vo: Cone,
    _debug_origin: Option<Vec2>,
    _debug_things: &mut Vec<DebugThing>,
) -> (Option<(f32, f32)>, Option<(f32, f32)>) {
    let r = target_vel.length();
    let left = intersect_circle_ray(r, vo.origin, vo.left);
    let right = intersect_circle_ray(r, vo.origin, vo.right);

    // TODO: are these are not correct?
    // need to draw arcs or something to really see the intervals..
    match (left, right) {
        ((Some(a), Some(b)), (Some(c), Some(d))) => {
            let a = target_vel.angle_between(a);
            let b = target_vel.angle_between(b);
            let c = target_vel.angle_between(c);
            let d = target_vel.angle_between(d);
            (Some((a, c)), Some((b, d)))
        }
        ((Some(a), Some(b)), (None, None)) => {
            let a = target_vel.angle_between(a);
            let b = target_vel.angle_between(b);
            (Some((a, b)), None)
        }
        ((None, None), (Some(a), Some(b))) => {
            let a = target_vel.angle_between(a);
            let b = target_vel.angle_between(b);
            (Some((b, a)), None)
        }
        ((None, Some(a)), (None, Some(b)))
        | ((None, Some(a)), (Some(_), Some(b)))
        | ((Some(_), Some(a)), (None, Some(b))) => {
            let a = target_vel.angle_between(a);
            let b = target_vel.angle_between(b);
            (Some((a, b)), None)
        }
        _ => (None, None),
    }
}

fn intersect_circle_ray(r: f32, o: Vec2, d: Vec2) -> (Option<Vec2>, Option<Vec2>) {
    let a = d.dot(d);
    let b = 2.0 * o.dot(d);
    let c = o.dot(o) - r.powi(2);
    let discr = b.powi(2) - 4.0 * a * c;
    if discr < 0.0 {
        return (None, None);
    }
    let discr_sqrt = discr.sqrt();
    let t0 = (-b - discr_sqrt) / (2.0 * a);
    let t1 = (-b + discr_sqrt) / (2.0 * a);
    (
        (t0 >= 0.0).then(|| o + d * t0),
        (t1 >= 0.0).then(|| o + d * t1),
    )
}
