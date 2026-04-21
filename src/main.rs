mod bot;
mod meshchunks;
mod orca;
mod spatialgrid;
mod tilemap;
mod world;

use std::ops::{Add, Mul, Sub};
use std::sync::LazyLock;

use nannou::color::{Rgb, rgb_u32, rgba8};
use nannou::prelude::*;
use nannou::rand::rngs::SmallRng;
use nannou::rand::{Rng, SeedableRng};
use nannou_egui::{
    Egui,
    egui::{self, Slider},
};

use slotmap::DefaultKey;

use crate::bot::Task;
use crate::world::World;

static BACKGROUND_COLOR: LazyLock<Rgb<u8>> = LazyLock::new(|| rgb_u32(0x101010));
static GROUND_COLOR: LazyLock<Rgb<u8>> = LazyLock::new(|| rgb_u32(0x363652));
static WALL_COLOR: LazyLock<Rgb<u8>> = LazyLock::new(|| rgb_u32(0x545480));
static WALL_BORDER_COLOR: LazyLock<Rgb<u8>> = LazyLock::new(|| rgb_u32(0x7979a7));
static FOREGROUND_COLOR: LazyLock<Rgb<u8>> = LazyLock::new(|| rgb_u32(0xe7dfdb));

fn main() {
    nannou::app(model).event(event).run();
}

struct Model {
    egui: Egui,
    client: Client,
    settings: Settings,
    world: World,
    rng: SmallRng,
}

impl Model {
    fn mouse_world_pos(&self) -> Option<Vec2> {
        self.client
            .mouse_position
            .map(|m| self.client.camera.screen_to_world(m))
    }

    fn reset_world(&mut self) {
        self.client.selection.clear();
        self.world.bots.clear();
    }

    fn tick(&mut self) {
        self.world.tick(&self.settings, &mut self.rng);
    }
}

struct Client {
    accumulator: f32,
    camera: Camera,
    drag_start: Option<Vec2>,
    mouse_position: Option<Vec2>,
    selection: Vec<DefaultKey>,
    edit_walls_mode: bool,
}

struct Camera {
    position: Vec2,
    zoom: Vec2,
}

impl Camera {
    fn _world_to_screen(&self, w: Vec2) -> Vec2 {
        (w - self.position) * self.zoom
    }

    fn screen_to_world(&self, s: Vec2) -> Vec2 {
        (s / self.zoom) + self.position
    }
}

struct Settings {
    timestep: f32,
    use_orca: bool,
    orca_time_horizon: f32,
    collision_resolver_iters: usize,
    collision_resolver_fraction: f32,
    arrival_distance: f32,
    timescale: f32,
    interpolate_frames: bool,
    draw_head_dot: bool,
    draw_debug_lines: bool,
    draw_trail: bool,
    paused: bool,
}

fn model(app: &App) -> Model {
    let window_id = app
        .new_window()
        .title("it's just rice bruv")
        .view(view)
        .raw_event(raw_gui_event)
        .build()
        .unwrap();
    let window = app.window(window_id).unwrap();
    let egui = Egui::from_window(&window);

    let world_size = ivec2(256, 256);

    let settings = Settings {
        timestep: 0.05,
        use_orca: true,
        orca_time_horizon: 0.3,
        collision_resolver_iters: 3,
        collision_resolver_fraction: 0.5,
        arrival_distance: 0.01,
        timescale: 1.0,
        interpolate_frames: true,
        draw_head_dot: true,
        draw_debug_lines: true,
        draw_trail: false,
        paused: false,
    };

    let client = Client {
        accumulator: 0.0,
        selection: Vec::new(),
        mouse_position: None,
        drag_start: None,
        camera: Camera {
            position: Vec2::ZERO,
            zoom: Vec2::splat(50.0),
        },
        edit_walls_mode: false,
    };

    let mut model = Model {
        egui,
        settings,
        client,
        world: World::new(world_size),
        rng: SmallRng::seed_from_u64(0),
    };
    model.reset_world();

    model
}

fn raw_gui_event(_app: &App, model: &mut Model, event: &nannou::winit::event::WindowEvent) {
    // allow egui to see the raw winit events if they happen in the gui window
    model.egui.handle_raw_event(event);
}

