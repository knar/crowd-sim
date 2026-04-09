use nannou::glam::{IVec2, Vec2, ivec2, vec2};
use pather::Pather;
use slotmap::{DefaultKey, SlotMap};

use crate::{
    Settings,
    bot::{Bot, Task, massage_waypoints},
    meshchunks::MeshChunks,
    spatialgrid::SpatialGrid,
    tilemap::TileMap,
};

pub struct World {
    pub bots: SlotMap<DefaultKey, Bot>,
    pub tilemap: TileMap,
    pather: Pather,
    repath_needed: bool,
    pub half_size: Vec2,
    pub mesh_chunks: MeshChunks,
    grid: SpatialGrid,
}

struct SteeringResult {
    acceleration: Vec2,
    sep: Vec2,
    seek: Vec2,
    friction: Vec2,
}

impl World {
    pub fn new(size: IVec2) -> Self {
        let tilemap = TileMap::new(size);
        let mesh_chunks = MeshChunks::new(&tilemap, ivec2(8, 8));
        let half_size = vec2(size.x as f32, size.y as f32) / 2.0;
        Self {
            bots: SlotMap::new(),
            tilemap,
            pather: Pather::new(math::ivec2(size.x, size.y)),
            repath_needed: true,
            half_size,
            mesh_chunks,
            grid: SpatialGrid::new(size, 2.0, half_size),
        }
    }

    pub fn tick(&mut self, settings: &Settings) {
        let dt = settings.timestep;

        self.repath_bots();
        self.stringpull_waypoints();
        self.repop_spatial_grid();
        self.apply_arrivals(settings);

        let steer_results: Vec<(DefaultKey, SteeringResult)> = self
            .bots
            .keys()
            .map(|i| (i, self.compute_steering(i, settings, dt)))
            .collect();

        for (k, res) in steer_results {
            let bot = &mut self.bots[k];
            bot.debug_accel = res.acceleration;
            bot.debug_seek = res.seek;
            bot.debug_sep = res.sep;
            bot.debug_friction = res.friction;

            self.integrate_bot(k, res.acceleration, settings, dt);
        }
    }

