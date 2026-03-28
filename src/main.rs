mod meshchunks;
mod tilemap;
mod world;

use std::ops::{Add, Mul, Sub};
use std::sync::LazyLock;

use nannou::color::{Rgb, rgb_u32, rgba8};
use nannou::prelude::*;
use nannou_egui::{
    Egui,
    egui::{self, Slider},
};

use slotmap::DefaultKey;

use crate::world::{DebugThing, Task, World};

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
        self.world.debug_things.clear();

        // for (x, y) in [(-3.5, -2.0), (-3.0, 0.0), (-2.0, 3.0)] {
        //     self.world
        //         .add_bot(vec2(x, y), Vec2::ZERO, Some(Task::Move(vec2(5.5, 4.1))));
        // }
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
    stopping_time: f32,
    spring_constant: f32,
    spring_distance: f32,
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
        .title("its just boids bruv")
        .view(view)
        .raw_event(raw_gui_event)
        .build()
        .unwrap();
    let window = app.window(window_id).unwrap();
    let egui = Egui::from_window(&window);

    let world_size = ivec2(240, 160);

    let settings = Settings {
        timestep: 0.05,
        stopping_time: 0.15,
        spring_constant: 16.0,
        spring_distance: 0.2,
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
            zoom: Vec2::splat(32.0),
        },
        edit_walls_mode: false,
    };

    let mut model = Model {
        egui,
        settings,
        client,
        world: World::new(world_size),
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
                model.world.tick(&model.settings);
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
            ui.add(Slider::new(&mut model.settings.timestep, 0.01..=1.0).text("Timestep"));
            ui.add(
                Slider::new(&mut model.settings.stopping_time, 0.01..=0.5).text("Stopping time"),
            );
            ui.add(
                Slider::new(&mut model.settings.spring_constant, 0.01..=50.0)
                    .text("Spring constant"),
            );
            ui.add(
                Slider::new(&mut model.settings.spring_distance, 0.01..=0.5)
                    .text("Spring distance"),
            );
            ui.add(Slider::new(&mut model.settings.timescale, 0.01..=5.0).text("Timescale"));
            ui.checkbox(&mut model.settings.interpolate_frames, "Interpolate frames");
            ui.checkbox(&mut model.settings.draw_head_dot, "Draw head dot");
            ui.checkbox(&mut model.settings.draw_debug_lines, "Draw debug lines");
            ui.checkbox(&mut model.settings.draw_trail, "Draw trail");
            ui.checkbox(&mut model.settings.paused, "Pause");
            if model.settings.paused && ui.button("Tick").clicked() {
                model.world.tick(&model.settings);
            }

            if !model.client.selection.is_empty() {
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
                model.world.tick(&model.settings);
            }
            Key::S => {
                if let Some(pos) = model.mouse_world_pos() {
                    model.world.add_bot(pos, Vec2::ZERO, None);
                }
            }
            Key::R => {
                model.reset_world();
            }
            Key::D => {
                model.settings.draw_debug_lines = !model.settings.draw_debug_lines;
            }
            Key::M => {
                model.client.edit_walls_mode = !model.client.edit_walls_mode;
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
    for (k, bot) in &world.bots {
        let pos = if !settings.paused && settings.interpolate_frames {
            lerp(bot.prev_pos(), bot.position, frac)
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

        // the circle!
        wdraw
            .ellipse()
            .xy(pos)
            .radius(bot.radius - 0.02)
            .resolution(64.0)
            .stroke(*FOREGROUND_COLOR)
            .stroke_weight(0.04)
            .no_fill();

        if model.settings.draw_head_dot {
            let prev_vel = (bot.prev_pos() - bot.prev_prev_pos()) / model.settings.timestep;
            let lean = lerp(prev_vel, bot.velocity, frac) / bot.max_speed;

            wdraw
                .ellipse()
                .xy(pos + lean * bot.radius * 0.6)
                .radius(bot.radius * 0.4)
                .resolution(64.0)
                .color(*FOREGROUND_COLOR);
        }

        if model.client.selection.contains(&k) {
            wdraw
                .ellipse()
                .xy(pos)
                .radius(bot.radius)
                .resolution(64.0)
                .stroke(rgba(0.4, 0.8, 0.4, 1.0))
                .stroke_weight(0.03)
                .no_fill();
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

    if model.settings.draw_debug_lines {
        for &(thing, color) in &model.world.debug_things {
            match thing {
                DebugThing::Point(p) => {
                    wdraw.rect().xy(p).w_h(0.1, 0.1).color(color);
                }
                DebugThing::Vec(p, v) => {
                    draw_arrow(&wdraw, p, p + v, 0.03, color);
                }
                DebugThing::Arrow(a, b) => {
                    draw_arrow(&wdraw, a, b, 0.03, color);
                }
            }
        }
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