fn event(app: &App, model: &mut Model, event: Event) {
    match event {
        Event::Update(update) => {
            if !model.settings.paused {
                model.client.accumulator +=
                    update.since_last.as_secs_f32() * model.settings.timescale;
            }
            while model.client.accumulator >= model.settings.timestep {
                model.client.accumulator -= model.settings.timestep;
                model.tick();
            }

            model.egui.set_elapsed_time(update.since_start);
            settings_window(model);
        }
        Event::WindowEvent {
            simple: Some(event),
            ..
        } => {
            let gui_ctx = model.egui.ctx();
            if gui_ctx.wants_pointer_input() || gui_ctx.wants_keyboard_input() {
                return;
            }
            handle_sim_event(app, model, event);
        }
        _ => {}
    }
}

fn settings_window(model: &mut Model) {
    let ctx = model.egui.begin_frame();
    egui::Window::new("Settings")
        .default_pos((20.0, 20.0))
        .show(&ctx, |ui| {
            egui::Grid::new("settings_grid")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Timestep");
                    ui.add(Slider::new(&mut model.settings.timestep, 0.01..=0.2));
                    ui.end_row();

                    ui.label("Use ORCA");
                    ui.checkbox(&mut model.settings.use_orca, "");
                    ui.end_row();

                    ui.label("ORCA Time Horizon");
                    ui.add(Slider::new(
                        &mut model.settings.orca_time_horizon,
                        0.1..=3.0,
                    ));
                    ui.end_row();

                    ui.label("Collision resolver iterations");
                    ui.add(Slider::new(
                        &mut model.settings.collision_resolver_iters,
                        1..=10,
                    ));
                    ui.end_row();

                    ui.label("Collision resolver fraction");
                    ui.add(Slider::new(
                        &mut model.settings.collision_resolver_fraction,
                        0.05..=1.0,
                    ));
                    ui.end_row();

                    ui.label("Arrival distance");
                    ui.add(Slider::new(
                        &mut model.settings.arrival_distance,
                        0.001..=0.2,
                    ));
                    ui.end_row();

                    ui.separator();
                    ui.separator();
                    ui.end_row();

                    ui.label("Timescale");
                    ui.add(Slider::new(&mut model.settings.timescale, 0.01..=5.0));
                    ui.end_row();

                    ui.label("Interpolate frames");
                    ui.checkbox(&mut model.settings.interpolate_frames, "");
                    ui.end_row();

                    ui.label("Draw head dot");
                    ui.checkbox(&mut model.settings.draw_head_dot, "");
                    ui.end_row();

                    ui.label("Draw debug lines");
                    ui.checkbox(&mut model.settings.draw_debug_lines, "");
                    ui.end_row();

                    ui.label("Draw trail");
                    ui.checkbox(&mut model.settings.draw_trail, "");
                    ui.end_row();

                    ui.separator();
                    ui.separator();
                    ui.end_row();

                    ui.label("Pause");
                    ui.checkbox(&mut model.settings.paused, "");
                    ui.end_row();

                    if model.settings.paused && ui.button("Tick").clicked() {
                        model.world.tick(&model.settings, &mut model.rng);
                        ui.end_row();
                    }
                });

            if !model.client.selection.is_empty() {
                ui.label(format!("Selected: {}", model.client.selection.len()));
                let bot = &model.world.bots[model.client.selection[0]];
                ui.label(bot.summary());
            }
        });
}

