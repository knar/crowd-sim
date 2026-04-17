use nannou::glam::{IVec2, Vec2};
use slotmap::DefaultKey;

pub struct SpatialGrid {
    cells: Vec<Vec<DefaultKey>>,
    cell_size: f32,
    cols: i32,
    rows: i32,
    origin_offset: Vec2,
}

impl SpatialGrid {
    pub fn new(size: IVec2, cell_size: f32, origin_offset: Vec2) -> Self {
        let cols = (size.x as f32 / cell_size).ceil() as i32;
        let rows = (size.y as f32 / cell_size).ceil() as i32;
        Self {
            cells: vec![Vec::new(); (cols * rows) as usize],
            cell_size,
            cols,
            rows,
            origin_offset,
        }
    }

    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.clear();
        }
    }

    fn idx(&self, pos: Vec2) -> Option<usize> {
        let p = pos + self.origin_offset;
        let x = (p.x / self.cell_size) as i32;
        let y = (p.y / self.cell_size) as i32;
        if x >= 0 && x < self.cols && y >= 0 && y < self.rows {
            Some((y * self.cols + x) as usize)
        } else {
            None
        }
    }

    pub fn insert(&mut self, pos: Vec2, key: DefaultKey) {
        if let Some(idx) = self.idx(pos) {
            self.cells[idx].push(key);
        }
    }

    pub fn query(&self, pos: Vec2, radius: f32) -> impl Iterator<Item = DefaultKey> + '_ {
        let p = pos + self.origin_offset;
        let min_x = (((p.x - radius) / self.cell_size).floor() as i32).max(0);
        let max_x = (((p.x + radius) / self.cell_size).ceil() as i32).min(self.cols);
        let min_y = (((p.y - radius) / self.cell_size).floor() as i32).max(0);
        let max_y = (((p.y + radius) / self.cell_size).ceil() as i32).min(self.rows);

        (min_y..max_y).flat_map(move |y| {
            (min_x..max_x).flat_map(move |x| {
                let idx = (y * self.cols + x) as usize;
                self.cells[idx].iter().copied()
            })
        })
    }

    pub fn iter_keys(&self) -> impl Iterator<Item = DefaultKey> + '_ {
        self.cells.iter().flat_map(|cell| cell.iter().copied())
    }
}