    fn repath_bots(&mut self) {
        for (_, bot) in self.bots.iter_mut() {
            let Some(Task::Move(target)) = bot.tasks.first() else {
                continue;
            };

            if !self.repath_needed && !bot.waypoints.is_empty() && bot.waypoints[0] == *target {
                continue;
            }

            let s = self.tilemap.coord(bot.position);
            let s = math::ivec2(s.x, s.y);
            let t = math::vec2(target.x + self.half_size.x, target.y + self.half_size.y);

            bot.waypoints = vec![*target];
            let raw_path: Vec<IVec2> = self
                .pather
                .path_to_pos(s, t)
                .into_iter()
                .map(|p| ivec2(p.x, p.y))
                .collect();
            bot.waypoints
                .extend(massage_waypoints(&self.tilemap, &raw_path, bot.radius));
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

    fn repop_spatial_grid(&mut self) {
        self.grid.clear();
        for (k, bot) in &self.bots {
            self.grid.insert(bot.position, k);
        }
    }

    fn apply_arrivals(&mut self, settings: &Settings) {
        let mut arrivals = vec![];
        for (i, bot) in &self.bots {
            let Some(Task::Move(target)) = bot.tasks.first() else {
                continue;
            };

            // direct arrival
            let dist_sq = bot.position.distance_squared(*target);
            if dist_sq < settings.arrival_distance.powi(2) {
                arrivals.push(i);
                continue;
            }

            // proxy arrival
            // let query_radius = bot.radius + 0.5 + settings.spring_distance;
            // let arrived_by_proxy = self
            //     .grid
            //     .query(bot.position, query_radius)
            //     .iter()
            //     .any(|&j| {
            //         if i == j {
            //             return false;
            //         }
            //         let other = &self.bots[j];
            //         // if !other.tasks.is_empty() {
            //         //     return false;
            //         // }
            //         if other.last_target != Some(*target) {
            //             return false;
            //         }
            //         // if we are within the arrived bot's spring zone
            //         let dist_sq = bot.position.distance_squared(other.position);
            //         let r_sq = (bot.radius + other.radius + settings.spring_distance).powi(2);
            //         dist_sq < r_sq
            //     });
            //
            // if arrived_by_proxy {
            //     arrivals.push(i);
            // }
        }
        for i in arrivals {
            let bot = &mut self.bots[i];
            if let Some(Task::Move(target)) = bot.tasks.first() {
                bot.last_target = Some(*target);
            }
            bot.tasks.remove(0);
            bot.waypoints.clear();
        }
    }

    fn compute_steering(&self, i: DefaultKey, settings: &Settings, dt: f32) -> SteeringResult {
        let bot = &self.bots[i];

        let seek = bot.waypoints.last().map_or(Vec2::ZERO, |target| {
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
            let v_target = dir * v_target_mag;
            let a_req = (v_target - bot.velocity) / dt;
            a_req.clamp_length_max(bot.max_accel)
        });

        let query_radius = bot.radius + 0.5 + settings.spring_distance;
        let neighbors = self.grid.query(bot.position, query_radius);
        let others = neighbors
            .iter()
            .filter(|&&k| k != i)
            .map(|&k| (k, &self.bots[k]));
        let sep = others
            .map(|(o, other)| {
                // let mul = if bot.tasks.is_empty() == other.tasks.is_empty() {
                //     1.0
                // } else if bot.tasks.is_empty() {
                //     2.0
                // } else {
                //     0.0
                // };

                let d = bot.position - other.position;
                let r0 = bot.radius + other.radius;
                let r1 = r0 + settings.spring_distance;
                if d.length_squared() < r1 * r1 {
                    if d.length_squared() > 1.0 / 1024.0 {
                        let dist = d.length();
                        let dir = d / dist;
                        let x = (dist - r0) / settings.spring_distance;
                        let f_spring = (1.0 - x) * settings.spring_constant;

                        let v_rel = bot.velocity - other.velocity;
                        let approach_speed = v_rel.dot(dir);
                        let f_damping = -settings.damping_constant * approach_speed;

                        dir * (f_spring + f_damping)
                    } else if i < o {
                        vec2(1.0, 0.0) * settings.spring_constant
                    } else {
                        vec2(-1.0, 0.0) * settings.spring_constant
                    }
                } else {
                    Vec2::ZERO
                }
            })
            .fold(Vec2::ZERO, |acc, f| acc + f);

        let mut steer = sep + seek;
        let mut friction = Vec2::ZERO;
        if bot.waypoints.is_empty() {
            friction = (-bot.velocity / dt).clamp_length_max(bot.max_accel);
            steer += friction;
        }

        SteeringResult {
            acceleration: steer.clamp_length_max(bot.max_accel),
            sep,
            seek,
            friction,
        }
    }

    fn integrate_bot(&mut self, k: DefaultKey, accel: Vec2, _settings: &Settings, dt: f32) {
        let bot = &mut self.bots[k];
        bot.log_position();

        let vel = (bot.velocity + accel * dt).clamp_length_max(bot.max_speed);
        let pos = bot.position + vel * dt;
        let old_pos = bot.position;
        bot.position = self.tilemap.resolve_collisions(pos, bot.radius);
        bot.velocity = (bot.position - old_pos) / dt;
        if bot.velocity.length_squared() < 0.1f32.powi(2) {
            bot.velocity = Vec2::ZERO;
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

    pub fn delete_bot(&mut self, k: DefaultKey) {
        self.bots.remove(k);
    }

    pub fn set_bot_task(&mut self, bot_key: DefaultKey, task: Task) {
        self.bots[bot_key].tasks = vec![task];
    }

    pub fn add_bot_task(&mut self, bot_key: DefaultKey, task: Task) {
        self.bots[bot_key].tasks.push(task);
    }
}