fn handle_sim_event(app: &App, model: &mut Model, event: WindowEvent) {
    match event {
        WindowEvent::KeyPressed(key) => match key {
            Key::Space => {
                model.settings.paused = !model.settings.paused;
            }
            Key::Return if model.settings.paused => {
                model.tick();
            }
            Key::S => {
                if let Some(pos) = model.mouse_world_pos() {
                    model.world.add_bot(pos, Vec2::ZERO, None);
                }
            }
            Key::R => {
                model.reset_world();
            }
            Key::Key1 => {
                model.reset_world();
                let pos = vec2(-5.0, 0.0);
                model.world.add_bot(pos, Vec2::ZERO, Some(Task::Move(-pos)));
                model.world.add_bot(-pos, Vec2::ZERO, Some(Task::Move(pos)));
            }
            Key::Key2 => {
                model.reset_world();
                let pos = vec2(-5.0, 0.0);
                let dy = vec2(0.0, 0.4);
                model.world.add_bot(pos, Vec2::ZERO, Some(Task::Move(-pos)));
                model
                    .world
                    .add_bot(-pos + dy, Vec2::ZERO, Some(Task::Move(pos)));
                model
                    .world
                    .add_bot(-pos - dy, Vec2::ZERO, Some(Task::Move(pos)));
            }
            Key::Key3 => {
                model.reset_world();
                let mut rng = SmallRng::seed_from_u64(0);
                let x = 5.0;
                for _ in 0..10 {
                    let pos = vec2(rng.gen_range(-x..x), rng.gen_range(-x..x));
                    model.world.add_bot(pos, Vec2::ZERO, Some(Task::Move(-pos)));
                }
            }
            Key::Key4 => {
                model.reset_world();
                let mut rng = SmallRng::seed_from_u64(0);
                let x = 10.0;
                for _ in 0..50 {
                    let pos = vec2(rng.gen_range(-x..x), rng.gen_range(-x..x));
                    model.world.add_bot(pos, Vec2::ZERO, Some(Task::Move(-pos)));
                }
            }
            Key::Key5 => {
                model.reset_world();
                let mut rng = SmallRng::seed_from_u64(0);
                for _ in 0..100 {
                    let a = vec2(rng.gen_range(-10.0..-8.0), rng.gen_range(-10.0..10.0));
                    let b = vec2(rng.gen_range(8.0..10.0), rng.gen_range(-10.0..10.0));
                    model
                        .world
                        .add_bot(a, Vec2::ZERO, Some(Task::Move(vec2(-a.x, a.y))));
                    model
                        .world
                        .add_bot(b, Vec2::ZERO, Some(Task::Move(vec2(-b.x, b.y))));
                }
            }
            Key::D => {
                model.settings.draw_debug_lines = !model.settings.draw_debug_lines;
            }
            Key::W => {
                model.client.edit_walls_mode = !model.client.edit_walls_mode;
            }
            Key::F => {
                let min_x = -model.world.half_size.x;
                let max_x = model.world.half_size.x;
                let min_y = -model.world.half_size.y;
                let max_y = model.world.half_size.y;
                for _ in 0..5 {
                    let x = random_range(min_x, max_x);
                    let y = random_range(min_y, max_y);
                    model.world.add_bot(vec2(x, y), Vec2::ZERO, None);
                }
            }
            Key::A => {
                model.client.selection = model.world.bots.keys().collect();
            }
            Key::Delete => {
                for k in model.client.selection.drain(..) {
                    model.world.delete_bot(k);
                }
            }
            _ => {}
        },
        WindowEvent::MousePressed(btn) => {
            if !model.client.edit_walls_mode {
                match btn {
                    MouseButton::Left => {
                        model.client.drag_start = model.mouse_world_pos();
                    }
                    MouseButton::Right => {
                        if let Some(pos) = model.mouse_world_pos() {
                            for &k in &model.client.selection {
                                let task = Task::Move(pos);
                                if app.keys.mods.shift() {
                                    model.world.add_bot_task(k, task);
                                } else {
                                    model.world.set_bot_task(k, task);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            } else if let Some(pos) = model.mouse_world_pos()
                && pos == pos.clamp(-model.world.half_size, model.world.half_size)
            {
                match btn {
                    MouseButton::Left => {
                        model.world.set_wall(pos, true);
                        model.world.mesh_chunks.update(&model.world.tilemap);
                    }
                    MouseButton::Right => {
                        model.world.set_wall(pos, false);
                        model.world.mesh_chunks.update(&model.world.tilemap);
                    }
                    _ => {}
                }
            }
        }
        WindowEvent::MouseReleased(MouseButton::Left) => {
            if let (Some(start), Some(end)) = (model.client.drag_start, model.mouse_world_pos()) {
                let frac = model.client.accumulator / model.settings.timestep;
                let rect_center = start + (end - start) / 2.0;
                let rect_size = (start - end).abs();
                model.client.selection = model
                    .world
                    .bots
                    .keys()
                    .filter(|&k| {
                        let bot = &model.world.bots[k];
                        let pos = lerp(bot.prev_pos(), bot.position, frac);
                        circle_rect_intersects(pos, bot.radius, rect_center, rect_size)
                    })
                    .collect();
                model.client.drag_start = None;
            }
        }
        WindowEvent::MouseMoved(pos) => {
            // drag-pan
            if app.mouse.buttons.middle().is_down()
                && let Some(last) = model.client.mouse_position
            {
                let d = pos - last;
                model.client.camera.position -= d / model.client.camera.zoom;
            }

            model.client.mouse_position = Some(pos);

            // edit walls
            if model.client.edit_walls_mode
                && let Some(pos) = model.mouse_world_pos()
                && pos == pos.clamp(-model.world.half_size, model.world.half_size)
            {
                if app.mouse.buttons.left().is_down() {
                    model.world.set_wall(pos, true);
                    model.world.mesh_chunks.update(&model.world.tilemap);
                } else if app.mouse.buttons.right().is_down() {
                    model.world.set_wall(pos, false);
                    model.world.mesh_chunks.update(&model.world.tilemap);
                }
            }
        }
        WindowEvent::MouseExited => model.client.mouse_position = None,
        WindowEvent::MouseWheel(delta, _) => {
            let y = match delta {
                MouseScrollDelta::LineDelta(_, lines) => lines * 10.0,
                MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
            };
            let prev_pos = model.mouse_world_pos().unwrap();
            model.client.camera.zoom *= (y / 100.0).exp2();
            let new_pos = model.mouse_world_pos().unwrap();
            model.client.camera.position += prev_pos - new_pos;
        }
        _ => {}
    }
}

fn view(app: &App, model: &Model, frame: Frame) {
    let cam = &model.client.camera;
    let settings = &model.settings;
    let world = &model.world;

    let draw = app.draw();

    draw.background().color(*BACKGROUND_COLOR);

    let wdraw = draw
        .scale_x(cam.zoom.x)
        .scale_y(cam.zoom.y)
        .translate(-vec3(cam.position.x, cam.position.y, 0.0));

    wdraw
        .rect()
        .x_y(0.0, 0.0)
        .wh(world.half_size * 2.0)
        .color(*GROUND_COLOR);

    // grid dots
    let half_win_size = (app.window_rect().wh() / 2.0).ceil();
    let view_min = cam.screen_to_world(-half_win_size);
    let view_max = cam.screen_to_world(half_win_size);
    if cam.zoom.abs().min_element() > 30.0 {
        let size = 0.05;
        let color = rgba(1.0, 1.0, 1.0, 0.03);
        let min_x = view_min.x.max(-world.half_size.x).floor() as i32;
        let max_x = view_max.x.min(world.half_size.x).ceil() as i32;
        let min_y = view_min.y.max(-world.half_size.y).floor() as i32;
        let max_y = view_max.y.min(world.half_size.y).ceil() as i32;
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                wdraw
                    .rect()
                    .x_y(x as f32, y as f32)
                    .w_h(size, size)
                    .color(color);
            }
        }
    }

    // walls
    let view_center = cam.position;
    let view_size = view_max - view_min;
    model.world.mesh_chunks.draw(&wdraw, view_center, view_size);

    // bots
    let frac = model.client.accumulator / settings.timestep;
    for (_, bot) in &world.bots {
        let pos = if !settings.paused && settings.interpolate_frames {
            lerp(bot.prev_pos(), bot.position, frac)
        } else if settings.draw_debug_lines {
            bot.prev_pos()
        } else {
            bot.position
        };

        if model.settings.draw_trail {
            for &p in &bot.trail {
                wdraw
                    .ellipse()
                    .xy(p)
                    .radius(bot.radius * 0.2)
                    .resolution(16.0)
                    .color(rgba8(0xe7, 0xdf, 0xdb, 0x20));
            }
        }

        let clr = if bot.tasks.is_empty() {
            *FOREGROUND_COLOR
        } else {
            CYAN
        };
        // the circle!
        wdraw
            .ellipse()
            .xy(pos)
            .radius(bot.radius - 0.02)
            .resolution(32.0)
            .stroke(clr)
            .stroke_weight(0.04)
            .no_fill();

        if model.settings.draw_head_dot {
            let prev_vel = (bot.prev_pos() - bot.prev_prev_pos()) / model.settings.timestep;
            let lean = lerp(prev_vel, bot.velocity, frac) / bot.max_speed;

            wdraw
                .ellipse()
                .xy(pos + lean * bot.radius * 0.6)
                .radius(bot.radius * 0.4)
                .resolution(16.0)
                .color(*FOREGROUND_COLOR);
        }
    }

    for k in &model.client.selection {
        let bot = &model.world.bots[*k];
        let pos = if !settings.paused && settings.interpolate_frames {
            lerp(bot.prev_pos(), bot.position, frac)
        } else if settings.draw_debug_lines {
            bot.prev_pos()
        } else {
            bot.position
        };

        wdraw
            .ellipse()
            .xy(pos)
            .radius(bot.radius)
            .resolution(32.0)
            .stroke(rgba(0.4, 0.8, 0.4, 1.0))
            .stroke_weight(0.03)
            .no_fill();

        if model.settings.draw_debug_lines {
            let scale = model.settings.timestep;
            let thickness = 0.02;
            let p = pos;
            if bot.debug_accel.length_squared() > 0.001 {
                let v = bot.debug_accel * scale;
                draw_arrow(&wdraw, p, p + v, thickness * 1.5, rgba8(0, 255, 255, 255));
            }

            if bot.debug_arrival_dist > 0.0 {
                wdraw
                    .ellipse()
                    .xy(pos)
                    .radius(bot.debug_arrival_dist - 0.01)
                    .resolution(32.0)
                    .stroke(GREEN)
                    .stroke_weight(0.02)
                    .no_fill();
            }
        }

        for task in &bot.tasks {
            match task {
                Task::Move(target) => {
                    let clr = rgba(0.4, 0.8, 0.4, 1.0);
                    wdraw
                        .ellipse()
                        .xy(*target)
                        .radius(0.05)
                        .resolution(8.0)
                        .stroke(clr)
                        .stroke_weight(0.02)
                        .no_fill();
                    let r = 0.1;
                    wdraw
                        .line()
                        .start(*target + vec2(-r, 0.0))
                        .end(*target + vec2(r, 0.0))
                        .weight(0.02)
                        .color(clr);
                    wdraw
                        .line()
                        .start(*target + vec2(0.0, -r))
                        .end(*target + vec2(0.0, r))
                        .weight(0.02)
                        .color(clr);
                }
            }
        }

        let clr = rgba(0.8, 0.8, 0.8, 0.6);
        for w in bot.tasks.windows(2) {
            let Task::Move(a) = w[0];
            let Task::Move(b) = w[1];
            wdraw.line().start(a).end(b).weight(0.01).color(clr);
        }
    }

    // selection box
    if let (Some(start), Some(end)) = (model.client.drag_start, model.mouse_world_pos()) {
        wdraw
            .rect()
            .xy(start + (end - start) / 2.0)
            .wh(end - start)
            .color(rgba(1.0, 1.0, 1.0, 0.02));
    }

    // mouse debug info
    if let Some(pos) = model.client.mouse_position {
        let world = model.mouse_world_pos().unwrap();
        let win = app.window_rect().wh();
        let tile = model.world.tilemap.coord(world);
        let mouse_info = format!(
            "world: {}, {} | tile: {}, {} | screen: {}, {} | win: {}, {}",
            world.x, world.y, tile.x, tile.y, pos.x, pos.y, win.x, win.y
        );
        draw.text(&mouse_info)
            .color(WHITE)
            .font_size(12)
            .wh(app.main_window().rect().pad(4.0).wh())
            .align_text_bottom()
            .left_justify();
    }

    if model.client.edit_walls_mode {
        draw.text("edit walls mode")
            .color(WHITE)
            .font_size(12)
            .wh(app.main_window().rect().pad(4.0).wh())
            .align_text_bottom()
            .right_justify();
    }

    draw.to_frame(app, &frame).unwrap();
    // draw egui ontop of everything else
    model.egui.draw_to_frame(&frame).unwrap();
}

fn draw_arrow(draw: &Draw, start: Vec2, end: Vec2, thickness: f32, color: Srgba<u8>) {
    draw.line()
        .start(start)
        .end(end)
        .weight(thickness)
        .color(color);

    if start == end {
        return;
    }

    let d = (start - end) * 0.1;
    let a = mat2(vec2(0.87, 0.5), vec2(-0.5, 0.87)) * d;
    let b = mat2(vec2(0.87, -0.5), vec2(0.5, 0.87)) * d;
    draw.line()
        .start(end)
        .end(end + a)
        .weight(thickness)
        .color(color);
    draw.line()
        .start(end)
        .end(end + b)
        .weight(thickness)
        .color(color);
}

fn lerp<T>(start: T, end: T, factor: f32) -> T
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<f32, Output = T>,
{
    start + (end - start) * factor
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
