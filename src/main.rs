mod world;

use std::ops::{Add, Mul, Sub};

use nannou::prelude::*;
use nannou_egui::{
    Egui,
    egui::{self, Slider},
};
use slotmap::DefaultKey;

use crate::world::{DebugThing, Task, World};

const INITIAL_ZOOM: f32 = 100.0;

mod theme {
    use nannou::color::{Rgb, Rgba, rgb_u32, rgba8};

    pub fn bg() -> Rgb<u8> {
        rgb_u32(0x363652)
    }

    pub fn fg() -> Rgb<u8> {
        rgb_u32(0xe7dfdb)
    }

    pub fn grid() -> Rgba<u8> {
        rgba8(0xe7, 0xdf, 0xdb, 0x11)
    }
}

fn main() {
    nannou::app(model).event(event).run();
}

struct Model {
    egui: Egui,
    settings: Settings,
    accumulator: f32,
    world: World,
    selection: Vec<DefaultKey>,
    camera: Camera,
    mouse_position: Option<Vec2>,
    drag_start: Option<Vec2>,
}

impl Model {
    fn mouse_world_pos(&self) -> Option<Vec2> {
        self.mouse_position.map(|m| self.camera.screen_to_world(m))
    }

    fn reset_world(&mut self) {
        self.selection.clear();
        self.world.bots.clear();
        self.world.debug_things.clear();

        for (x, y) in [(-2.5, -2.0), (-3.0, 0.0), (-2.0, 3.0)] {
            self.world
                .add_bot(vec2(x, y), Vec2::ZERO, Some(Task::Move(vec2(2.5, 0.0))));
        }
    }
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

    let mut model = Model {
        settings: Settings {
            timestep: 0.1,
            stopping_time: 0.15,
            spring_constant: 16.0,
            spring_distance: 0.2,
            timescale: 1.0,
            interpolate_frames: true,
            draw_debug_lines: false,
            draw_trail: false,
            paused: false,
        },
        egui,
        accumulator: 0.0,
        world: World::new(),
        selection: Vec::new(),
        mouse_position: None,
        drag_start: None,
        camera: Camera {
            position: Vec2::ZERO,
            zoom: INITIAL_ZOOM * vec2(1.0, 1.0),
        },
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
                model.accumulator += update.since_last.as_secs_f32() * model.settings.timescale;
            }
            while model.accumulator >= model.settings.timestep {
                model.accumulator -= model.settings.timestep;
                model.world.tick(&model.settings);
            }

            model.egui.set_elapsed_time(update.since_start);
            let ctx = model.egui.begin_frame();
            egui::Window::new("Settings")
                .default_pos((20.0, 20.0))
                .show(&ctx, |ui| {
                    ui.add(Slider::new(&mut model.settings.timestep, 0.01..=1.0).text("Timestep"));
                    ui.add(
                        Slider::new(&mut model.settings.stopping_time, 0.01..=0.5)
                            .text("Stopping time"),
                    );
                    ui.add(
                        Slider::new(&mut model.settings.spring_constant, 0.01..=50.0)
                            .text("Spring constant"),
                    );
                    ui.add(
                        Slider::new(&mut model.settings.spring_distance, 0.01..=0.5)
                            .text("Spring distance"),
                    );
                    ui.add(
                        Slider::new(&mut model.settings.timescale, 0.01..=5.0).text("Timescale"),
                    );
                    ui.checkbox(&mut model.settings.interpolate_frames, "Interpolate frames");
                    // ui.checkbox(&mut model.settings.draw_debug_lines, "Draw debug lines");
                    ui.checkbox(&mut model.settings.draw_trail, "Draw trail");
                    ui.checkbox(&mut model.settings.paused, "Pause");
                    if model.settings.paused && ui.button("Tick").clicked() {
                        model.world.tick(&model.settings);
                    }

                    if !model.selection.is_empty() {
                        let bot = &model.world.bots[model.selection[0]];
                        ui.label(bot.summary());
                    }
                });
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
            _ => {}
        },
        WindowEvent::MousePressed(btn) => match btn {
            MouseButton::Left => {
                model.drag_start = model.mouse_world_pos();
            }
            MouseButton::Right => {
                if let Some(pos) = model.mouse_world_pos() {
                    for &k in &model.selection {
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
        },
        WindowEvent::MouseReleased(MouseButton::Left) => {
            if let (Some(start), Some(end)) = (model.drag_start, model.mouse_world_pos()) {
                let frac = model.accumulator / model.settings.timestep;
                model.selection = model.world.bots_in_rect(start, end, frac);
                model.drag_start = None;
            }
        }
        WindowEvent::MouseMoved(pos) => {
            if app.mouse.buttons.middle().is_down()
                && let Some(last) = model.mouse_position
            {
                let d = pos - last;
                model.camera.position -= d / model.camera.zoom;
            }

            model.mouse_position = Some(pos);
        }
        WindowEvent::MouseExited => model.mouse_position = None,
        WindowEvent::MouseWheel(delta, _) => {
            let y = match delta {
                MouseScrollDelta::LineDelta(_, lines) => lines * 10.0,
                MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
            };
            let prev_pos = model.mouse_world_pos().unwrap();
            model.camera.zoom *= (y / 100.0).exp2();
            let new_pos = model.mouse_world_pos().unwrap();
            model.camera.position += prev_pos - new_pos;
        }
        _ => {}
    }
}

fn view(app: &App, model: &Model, frame: Frame) {
    let draw = app.draw();

    draw.background().color(theme::bg());

    let wdraw = draw
        .scale_x(model.camera.zoom.x)
        .scale_y(model.camera.zoom.y)
        .translate(-vec3(model.camera.position.x, model.camera.position.y, 0.0));

    // grid dots
    if model.camera.zoom.abs().min_element() > 40.0 {
        let half_win_size = (app.window_rect().wh() / 2.0).ceil();
        let min = model.camera.screen_to_world(-half_win_size).floor();
        let max = model.camera.screen_to_world(half_win_size).ceil() + Vec2::ONE;
        let size = 0.05;
        // let color = rgba(1.0, 1.0, 1.0, 0.03);
        let color = theme::grid();
        for x in min.x as i32..max.x as i32 {
            for y in min.y as i32..max.y as i32 {
                wdraw
                    .rect()
                    .x_y(x as f32, y as f32)
                    .w_h(size, size)
                    .color(color);
            }
        }
    }

    // bots
    for (k, bot) in &model.world.bots {
        bot.draw(
            &wdraw,
            model.accumulator,
            &model.settings,
            model.selection.contains(&k),
        );
    }

    // selection box
    if let (Some(start), Some(end)) = (model.drag_start, model.mouse_world_pos()) {
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
            }
        }
    }

    // mouse debug info
    if let Some(pos) = model.mouse_position {
        let world = model.mouse_world_pos().unwrap();
        let win = app.window_rect().wh();
        let mouse_info = format!(
            "screen: {}, {}  world: {}, {}  win: {}, {}",
            pos.x, pos.y, world.x, world.y, win.x, win.y
        );
        draw.text(&mouse_info)
            .color(WHITE)
            .font_size(12)
            .wh(app.main_window().rect().pad(4.0).wh())
            .align_text_bottom()
            .left_justify();
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
