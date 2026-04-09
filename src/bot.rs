use crate::tilemap::TileMap;
use nannou::glam::{IVec2, Vec2, ivec2, vec2};
use std::f32::consts::SQRT_2;

#[derive(Debug)]
pub struct Bot {
    pub tasks: Vec<Task>,
    pub position: Vec2,
    pub velocity: Vec2,

    pub waypoints: Vec<Vec2>,

    pub radius: f32,
    pub max_speed: f32,
    pub max_accel: f32,

    pub trail: [Vec2; 20],
    pub trail_idx: usize,

    pub last_target: Option<Vec2>,

    pub debug_accel: Vec2,
    pub debug_sep: Vec2,
    pub debug_seek: Vec2,
    pub debug_friction: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Task {
    Move(Vec2),
}

impl Bot {
    pub fn new(pos: Vec2, vel: Vec2, task: Option<Task>) -> Bot {
        Bot {
            tasks: Vec::from_iter(task),
            position: pos,
            velocity: vel,
            waypoints: Vec::new(),
            radius: 0.2,
            max_speed: 6.0,
            max_accel: 60.0,
            trail: [pos; 20],
            trail_idx: 0,
            last_target: None,
            debug_accel: Vec2::ZERO,
            debug_sep: Vec2::ZERO,
            debug_seek: Vec2::ZERO,
            debug_friction: Vec2::ZERO,
        }
    }

    pub fn summary(&self) -> String {
        if let Some(task) = self.tasks.first() {
            match task {
                Task::Move(target) => format!(
                    "Task: Move({:.2},{:.2}), dist: {:.2}\nvel mag: {:.2}",
                    target.x,
                    target.y,
                    (*target - self.position).length(),
                    self.velocity.length()
                ),
            }
        } else {
            format!("Task: None\nvel mag: {}", self.velocity.length())
        }
    }

    pub fn prev_pos(&self) -> Vec2 {
        self.trail[self.trail_idx]
    }

    pub fn prev_prev_pos(&self) -> Vec2 {
        let prev_idx = (self.trail.len() + self.trail_idx - 1) % self.trail.len();
        self.trail[prev_idx]
    }

    pub fn log_position(&mut self) {
        self.trail_idx = (self.trail_idx + 1) % self.trail.len();
        self.trail[self.trail_idx] = self.position;
    }
}

pub fn massage_waypoints(tilemap: &TileMap, raw_path: &[IVec2], r: f32) -> Vec<Vec2> {
    let Some(&last) = raw_path.last() else {
        return vec![];
    };
    let mut path = vec![];
    let mut cur = raw_path[0];
    for w in raw_path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let d = (b - a).signum();
        while cur != b {
            path.push(tilemap.tile_center(cur));
            // if diagagonal step around a corner, add point to go wider
            if d.x != 0 && d.y != 0 {
                let pos = tilemap.tile_center(cur);
                let w = (0.5 + r.max(0.21)) / SQRT_2;
                if tilemap[cur + ivec2(d.x, 0)] {
                    let offset = vec2((1.0 - w) * d.x as f32, w * d.y as f32);
                    path.push(pos + offset);
                } else if tilemap[cur + ivec2(0, d.y)] {
                    let offset = vec2(w * d.x as f32, (1.0 - w) * d.y as f32);
                    path.push(pos + offset);
                }
            }
            cur += d;
        }
    }
    path.push(tilemap.tile_center(last));
    path
}
