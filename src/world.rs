use nannou::{
    Draw,
    color::{Srgba, rgba, rgba8},
    glam::Vec2,
};
use slotmap::{DefaultKey, SlotMap};

use crate::{Settings, lerp, theme};

pub struct World {
    pub bots: SlotMap<DefaultKey, Bot>,
    pub debug_things: Vec<(DebugThing, Srgba<u8>)>,
}

#[derive(Debug)]
pub struct Bot {
    tasks: Vec<Task>,

    position: Vec2,
    velocity: Vec2,

    radius: f32,
    max_speed: f32,
    max_accel: f32,

    trail: [Vec2; 20],
    trail_idx: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum DebugThing {
    Point(Vec2),
    Vec(Vec2, Vec2),
}

impl Bot {
    fn new(pos: Vec2, vel: Vec2, task: Option<Task>) -> Bot {
        Bot {
            tasks: Vec::from_iter(task),
            position: pos,
            velocity: vel,
            radius: 0.2,
            max_speed: 6.0,
            max_accel: 30.0,
            trail: [pos; 20],
            trail_idx: 0,
        }
    }

    pub fn summary(&self) -> String {
        if let Some(task) = self.tasks.first() {
            match task {
                Task::Move(target) => format!(
                    "vel mag: {}\ntarget dist: {}",
                    self.velocity.length(),
                    (*target - self.position).length()
                ),
            }
        } else {
            format!("vel mag: {}", self.velocity.length())
        }
    }

    pub fn draw(&self, draw: &Draw, accumulator: f32, settings: &Settings, selected: bool) {
        let pos = if !settings.paused && settings.interpolate_frames {
            self.lerped_pos(accumulator / settings.timestep)
        } else {
            self.position
        };

        if settings.draw_trail {
            for &p in &self.trail {
                draw.ellipse()
                    .xy(p)
                    .radius(self.radius * 0.2)
                    .resolution(16.0)
                    .color(rgba8(0xe7, 0xdf, 0xdb, 0x20));
            }
        }

        draw.ellipse()
            .xy(pos)
            .radius(self.radius - 0.02)
            .resolution(64.0)
            .stroke(theme::fg())
            .stroke_weight(0.04)
            .no_fill();

        // interpolated vel
        let lean = lerp(
            self.prev_velocity(settings),
            self.velocity,
            accumulator / settings.timestep,
        ) / self.max_speed;

        draw.ellipse()
            .xy(pos + lean * self.radius * 0.6)
            .radius(self.radius * 0.4)
            .resolution(64.0)
            .color(theme::fg());

        if selected {
            draw.ellipse()
                .xy(pos)
                .radius(self.radius)
                .resolution(64.0)
                .stroke(rgba(0.4, 0.8, 0.4, 1.0))
                .stroke_weight(0.03)
                .no_fill();
        }
    }

    fn lerped_pos(&self, frac: f32) -> Vec2 {
        let last_pos = self.trail[self.trail_idx];
        lerp(last_pos, self.position, frac)
    }

    fn prev_velocity(&self, settings: &Settings) -> Vec2 {
        let prev_idx = (self.trail.len() + self.trail_idx - 1) % self.trail.len();
        (self.trail[self.trail_idx] - self.trail[prev_idx]) / settings.timestep
    }

    fn log_position(&mut self) {
        self.trail_idx = (self.trail_idx + 1) % self.trail.len();
        self.trail[self.trail_idx] = self.position;
    }

    fn seek_force(&self, dt: f32) -> Vec2 {
        let Some(Task::Move(target)) = self.tasks.first() else {
            return (-self.velocity / dt) * 0.5;
        };

        let d = *target - self.position;
        let dist = d.length();

        if dist < 0.1_f32.powi(2) {
            return -self.velocity / dt;
        }

        let max_snap_dist = self.max_accel * dt * dt;
        let ideal_speed = if dist <= max_snap_dist {
            dist / dt
        } else {
            let safe_accel = self.max_accel * 0.90;
            let a_dt = safe_accel * dt;
            let inner = (a_dt * a_dt) + (8.0 * safe_accel * dist);
            let v_optimal = (inner.sqrt() - a_dt) / 2.0;
            v_optimal.min(self.max_speed)
        };

        let desired_vel = ((d / dist) * ideal_speed).clamp_length_max(self.max_speed);
        ((desired_vel - self.velocity) / dt).clamp_length_max(self.max_accel)
    }

