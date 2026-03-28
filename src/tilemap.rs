use std::ops::{Index, IndexMut};

use nannou::glam::{IVec2, Vec2, ivec2, vec2};

pub struct TileMap {
    walls: Vec<bool>,
    stride: i32,
    pub size: IVec2,
    half_size: Vec2,
}

impl TileMap {
    pub fn new(size: IVec2) -> Self {
        let stride = size.x + 2;
        let mut walls = vec![false; (stride * (size.y + 2)) as usize];
        for x in 0..size.x + 2 {
            walls[x as usize] = true;
            walls[(stride * (size.y + 1) + x) as usize] = true;
        }
        for y in 1..=size.y {
            walls[(stride * y) as usize] = true;
            walls[(stride * (y + 1) - 1) as usize] = true;
        }
        let half_size = vec2(size.x as f32, size.y as f32) / 2.0;
        Self {
            walls,
            stride,
            size,
            half_size,
        }
    }

    pub fn set(&mut self, pos: Vec2, wall: bool) -> bool {
        if self[pos] == wall {
            false
        } else {
            self[pos] = wall;
            true
        }
    }

    pub fn tile_center(&self, coord: IVec2) -> Vec2 {
        vec2(coord.x as f32, coord.y as f32) - self.half_size + Vec2::splat(0.5)
    }

    pub fn coord(&self, pos: Vec2) -> IVec2 {
        let tile = (pos + self.half_size).floor();
        ivec2(tile.x as i32, tile.y as i32)
    }

    fn idx(&self, tile: IVec2) -> usize {
        (self.stride * (tile.y + 1) + tile.x + 1) as usize
    }

    pub fn line_of_sight(&self, from: Vec2, to: Vec2) -> bool {
        self.line_hit(from, to) == to
    }

    fn line_hit(&self, from: Vec2, to: Vec2) -> Vec2 {
        let dir = (to - from).normalize();
        let unit = vec2((1.0 / dir.x).abs(), (1.0 / dir.y).abs());
        let s = from + self.half_size;
        let (mut ray_x, step_x) = if dir.x < 0.0 {
            (unit.x * s.x.fract(), -1)
        } else {
            (unit.x * (1.0 - s.x.fract()), 1)
        };
        let (mut ray_y, step_y) = if dir.y < 0.0 {
            (unit.y * s.y.fract(), -1)
        } else {
            (unit.y * (1.0 - s.y.fract()), 1)
        };
        let mut dist;
        let mut cur = self.coord(from);
        let end = self.coord(to);
        while cur != end {
            if ray_x < ray_y {
                dist = ray_x;
                ray_x += unit.x;
                cur.x += step_x;
            } else {
                dist = ray_y;
                ray_y += unit.y;
                cur.y += step_y;
            }
            if !self.walls[self.idx(cur)] {
                continue;
            }
            if cur == end {
                // // Allow line of sight if the point does not collide
                if self.resolve_collisions(to, 0.0) == to {
                    return to;
                }
            }
            return from + dir * dist;
        }
        to
    }

    pub fn resolve_collisions(&self, mut pos: Vec2, radius: f32) -> Vec2 {
        assert!(radius <= 0.5);

        let half_size = self.half_size - Vec2::splat(radius);
        pos.clamp(-half_size, half_size);

        let tile = self.coord(pos);
        let tile_center = self.tile_center(tile);

        // pos relative to tile center
        let tpos = pos - tile_center;
        let apos = tpos.abs();
        let sign = Vec2::from(tpos.to_array().map(f32::signum));

        let g = self[tile];
        let gx = self[tile + ivec2(sign.x as i32, 0)];
        let gy = self[tile + ivec2(0, sign.y as i32)];
        let gxy = self[tile + ivec2(sign.x as i32, sign.y as i32)];

        let crad = radius + 0.5;
        if !g {
            // +X wall or corner
            if gx {
                if apos.x > 0.5 - radius {
                    if gy || gxy {
                        pos.x = (0.5 - radius) * sign.x + tile_center.x;
                    } else {
                        let d = apos - vec2(1.0, 0.0);
                        if d.length_squared() < crad * crad {
                            pos = (d / d.length() * crad + vec2(1.0, 0.0)) * sign + tile_center;
                        }
                    }
                }
            }
            // +Y wall or corner
            if gy {
                // push down
                if apos.y > 0.5 - radius {
                    if gx || gxy {
                        pos.y = (0.5 - radius) * sign.y + tile_center.y;
                    } else {
                        let d = apos - vec2(0.0, 1.0);
                        if d.length_squared() < crad * crad {
                            pos = (d / d.length() * crad + vec2(0.0, 1.0)) * sign + tile_center;
                        }
                    }
                }
            }
            // +XY corner
            if gxy && !(gx || gy) {
                let d = apos - vec2(1.0, 1.0);
                if d.length_squared() < crad * crad {
                    pos = (d / d.length() * crad + vec2(1.0, 1.0)) * sign + tile_center;
                }
            }
        } else {
            // -X wall
            if gy || gxy {
                pos.x = (0.5 + radius) * sign.x + tile_center.x;
            }
            // -Y wall
            if gx || gxy {
                pos.y = (0.5 + radius) * sign.y + tile_center.y;
            }
            // -XY corner
            if !(gxy || gx || gy) {
                let d = apos;
                if d.length_squared() < crad * crad {
                    pos = (d / d.length() * crad) * sign + tile_center;
                }
            }
        }
        pos
    }
}

impl Index<IVec2> for TileMap {
    type Output = bool;
    fn index(&self, index: IVec2) -> &Self::Output {
        &self.walls[self.idx(index)]
    }
}

impl IndexMut<IVec2> for TileMap {
    fn index_mut(&mut self, index: IVec2) -> &mut Self::Output {
        let idx = self.idx(index);
        &mut self.walls[idx]
    }
}

impl Index<Vec2> for TileMap {
    type Output = bool;
    fn index(&self, index: Vec2) -> &Self::Output {
        &self.walls[self.idx(self.coord(index))]
    }
}

impl IndexMut<Vec2> for TileMap {
    fn index_mut(&mut self, index: Vec2) -> &mut Self::Output {
        let idx = self.idx(self.coord(index));
        &mut self.walls[idx]
    }
}
