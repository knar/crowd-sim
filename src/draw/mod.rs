pub mod meshchunks;

use std::sync::LazyLock;

use nannou::{
    App, Draw,
    color::*,
    glam::{Vec2, mat2, vec2, vec3},
};

use crate::{DebugThing, Model, Task, math::lerp};

pub static BACKGROUND_COLOR: LazyLock<Rgb<u8>> = LazyLock::new(|| rgb_u32(0x101010));
pub static GROUND_COLOR: LazyLock<Rgb<u8>> = LazyLock::new(|| rgb_u32(0x363652));
pub static WALL_COLOR: LazyLock<Rgb<u8>> = LazyLock::new(|| rgb_u32(0x545480));
pub static WALL_BORDER_COLOR: LazyLock<Rgb<u8>> = LazyLock::new(|| rgb_u32(0x7979a7));
pub static FOREGROUND_COLOR: LazyLock<Rgb<u8>> = LazyLock::new(|| rgb_u32(0xe7dfdb));

pub fn draw_world(app: &App, model: &Model, draw: &Draw) {
    let cam = &model.client.camera;
    let settings = &model.settings;
    let world = &model.world;

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

        let clr = if bot.debug_colliding {
            RED
        } else if !bot.tasks.is_empty() {
            CYAN
        } else {
            *FOREGROUND_COLOR
        };

        let thick = 0.02;
        // the circle
        wdraw
            .ellipse()
            .xy(pos)
            .radius(bot.radius - thick / 2.0)
            .resolution(32.0)
            .stroke(clr)
            .stroke_weight(thick)
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
        } else {
            wdraw
                .line()
                .start(pos)
                .end(pos + bot.dir * bot.radius)
                .weight(thick)
                .color(clr);
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

            // rally lines
            let clr = rgba(0.8, 0.8, 0.8, 0.6);
            for w in bot.tasks.windows(2) {
                let a = model.world.task_pos(w[0]);
                let b = model.world.task_pos(w[1]);
                wdraw.line().start(a).end(b).weight(0.01).color(clr);
            }
            if let Some(next) = bot.tasks.first() {
                wdraw
                    .line()
                    .start(pos)
                    .end(model.world.task_pos(*next))
                    .weight(0.01)
                    .color(clr);
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
                Task::Follow(other) => {
                    let other = &model.world.bots[*other];
                    wdraw
                        .ellipse()
                        .xy(other.position)
                        .radius(other.radius)
                        .resolution(32.0)
                        .stroke(YELLOW)
                        .stroke_weight(0.03)
                        .no_fill();
                }
            }
        }
    }

    // debug things
    if model.settings.draw_debug_lines {
        for thing in &model.debug_things {
            match *thing {
                DebugThing::Vec(offset, v, clr) => {
                    wdraw
                        .line()
                        .start(offset)
                        .end(offset + v)
                        .weight(0.04)
                        .color(clr);
                }
                DebugThing::Circle(center, r, clr) => {
                    wdraw
                        .ellipse()
                        .xy(center)
                        .radius(r)
                        .resolution(128.0)
                        .stroke(clr)
                        .stroke_weight(0.04)
                        .no_fill();
                }
                DebugThing::Point(pos, clr) => {
                    wdraw
                        .ellipse()
                        .xy(pos)
                        .radius(0.06)
                        .resolution(8.0)
                        .color(clr);
                }
                DebugThing::Arc(origin, r, start, end, clr) => {
                    let res = 20;
                    let mut a = vec2(start.cos(), start.sin()) * r;
                    for i in 1..=res {
                        let theta = lerp(start, end, i as f32 / res as f32);
                        let b = vec2(theta.cos(), theta.sin()) * r;
                        wdraw
                            .line()
                            .start(origin + a)
                            .end(origin + b)
                            .weight(0.04)
                            .color(clr);
                        a = b;
                    }
                }
                DebugThing::Text(ref s) => {
                    draw.text(s)
                        .color(WHITE)
                        .font_size(12)
                        .wh(app.main_window().rect().pad(4.0).wh())
                        .align_text_middle_y()
                        .align_text_bottom();
                }
            }
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
        let tile = model.world.tilemap.coord(world);
        let mouse_info = format!(
            "world: {:.2}, {:.2} | tile: {}, {} | screen: {:.2}, {:.2}",
            world.x, world.y, tile.x, tile.y, pos.x, pos.y,
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
}

pub fn draw_arrow(draw: &Draw, start: Vec2, end: Vec2, thickness: f32, color: Srgba<u8>) {
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