    fn separation_force<'a>(&self, others: impl Iterator<Item = &'a Bot>, dt: f32) -> Vec2 {
        others
            .map(|other| {
                let d = self.position - other.position;
                if d.length_squared() < (self.radius + other.radius).powi(2) {
                    let half_overlap = (self.radius + other.radius - d.length()) / 2.0;
                    d.normalize() * half_overlap / (dt * dt)
                } else {
                    Vec2::ZERO
                }
            })
            .fold(Vec2::ZERO, |acc, f| acc + f)
    }
}

#[derive(Debug)]
pub enum Task {
    Move(Vec2),
}

impl World {
    pub fn new() -> Self {
        Self {
            bots: SlotMap::new(),
            debug_things: Vec::new(),
        }
    }

    pub fn bots_in_rect(&self, corner1: Vec2, corner2: Vec2, frac: f32) -> Vec<DefaultKey> {
        let rect_center = corner1 + (corner2 - corner1) / 2.0;
        let rect_size = (corner1 - corner2).abs();
        self.bots
            .keys()
            .filter(|&k| {
                circle_rect_intersects(
                    self.bots[k].lerped_pos(frac),
                    self.bots[k].radius,
                    rect_center,
                    rect_size,
                )
            })
            .collect()
    }

    pub fn add_bot(&mut self, pos: Vec2, vel: Vec2, task: Option<Task>) {
        self.bots.insert(Bot::new(pos, vel, task));
    }

    pub fn set_bot_task(&mut self, bot_key: DefaultKey, task: Task) {
        self.bots[bot_key].tasks = vec![task];
    }

    pub fn add_bot_task(&mut self, bot_key: DefaultKey, task: Task) {
        self.bots[bot_key].tasks.push(task);
    }

    pub fn tick(&mut self, settings: &Settings) {
        self.debug_things.clear();
        let dt = settings.timestep;

        let mut steer_forces = vec![];
        for i in self.bots.keys() {
            let bot = &self.bots[i];

            let seek = self.bots[i].seek_force(dt);

            let others = self.bots.keys().filter(|&k| k != i).map(|k| &self.bots[k]);
            let sep = self.bots[i].separation_force(others, dt);

            let steer = (seek + sep).clamp_length_max(bot.max_accel);
            steer_forces.push((i, steer));
        }

        for (k, accel) in steer_forces {
            let bot = &mut self.bots[k];
            bot.log_position();

            if let Some(Task::Move(target)) = bot.tasks.first()
                && bot.velocity.length_squared() < 0.1_f32.powi(2)
                && (*target - bot.position).length_squared() < 0.1_f32.powi(2)
            {
                bot.tasks.remove(0);
            }

            bot.velocity = (bot.velocity + accel * dt).clamp_length_max(bot.max_speed);
            bot.position += bot.velocity * dt;
        }
    }
}

fn circle_rect_intersects(
    circle_center: Vec2,
    circle_radius: f32,
    rect_center: Vec2,
    rect_size: Vec2,
) -> bool {
    let half_size = rect_size / 2.0;
    let d = (circle_center - rect_center).abs();
    if d.x > half_size.x + circle_radius || d.y > half_size.y + circle_radius {
        return false;
    }
    if d.x < half_size.x || d.y < half_size.y {
        return true;
    }
    let corner_dist_sq = d.distance_squared(half_size);
    corner_dist_sq <= circle_radius * circle_radius
}

// let desired_vel = (d / dt).clamp_length_max(bot.max_speed);
// let steer =
//     ((desired_vel - bot.velocity) / dt).clamp_length_max(bot.max_accel);
// let time_to_stop = bot.velocity.length() / bot.max_accel; // vf = vi + at
// let dist_to_stop = ((bot.velocity / 2.0) * time_to_stop).length();
// println!(
//     "{:.2}\t{:.2}\t{:.2}\t{:.2}s\t{:.2}",
//     // steer.x,
//     // bot.velocity.x,
//     // offset.x,
//     steer.length(),
//     bot.velocity.length(),
//     offset.length(),
//     time_to_stop,
//     dist_to_stop
// );
// self.debug_things.push((
//     DebugThing::Vec(bot.position, desired_vel * dt),
//     rgba(255, 255, 0, 200),
// ));
// self.debug_things.push((
//     DebugThing::Vec(bot.position, steer),
//     rgba(0, 255, 255, 128),
// ));
