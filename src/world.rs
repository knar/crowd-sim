use nannou::{
    glam::{IVec2, Vec2, ivec2, vec2},
    rand::{Rng, rngs::SmallRng},
};
use pather::Pather;
use slotmap::{DefaultKey, SlotMap};

use crate::{
    Settings,
    bot::{Bot, Task, massage_waypoints},
    meshchunks::MeshChunks,
    orca::{OptimizationGoal, linear_program_2, linear_program_3},
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

    pub fn tick(&mut self, settings: &Settings, rng: &mut SmallRng) {
        for (_k, bot) in &mut self.bots {
            bot.log_position();
        }

        self.repath_bots();
        self.stringpull_waypoints();

        self.repop_spatial_grid();
        self.apply_arrivals(settings);

        let new_vels: Vec<_> = self
            .grid
            .iter_keys()
            .map(|i| {
                let vel = if settings.use_orca {
                    self.compute_orca_velocity(i, settings, rng)
                } else {
                    self.compute_target_velocity(i, settings, rng)
                };
                (i, vel)
            })
            .collect();

        for (k, vel) in new_vels {
            let bot = &mut self.bots[k];

            bot.velocity = vel.clamp_length_max(bot.max_speed);
            if bot.waypoints.is_empty() && bot.velocity.length_squared() < 0.001 {
                bot.velocity = Vec2::ZERO;
            }

            bot.position += bot.velocity * settings.timestep;
        }

        self.repop_spatial_grid();
        for _ in 0..settings.collision_resolver_iters {
            self.resolve_bot_bot_collisions(settings);
            self.resolve_bot_wall_collisions();
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
        for i in self.grid.iter_keys() {
            let bot = &self.bots[i];
            let Some(Task::Move(target)) = bot.tasks.first() else {
                self.bots[i].debug_arrival_dist = 0.0;
                continue;
            };

            let dist_sq = bot.position.distance_squared(*target);

            let n_overlapping = self
                .grid
                .query(bot.position, bot.radius + 0.5)
                .filter(|&j| i != j)
                .filter(|&j| {
                    bot.position.distance_squared(self.bots[j].position)
                        <= (bot.radius + self.bots[j].radius + 0.05).powi(2)
                })
                .count();
            let arrival_distance = if n_overlapping > 0 {
                settings.arrival_distance.max(bot.radius) * (1.0 + 0.2 * n_overlapping as f32)
            } else {
                settings.arrival_distance
            };

            self.bots[i].debug_arrival_dist = arrival_distance;

            if dist_sq < arrival_distance.powi(2) {
                arrivals.push(i);
            }
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

    fn compute_orca_velocity(
        &self,
        i: DefaultKey,
        settings: &Settings,
        rng: &mut SmallRng,
    ) -> Vec2 {
        let target_vel = self.compute_target_velocity(i, settings, rng);
        let bot = &self.bots[i];
        let query_radius = 2.0 * bot.radius + 12.0 * settings.orca_time_horizon;
        let neighbors: Vec<_> = self.grid.query(bot.position, query_radius).collect();
        let lines = bot.generate_orca_lines(
            &self.bots,
            &neighbors,
            settings.orca_time_horizon,
            settings.timestep,
        );
        let goal = OptimizationGoal::MinimizeDistanceTo(target_vel);

        match linear_program_2(&lines, bot.max_speed, goal) {
            Ok(v) => v,
            Err((failed_idx, last_good_vel)) => {
                linear_program_3(&lines, failed_idx, bot.max_speed, last_good_vel)
            }
        }
    }

    fn compute_target_velocity(
        &self,
        i: DefaultKey,
        settings: &Settings,
        rng: &mut SmallRng,
    ) -> Vec2 {
        let dt = settings.timestep;
        let bot = &self.bots[i];

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

    fn resolve_bot_bot_collisions(&mut self, settings: &Settings) {
        let mut displacements = Vec::new();
        for i in self.bots.keys() {
            let neighbors = self
                .grid
                .query(self.bots[i].position, self.bots[i].radius + 1.0);
            for j in neighbors {
                if i >= j {
                    continue;
                }

                let d = self.bots[i].position - self.bots[j].position;
                let dist_sq = d.length_squared();
                let r_sum = self.bots[i].radius + self.bots[j].radius;

                if dist_sq < r_sum.powi(2) {
                    let (dir, overlap) = if dist_sq > 0.00001 {
                        let dist = dist_sq.sqrt();
                        (d / dist, r_sum - dist)
                    } else {
                        (vec2(1.0, 0.0), r_sum)
                    };

                    let moving_i = !self.bots[i].waypoints.is_empty();
                    let moving_j = !self.bots[j].waypoints.is_empty();
                    let (weight_i, weight_j) = match (moving_i, moving_j) {
                        (true, true) => (0.5, 0.5),
                        (false, false) => (0.5, 0.5),
                        (false, true) => (1.0, 0.0),
                        (true, false) => (0.0, 1.0),
                    };

                    displacements.push((i, dir * (overlap * weight_i)));
                    displacements.push((j, -dir * (overlap * weight_j)));
                }
            }
        }

        for (i, d) in displacements {
            self.bots[i].position += settings.collision_resolver_fraction * d;
        }
    }

    fn resolve_bot_wall_collisions(&mut self) {
        for (_, bot) in &mut self.bots {
            bot.position = self.tilemap.resolve_collisions(bot.position, bot.radius);
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
