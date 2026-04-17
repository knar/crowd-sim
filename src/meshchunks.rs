use std::f32::consts::PI;

use nannou::{
    Draw,
    color::{Hsl, LinSrgba},
    geom::Tri,
    glam::{IVec2, Vec2, Vec3, ivec2, vec2},
    math::Vec2Rotate,
};

use crate::{WALL_BORDER_COLOR, WALL_COLOR, axis_aligned_rect_rect_intersects, tilemap::TileMap};

#[derive(Debug)]
pub struct MeshChunks {
    chunks: Vec<Chunk>,
    stride: i32,
    chunk_size: IVec2,
    map_size: IVec2,
}

#[derive(Clone, Debug)]
struct Chunk {
    tris: Vec<Tri<(Vec3, LinSrgba)>>,
    dirty: bool,
}

impl MeshChunks {
    pub fn new(tilemap: &TileMap, chunk_size: IVec2) -> Self {
        let size = (tilemap.size - IVec2::ONE) / chunk_size + IVec2::ONE;
        let mut mesh_chunks = MeshChunks {
            chunks: vec![
                Chunk {
                    tris: Vec::new(),
                    dirty: true,
                };
                (size.x * size.y) as usize
            ],
            stride: size.x,
            chunk_size,
            map_size: tilemap.size,
        };
        mesh_chunks.update(tilemap);
        mesh_chunks
    }

    pub fn mark_dirty(&mut self, tile: IVec2) {
        for dx in [-1, 0, 1] {
            for dy in [-1, 0, 1] {
                let tile = tile + ivec2(dx, dy);
                if tile != tile.clamp(IVec2::ZERO, self.map_size - IVec2::ONE) {
                    continue;
                }
                let coord = tile / self.chunk_size;
                let idx = (coord.y * self.stride + coord.x) as usize;
                self.chunks[idx].dirty = true;
            }
        }
    }

