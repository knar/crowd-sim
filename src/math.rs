use std::ops::{Add, Mul, Sub};

use nannou::glam::Vec2;

pub fn lerp<T>(start: T, end: T, factor: f32) -> T
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<f32, Output = T>,
{
    start + (end - start) * factor
}

pub fn circle_rect_intersects(
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

pub fn axis_aligned_rect_rect_intersects(
    a_center: Vec2,
    a_size: Vec2,
    b_center: Vec2,
    b_size: Vec2,
) -> bool {
    let a_halfsize = a_size / 2.0;
    let b_halfsize = b_size / 2.0;
    let a_min = a_center - a_halfsize;
    let a_max = a_center + a_halfsize;
    let b_min = b_center - b_halfsize;
    let b_max = b_center + b_halfsize;
    a_min.x.max(b_min.x) < a_max.x.min(b_max.x) && a_min.y.max(b_min.y) < a_max.y.min(b_max.y)
}

pub fn distance_to_segment_sq(start: Vec2, end: Vec2, target: Vec2) -> f32 {
    let ab = end - start;
    let ap = target - start;
    let len_sq = ab.length_squared();
    if len_sq == 0.0 {
        return (target - start).length_squared();
    }
    let t = (ap.dot(ab) / len_sq).clamp(0.0, 1.0);
    let closest_point = start + (ab * t);
    (target - closest_point).length_squared()
}
