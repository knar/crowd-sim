use std::f32::consts::SQRT_2;

use nannou::{
    color::{Srgba, rgba},
    glam::{IVec2, Vec2, ivec2, vec2},
};
use pather::Pather;
use slotmap::{DefaultKey, SlotMap};

use crate::{Settings, meshchunks::MeshChunks, tilemap::TileMap};

pub struct World {
    pub bots: SlotMap<DefaultKey, Bot>,
    pub debug_things: Vec<(DebugThing, Srgba<u8>)>,
    pub tilemap: TileMap,
    pather: Pather,
    repath_needed: bool,
    pub half_size: Vec2,
    pub mesh_chunks: MeshChunks,
}

impl World {
    pub fn new(size: IVec2) -> Self {
        let tilemap = TileMap::new(size);
        let mesh_chunks = MeshChunks::new(&tilemap, ivec2(8, 8));
        Self {
            bots: SlotMap::new(),
            debug_things: Vec::new(),
            tilemap,
            pather: Pather::new(math::ivec2(size.x, size.y)),
            repath_needed: true,
            half_size: vec2(size.x as f32, size.y as f32) / 2.0,
            mesh_chunks,
        }
    }

    pub fn tick(&mut self, settings: &Settings) {
        self.debug_things.clear();
        let dt = settings.timestep;

        self.repath_bots();
        self.stringpull_waypoints();

        let mut steer_forces = vec![];
        for i in self.bots.keys() {
            let bot = &self.bots[i];

            for pair in bot.waypoints.windows(2) {
                self.debug_things
                    .push((DebugThing::Arrow(pair[1], pair[0]), rgba(0, 255, 0, 255)));
            }

            let others = self.bots.keys().filter(|&k| k != i).map(|k| &self.bots[k]);
            let sep = bot.separation(others, settings);
            let seek = bot.seek(settings);
            let steer = sep + seek;
            steer_forces.push((i, steer));
        }

        for (k, accel) in steer_forces {
            let bot = &mut self.bots[k];
            bot.log_position();

            bot.velocity = (bot.velocity + accel * dt).clamp_length_max(bot.max_speed);
            bot.position += bot.velocity * dt;
            bot.position = self.tilemap.resolve_collisions(bot.position, bot.radius);

            if let Some(waypoint) = bot.waypoints.last()
                && (*waypoint - bot.position).length_squared() < 0.3_f32.powi(2)
            {
                bot.waypoints.pop();
            }

            if let Some(Task::Move(target)) = bot.tasks.first()
                && bot.velocity.length_squared() < 0.1_f32.powi(2)
                && (*target - bot.position).length_squared() < 0.1_f32.powi(2)
            {
                bot.tasks.remove(0);
            }
        }
    }

    fn repath_bots(&mut self) {
        for (_, bot) in self.bots.iter_mut() {
            let Some(Task::Move(target)) = bot.tasks.first() else {
                continue;
            };

            // repath only if a new target was assigned
            if !self.repath_needed && !bot.waypoints.is_empty() && bot.waypoints[0] == *target {
                continue;
            }

            // start and target in the pather's tile-space
            let s = self.tilemap.coord(bot.position);
            let s = math::ivec2(s.x, s.y);
            let t = math::vec2(target.x + self.half_size.x, target.y + self.half_size.y);

            // "backwards" path
            bot.waypoints = vec![*target];
            let raw_path: Vec<IVec2> = self
                .pather
                .path_to_pos(s, t)
                .into_iter()
                .map(|p| ivec2(p.x, p.y))
                .collect();
            bot.waypoints
                .extend(massage_waypoints(&self.tilemap, &raw_path));
        }
        self.repath_needed = false;
    }

    fn stringpull_waypoints(&mut self) {
        for (_, bot) in self.bots.iter_mut() {
            while bot.waypoints.len() > 1 {
                let next = bot.waypoints[bot.waypoints.len() - 2];
                if self.tilemap.line_of_sight(bot.position, next) {
                    bot.waypoints.pop();
                } else {
                    break;
                }
            }
        }
    }