    pub fn update(&mut self, tilemap: &TileMap) {
        for (i, chunk) in self.chunks.iter_mut().enumerate() {
            if !chunk.dirty {
                continue;
            }
            chunk.tris.clear();
            let cx = ((i as i32) % self.stride) * self.chunk_size.x;
            let cy = ((i as i32) / self.stride) * self.chunk_size.y;
            for x in cx..cx + self.chunk_size.x {
                if x >= tilemap.size.x {
                    break;
                }
                for y in cy..cy + self.chunk_size.y {
                    if y >= tilemap.size.y {
                        continue;
                    }
                    let cur = ivec2(x, y);
                    if tilemap[cur] {
                        let center = tilemap.tile_center(cur);
                        // border between r0 and r1
                        let r0 = 0.45;
                        let r1 = 0.5;
                        for d in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)]
                            .map(|(x, y)| vec2(x, y))
                        {
                            let gx = tilemap[center + vec2(d.x, 0.0)];
                            let gy = tilemap[center + vec2(0.0, d.y)];
                            let gxy = tilemap[center + d];

                            // gx | gy | gxy
                            //  f |  f |  f  ->  rounded corner
                            //  t |  f |  -  ->  border on y edge
                            //  f |  t |  -  ->  border on x edge
                            //  f |  f |  t  ->  border on both x and y edge
                            //  t |  t |  f  ->  "inner corner"
                            //  t |  t |  t  ->  no border

                            let (fill, border, border_ontop) = if gx {
                                if gy {
                                    if gxy {
                                        // no border
                                        let fill = vec![
                                            [Vec2::ZERO, vec2(r1, 0.0), vec2(r1, r1)],
                                            [Vec2::ZERO, vec2(r1, r1), vec2(0.0, r1)],
                                        ];
                                        (fill, vec![], vec![])
                                    } else {
                                        // inner corner
                                        let fill = vec![
                                            [Vec2::ZERO, vec2(r1, 0.0), vec2(r1, r1)],
                                            [Vec2::ZERO, vec2(r1, r1), vec2(0.0, r1)],
                                        ];
                                        let border_ontop = vec![
                                            [vec2(r0, r0), vec2(r0, r1), vec2(r1, r1)],
                                            [vec2(r0, r0), vec2(r1, r0), vec2(r1, r1)],
                                        ];
                                        (fill, vec![], border_ontop)
                                    }
                                } else {
                                    // border on y edge
                                    let fill = vec![
                                        [Vec2::ZERO, vec2(r1, 0.0), vec2(r1, r0)],
                                        [Vec2::ZERO, vec2(r1, r0), vec2(0.0, r0)],
                                    ];
                                    let border = vec![
                                        [Vec2::ZERO, vec2(r1, 0.0), vec2(r1, r1)],
                                        [Vec2::ZERO, vec2(r1, r1), vec2(0.0, r1)],
                                    ];
                                    (fill, border, vec![])
                                }
                            } else if gy {
                                // border on x edge
                                let fill = vec![
                                    [Vec2::ZERO, vec2(0.0, r1), vec2(r0, r1)],
                                    [Vec2::ZERO, vec2(r0, r1), vec2(r0, 0.0)],
                                ];
                                let border = vec![
                                    [Vec2::ZERO, vec2(0.0, r1), vec2(r1, r1)],
                                    [Vec2::ZERO, vec2(r1, r1), vec2(r1, 0.0)],
                                ];
                                (fill, border, vec![])
                            } else if gxy {
                                // border on both x and y edge
                                let fill = vec![
                                    [Vec2::ZERO, vec2(0.0, r0), vec2(r0, r0)],
                                    [Vec2::ZERO, vec2(r0, r0), vec2(r0, 0.0)],
                                ];
                                let border = vec![
                                    [Vec2::ZERO, vec2(0.0, r1), vec2(r1, r1)],
                                    [Vec2::ZERO, vec2(r1, r1), vec2(r1, 0.0)],
                                ];
                                (fill, border, vec![])
                            } else {
                                // rounded corner
                                let n = 6;
                                let step = PI / 2.0 / n as f32;
                                let s = Vec2::ONE.rotate(-PI / 4.0).normalize();
                                let mut fill = vec![];
                                let mut border = vec![];
                                for i in 0..n {
                                    let a = s.rotate(step * i as f32);
                                    let b = s.rotate(step * (i + 1) as f32);
                                    fill.push([Vec2::ZERO, a * r0, b * r0]);
                                    border.push([Vec2::ZERO, a * r1, b * r1]);
                                }
                                (fill, border, vec![])
                            };

                            for (color, tris) in [
                                (*WALL_BORDER_COLOR, border),
                                (*WALL_COLOR, fill),
                                (*WALL_BORDER_COLOR, border_ontop),
                            ] {
                                let color = Hsl::from(color.into_format());
                                let color = LinSrgba::from(color);
                                for vs in tris {
                                    let vs = vs.map(|p| (p * d) + center);
                                    let tri = Tri::from(vs.map(|p| (p.extend(0.0), color)));
                                    chunk.tris.push(tri);
                                }
                            }
                        }
                    }
                }
            }

            chunk.dirty = false;
        }
    }

    pub fn draw(&self, draw: &Draw, view_center: Vec2, view_size: Vec2) {
        let chunk_size = self.chunk_size.as_f32();
        let half_map_size = self.map_size.as_f32() / 2.0;
        for (i, chunk) in self.chunks.iter().enumerate() {
            let i = i as i32;
            let block = ivec2(i % self.stride, i / self.stride) * self.chunk_size;
            let chunk_center = block.as_f32() + chunk_size / 2.0 - half_map_size;
            if axis_aligned_rect_rect_intersects(chunk_center, chunk_size, view_center, view_size) {
                draw.mesh().tris_colored(chunk.tris.iter().copied());
            }
        }
    }
}
