mod bot;
mod draw;
mod math;
mod meshchunks;
mod scenarios;
mod spatialgrid;
mod steer;
mod tilemap;
mod world;

use nannou::prelude::*;
use nannou::rand::SeedableRng;
use nannou::rand::rngs::SmallRng;
use nannou_egui::{
    Egui,
    egui::{self, Slider},
};

use slotmap::DefaultKey;

use crate::bot::Task;
use crate::draw::draw_world;
use crate::math::{circle_rect_intersects, lerp};
use crate::scenarios::*;
use crate::world::World;

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
        .title("crowd sim pls")
        .view(view)
        .maximized(true)
        .raw_event(raw_gui_event)
        .build()
        .unwrap();
    let window = app.window(window_id).unwrap();
    let egui = Egui::from_window(&window);

    let world_size = ivec2(256, 256);

    let settings = Settings {
        timestep: 0.02,
        use_orca: false,
        orca_time_horizon: 0.3,
        collision_resolver_iters: 1,
        collision_resolver_fraction: 1.0,
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

            Key::Key1 => scenario_simple_avoid(model),
            Key::Key2 => scenario_symmetry_avoid(model),
            Key::Key3 => scenario_symmetry_avoid2(model),
            Key::Key4 => scenario_origin_swap_n(model, 10, 5.0),
            Key::Key5 => scenario_origin_swap_n(model, 50, 10.0),
            Key::Key6 => scenario_lines_swap_n(model, 100),

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
                            let task =
                                if let Some(target) = model.world.grid.query(pos, 1.0).find(|v| {
                                    pos.distance_squared(model.world.bots[*v].position)
                                        < model.world.bots[*v].radius.powi(2)
                                }) && (model.client.selection.len() > 1
                                    || model.client.selection[0] != target)
                                {
                                    Task::Follow(target)
                                } else {
                                    Task::Move(pos)
                                };
                            for &k in &model.client.selection {
                                if let Task::Follow(other) = task
                                    && other == k
                                {
                                    continue;
                                }
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
    let draw = app.draw();

    draw_world(app, model, &draw);

    draw.to_frame(app, &frame).unwrap();

    // draw egui ontop of everything else
    model.egui.draw_to_frame(&frame).unwrap();
}