    pub fn set_wall(&mut self, pos: Vec2, wall: bool) {
        if self.tilemap.set(pos, wall) {
            let tile = self.tilemap.coord(pos);
            self.pather.set(math::ivec2(tile.x, tile.y), !wall);
            self.repath_needed = true;
            self.mesh_chunks.mark_dirty(tile);
        }
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
}

#[derive(Debug)]
pub struct Bot {
    tasks: Vec<Task>,
    pub position: Vec2,
    pub velocity: Vec2,

    pub waypoints: Vec<Vec2>,

    pub radius: f32,
    pub max_speed: f32,
    max_accel: f32,

    pub trail: [Vec2; 20],
    trail_idx: usize,
}

#[derive(Debug)]
pub enum Task {
    Move(Vec2),
}

#[derive(Debug, Clone, Copy)]
pub enum DebugThing {
    Point(Vec2),
    Vec(Vec2, Vec2),
    Arrow(Vec2, Vec2),
}

impl Bot {
    fn new(pos: Vec2, vel: Vec2, task: Option<Task>) -> Bot {
        Bot {
            tasks: Vec::from_iter(task),
            position: pos,
            velocity: vel,
            waypoints: Vec::new(),
            radius: 0.2,
            max_speed: 4.0,
            max_accel: 80.0,
            trail: [pos; 20],
            trail_idx: 0,
        }
    }

    pub fn summary(&self) -> String {
        if let Some(task) = self.tasks.first() {
            match task {
                Task::Move(target) => format!(
                    "Task: Move({:?}), dist: {}\nvel mag: {}",
                    target,
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

    fn log_position(&mut self) {
        self.trail_idx = (self.trail_idx + 1) % self.trail.len();
        self.trail[self.trail_idx] = self.position;
    }

    fn seek(&self, settings: &Settings) -> Vec2 {
        let dt = settings.timestep;

        // let Some(Task::Move(target)) = self.tasks.first() else {
        let Some(target) = self.waypoints.last() else {
            // stopping force
            return -1.0 / settings.stopping_time * self.velocity;
        };

        let x1 = self.max_speed.powi(2) / (2.0 * self.max_accel) * 2.0;
        let d = *target - self.position;
        let target_vel = (d / x1).clamp_length_max(1.0) * self.max_speed;
        ((target_vel - self.velocity) / dt).clamp_length_max(self.max_accel)
    }

    fn separation<'a>(&self, others: impl Iterator<Item = &'a Bot>, settings: &Settings) -> Vec2 {
        others
            .map(|other| {
                let d = self.position - other.position;
                let r0 = self.radius + other.radius;
                let r1 = r0 + settings.spring_distance;
                if d.length_squared() < r1 * r1 {
                    if d.length_squared() > 1.0 / 1024.0 {
                        let x = (d.length() - r0) / settings.spring_distance;
                        let f = (1.0 - x) * settings.spring_constant;
                        d / d.length() * f
                    } else {
                        panic!("perfectly overlapping!");
                    }
                } else {
                    Vec2::ZERO
                }
            })
            .fold(Vec2::ZERO, |acc, f| acc + f)
    }
}

fn massage_waypoints(tilemap: &TileMap, raw_path: &[IVec2]) -> Vec<Vec2> {
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
                if tilemap[cur + ivec2(d.x, 0)] {
                    let offset = vec2((1.0 - SQRT_2 * 0.5) * d.x as f32, SQRT_2 * 0.5 * d.y as f32);
                    path.push(pos + offset);
                } else if tilemap[cur + ivec2(0, d.y)] {
                    let offset = vec2(SQRT_2 * 0.5 * d.x as f32, (1.0 - SQRT_2 * 0.5) * d.y as f32);
                    path.push(pos + offset);
                }
            }
            cur += d;
        }
    }
    path.push(tilemap.tile_center(last));
    path
}
