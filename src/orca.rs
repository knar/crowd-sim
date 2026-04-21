use nannou::glam::{Vec2, vec2};
use slotmap::{DefaultKey, SlotMap};

use crate::bot::Bot;

// ported from https://github.com/snape/RVO2/blob/main/src/Agent.cc

const RVO_EPSILON: f32 = 0.00001;

#[derive(Default)]
pub struct Line {
    point: Vec2,
    dir: Vec2,
}

#[derive(Clone, Copy)]
pub enum OptimizationGoal {
    MinimizeDistanceTo(Vec2),
    MaximizeDirection(Vec2),
}

impl Bot {
    pub fn generate_orca_lines(
        &self,
        bots: &SlotMap<DefaultKey, Bot>,
        neighbors: &[DefaultKey],
        tau: f32,
        dt: f32,
    ) -> Vec<Line> {
        let mut lines = Vec::with_capacity(neighbors.len());
        let inv_tau = 1.0 / tau;
        let inv_dt = 1.0 / dt;

        for k in neighbors {
            let other = &bots[*k];
            if self.position == other.position {
                continue;
            }

            let rel_pos = other.position - self.position;
            let rel_vel = self.velocity - other.velocity;
            let dist_sq = rel_pos.length_squared();
            let r_sum = self.radius + other.radius;
            let r_sum_sq = r_sum.powi(2);

            let u: Vec2;
            let mut line = Line::default();

            if dist_sq > r_sum_sq {
                if self.velocity.length_squared() > 0.01 && other.velocity.length_squared() > 0.01 {
                    if self.velocity.dot(rel_pos) < 0.0 {
                        continue;
                    }
                    if self.velocity.normalize().dot(other.velocity.normalize()) > 0.94 {
                        continue;
                    }
                }

                let w = rel_vel - inv_tau * rel_pos;
                let w_len_sq = w.length_squared();
                let dot_prod1 = w.dot(rel_pos);

                if dot_prod1 < 0.0 && dot_prod1.powi(2) > r_sum_sq * w_len_sq {
                    let w_len = w_len_sq.sqrt();
                    let unit_w = if w_len > 0.0001 {
                        w / w_len
                    } else {
                        vec2(1.0, 0.0)
                    };
                    line.dir = vec2(unit_w.y, -unit_w.x);
                    u = (r_sum * inv_tau - w_len) * unit_w;
                } else {
                    let leg = (dist_sq - r_sum_sq).sqrt();

                    if rel_pos.perp_dot(w) > 0.0 {
                        line.dir = vec2(
                            rel_pos.x * leg - rel_pos.y * r_sum,
                            rel_pos.x * r_sum + rel_pos.y * leg,
                        ) / dist_sq;
                    } else {
                        line.dir = -vec2(
                            rel_pos.x * leg + rel_pos.y * r_sum,
                            -rel_pos.x * r_sum + rel_pos.y * leg,
                        ) / dist_sq;
                    }

                    let dot_prod2 = rel_vel.dot(line.dir);
                    u = dot_prod2 * line.dir - rel_vel;
                }
            } else {
                let w = rel_vel - inv_dt * rel_pos;
                let w_len = w.length();
                let unit_w = if w_len > 0.0001 {
                    w / w_len
                } else {
                    vec2(1.0, 0.0)
                };

                line.dir = vec2(unit_w.y, -unit_w.x);
                u = (r_sum * inv_dt - w_len) * unit_w;
            }

            line.point = self.velocity + 0.5 * u;
            lines.push(line);
        }

        lines
    }
}

pub fn linear_program_3(
    lines: &[Line],
    failed_idx: usize,
    radius: f32,
    last_good_vel: Vec2,
) -> Vec2 {
    let mut opt_vel = last_good_vel;
    let mut dist = 0.0;

    for i in failed_idx..lines.len() {
        if lines[i].dir.perp_dot(lines[i].point - opt_vel) > dist {
            let mut proj_lines = Vec::new();

            for j in 0..i {
                let mut proj_line = Line::default();

                let det = lines[i].dir.perp_dot(lines[j].dir);
                if det.abs() <= RVO_EPSILON {
                    if lines[i].dir.dot(lines[j].dir) > 0.0 {
                        continue;
                    } else {
                        proj_line.point = 0.5 * (lines[i].point + lines[j].point);
                    }
                } else {
                    let t = lines[j].dir.perp_dot(lines[i].point - lines[j].point) / det;
                    proj_line.point = lines[i].point + (lines[i].dir * t);
                }
                proj_line.dir = (lines[j].dir - lines[i].dir).normalize_or_zero();
                proj_lines.push(proj_line);
            }

            let push_dir = lines[i].dir.perp();
            let goal = OptimizationGoal::MaximizeDirection(push_dir);

            if let Ok(new_vel) = linear_program_2(&proj_lines, radius, goal) {
                opt_vel = new_vel;
                dist = lines[i].dir.perp_dot(lines[i].point - opt_vel);
            }
        }
    }
    opt_vel
}

pub fn linear_program_2(
    lines: &[Line],
    radius: f32,
    goal: OptimizationGoal,
) -> Result<Vec2, (usize, Vec2)> {
    let mut opt_vel = match goal {
        OptimizationGoal::MinimizeDistanceTo(target) => {
            if target.length_squared() > radius.powi(2) {
                target.normalize_or_zero() * radius
            } else {
                target
            }
        }
        OptimizationGoal::MaximizeDirection(dir) => dir * radius,
    };

    for (i, line) in lines.iter().enumerate() {
        if line.dir.perp_dot(line.point - opt_vel) > 0.0 {
            if let Some(new_vel) = linear_program_1(&lines[..=i], radius, goal) {
                opt_vel = new_vel;
            } else {
                return Err((i, opt_vel));
            }
        }
    }

    Ok(opt_vel)
}

fn linear_program_1(lines: &[Line], radius: f32, goal: OptimizationGoal) -> Option<Vec2> {
    let (line, prev_lines) = lines.split_last()?;

    let dot_product = line.point.dot(line.dir);
    let discriminant = dot_product.powi(2) + radius.powi(2) - line.point.length_squared();

    if discriminant < 0.0 {
        return None;
    }

    let sqrt_discriminant = discriminant.sqrt();
    let mut t_left = -dot_product - sqrt_discriminant;
    let mut t_right = -dot_product + sqrt_discriminant;

    for other in prev_lines {
        let denominator = line.dir.perp_dot(other.dir);
        let numerator = other.dir.perp_dot(line.point - other.point);

        if denominator.abs() <= RVO_EPSILON {
            if numerator < 0.0 {
                return None;
            }
            continue;
        }

        let t = numerator / denominator;

        if denominator >= 0.0 {
            t_right = t_right.min(t);
        } else {
            t_left = t_left.max(t);
        }

        if t_left > t_right {
            return None;
        }
    }

    match goal {
        OptimizationGoal::MinimizeDistanceTo(target) => {
            let t = line.dir.dot(target - line.point).clamp(t_left, t_right);
            Some(line.point + t * line.dir)
        }
        OptimizationGoal::MaximizeDirection(dir) => {
            if dir.dot(line.dir) > 0.0 {
                Some(line.point + t_right * line.dir)
            } else {
                Some(line.point + t_left * line.dir)
            }
        }
    }
}
