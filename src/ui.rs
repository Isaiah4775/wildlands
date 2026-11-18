//! HUD, resource counters, 12-slot hotbar, minimap, mastery tech tree and menus.

use macroquad::prelude::*;
use std::f32::consts::PI;

use crate::building::{BuildKind, BuildingManager, Ghost};
use crate::player::{
    hotbar, Player, SlotItem, MELEE_TIERS, RANGED_TIERS, RIFLE_BASE_DAMAGE, SHOTGUN_PELLET_DAMAGE,
};
use crate::progression::{
    Progression, UnlockId, MELEE_THRESHOLDS, RANGED_THRESHOLDS,
    RANGED_UNLOCK_MELEE_LEVEL,
};
use crate::world::{
    draw_text_center, rot_vec, text_px, text_width, with_alpha, World, OUTLINE, WORLD_SIZE,
};

pub const GOLD: Color = Color::new(0.98, 0.82, 0.36, 1.0);
const PANEL: Color = Color::new(0.05, 0.07, 0.10, 0.90);
const GREEN: Color = Color::new(0.42, 0.95, 0.52, 1.0);
const RED: Color = Color::new(1.0, 0.36, 0.34, 1.0);
const CYAN: Color = Color::new(0.55, 0.93, 1.0, 1.0);

pub fn in_rect(p: Vec2, rect: (f32, f32, f32, f32)) -> bool {
    let (x, y, w, h) = rect;
    p.x >= x && p.x <= x + w && p.y >= y && p.y <= y + h
}

fn draw_panel(x: f32, y: f32, w: f32, h: f32, fill: Color) {
    draw_rectangle(x, y, w, h, fill);
    draw_rectangle_lines(x, y, w, h, 2.0, Color::new(1.0, 1.0, 1.0, 0.10));
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, fill: Color) {
    draw_rectangle(x - 2.0, y - 2.0, w + 4.0, h + 4.0, Color::new(0.0, 0.0, 0.0, 0.55));
    draw_rectangle(x, y, w, h, Color::new(0.12, 0.13, 0.17, 1.0));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, fill);
    draw_rectangle_lines(x, y, w, h, 1.0, Color::new(1.0, 1.0, 1.0, 0.10));
}

/// High-contrast text with a one-pixel drop shadow so every letter stays
/// legible over grass, panels and busy combat.
fn label(text: &str, x: f32, y: f32, size: f32, color: Color) {
    let size = text_px(size);
    let stroke = (size * 0.09).clamp(1.5, 3.5);
    let outline = Color::new(0.0, 0.0, 0.0, 0.95);
    for (dx, dy) in [
        (-1.0, 0.0),
        (1.0, 0.0),
        (0.0, -1.0),
        (0.0, 1.0),
        (-1.0, -1.0),
        (1.0, -1.0),
        (-1.0, 1.0),
        (1.0, 1.0),
    ] {
        draw_text(text, x + dx * stroke, y + dy * stroke, size, outline);
    }
    draw_text(text, x, y, size, color);
}

fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0) as i32;
    format!("{}:{:02}", total / 60, total % 60)
}

// ---------------------------------------------------------------------------
// Hotbar
// ---------------------------------------------------------------------------

fn hotbar_geometry() -> (f32, f32, f32, f32) {
    let slot = 48.0;
    let gap = 6.0;
    let count = hotbar().len() as f32;
    let total = count * slot + (count - 1.0) * gap;
    let x0 = screen_width() * 0.5 - total * 0.5;
    let y0 = screen_height() - slot - 22.0;
    (slot, gap, x0, y0)
}

pub fn hotbar_slot_at(mouse: Vec2) -> Option<usize> {
    let (slot, gap, x0, y0) = hotbar_geometry();
    for i in 0..hotbar().len() {
        let x = x0 + i as f32 * (slot + gap);
        if in_rect(mouse, (x, y0, slot, slot)) {
            return Some(i);
        }
    }
    None
}

fn slot_key_label(i: usize) -> &'static str {
    match i {
        0 => "1",
        1 => "2",
        2 => "3",
        3 => "4",
        4 => "5",
        5 => "6",
        6 => "7",
        7 => "8",
        8 => "9",
        9 => "0",
        10 => "-",
        11 => "=",
        12 => "[",
        _ => "]",
    }
}

fn draw_lock(cx: f32, cy: f32) {
    let c = Color::new(0.95, 0.82, 0.45, 0.95);
    draw_line(cx - 6.0, cy - 4.0, cx - 6.0, cy - 9.0, 2.4, c);
    draw_line(cx + 6.0, cy - 4.0, cx + 6.0, cy - 9.0, 2.4, c);
    draw_line(cx - 6.0, cy - 9.0, cx + 6.0, cy - 9.0, 2.4, c);
    draw_rectangle(cx - 8.0, cy - 4.0, 16.0, 13.0, Color::new(0.06, 0.06, 0.08, 0.9));
    draw_rectangle_lines(cx - 8.0, cy - 4.0, 16.0, 13.0, 2.0, c);
    draw_circle(cx, cy + 1.0, 1.8, c);
}

fn melee_color(level: usize) -> Color {
    match level {
        1..=2 => Color::new(0.62, 0.46, 0.26, 1.0),
        3..=4 => Color::new(0.70, 0.73, 0.76, 1.0),
        5..=6 => Color::new(0.80, 0.84, 0.88, 1.0),
        7..=8 => CYAN,
        _ => GOLD,
    }
}

fn draw_melee_icon(level: usize, c: Vec2, fade: f32) {
    let blade = with_alpha(melee_color(level), fade);
    let brown = with_alpha(Color::new(0.55, 0.38, 0.21, 1.0), fade);
    if level == 1 {
        draw_line(c.x - 9.0, c.y + 9.0, c.x + 9.0, c.y - 9.0, 7.0, OUTLINE);
        draw_line(c.x - 8.0, c.y + 8.0, c.x + 7.0, c.y - 7.0, 4.0, brown);
        draw_circle(c.x + 9.0, c.y - 9.0, 5.0, OUTLINE);
        draw_circle(c.x + 9.0, c.y - 9.0, 3.4, blade);
        return;
    }
    draw_line(c.x - 8.0, c.y + 11.0, c.x + 3.0, c.y - 2.0, 5.0, OUTLINE);
    draw_line(c.x - 7.0, c.y + 10.0, c.x + 2.0, c.y - 1.0, 3.0, brown);
    let tip_len = 8.0 + (level as f32) * 0.7;
    draw_triangle(
        vec2(c.x + 3.0, c.y - 2.0),
        vec2(c.x + 7.0, c.y - tip_len),
        vec2(c.x - 4.0, c.y - 6.0),
        OUTLINE,
    );
    draw_triangle(
        vec2(c.x + 3.0, c.y - 3.0),
        vec2(c.x + 6.0, c.y - tip_len + 2.0),
        vec2(c.x - 3.0, c.y - 6.0),
        blade,
    );
}

fn draw_bow_icon(level: usize, c: Vec2, fade: f32) {
    let col = match level {
        0 | 1 => Color::new(0.72, 0.52, 0.28, 1.0),
        2 => Color::new(0.56, 0.78, 0.54, 1.0),
        3 => Color::new(0.72, 0.78, 0.84, 1.0),
        4 => CYAN,
        _ => GOLD,
    };
    let col = with_alpha(col, fade);
    draw_arc(c.x, c.y, 10, 14.0, 300.0, 3.0, 120.0, OUTLINE);
    draw_arc(c.x, c.y, 10, 12.0, 300.0, 2.0, 120.0, col);
    let p1 = c + vec2((300.0f32).to_radians().cos(), (300.0f32).to_radians().sin()) * 12.0;
    let p2 = c + vec2((60.0f32).to_radians().cos(), (60.0f32).to_radians().sin()) * 12.0;
    draw_line(p1.x, p1.y, p2.x, p2.y, 1.4, Color::new(0.90, 0.90, 0.82, fade));
    draw_line(c.x, c.y, c.x + 15.0, c.y, 2.0, with_alpha(GOLD, fade));
}

fn draw_gun_icon(c: Vec2, fade: f32, shotgun: bool) {
    let dark = with_alpha(Color::new(0.28, 0.31, 0.36, 1.0), fade);
    let metal = with_alpha(Color::new(0.58, 0.63, 0.70, 1.0), fade);
    let wood = with_alpha(Color::new(0.54, 0.37, 0.20, 1.0), fade);
    let dir = vec2(0.84, -0.54);
    let back = c - dir * 14.0;
    let front = c + dir * (if shotgun { 10.0 } else { 15.0 });
    draw_line(back.x, back.y, front.x, front.y, 8.0, OUTLINE);
    draw_line(back.x, back.y, front.x, front.y, 5.0, dark);
    draw_line(c.x, c.y, front.x, front.y, 4.0, OUTLINE);
    draw_line(c.x, c.y, front.x, front.y, 2.2, metal);
    draw_rot_rect_icon(back - dir * 3.0, 12.0, 8.0, -0.57, wood);
    if shotgun {
        draw_rot_rect_icon(c + dir * 5.0, 12.0, 7.0, -0.57, wood);
    } else {
        draw_line(
            c.x - dir.y * 2.0,
            c.y + dir.x * 2.0,
            c.x + dir.x * 8.0 - dir.y * 4.0,
            c.y + dir.y * 8.0 + dir.x * 4.0,
            2.5,
            dark,
        );
    }
    draw_circle(front.x, front.y, 3.0, with_alpha(GOLD, fade));
}

fn build_color(kind: BuildKind) -> Color {
    match kind {
        BuildKind::WoodWall => Color::new(0.55, 0.38, 0.21, 1.0),
        BuildKind::StoneWall => Color::new(0.58, 0.61, 0.64, 1.0),
        BuildKind::MetalWall => Color::new(0.38, 0.48, 0.58, 1.0),
        BuildKind::Door => Color::new(0.64, 0.46, 0.26, 1.0),
        BuildKind::StoneGate => Color::new(0.66, 0.69, 0.72, 1.0),
        BuildKind::MetalGate => Color::new(0.45, 0.68, 0.78, 1.0),
        BuildKind::SpikeTrap => Color::new(0.42, 0.44, 0.48, 1.0),
        BuildKind::MetalSpike => Color::new(0.36, 0.55, 0.62, 1.0),
        BuildKind::Turret => Color::new(0.46, 0.49, 0.55, 1.0),
        BuildKind::Campfire => Color::new(0.95, 0.55, 0.22, 1.0),
    }
}

fn draw_build_icon(kind: BuildKind, c: Vec2, fade: f32) {
    let col = with_alpha(build_color(kind), fade);
    match kind {
        BuildKind::WoodWall | BuildKind::StoneWall | BuildKind::MetalWall => {
            draw_rectangle(c.x - 10.0, c.y - 10.0, 20.0, 20.0, OUTLINE);
            draw_rectangle(c.x - 8.0, c.y - 8.0, 16.0, 16.0, col);
            if kind == BuildKind::WoodWall {
                for i in -1..=1 {
                    draw_line(
                        c.x - 8.0,
                        c.y + i as f32 * 5.0,
                        c.x + 8.0,
                        c.y + i as f32 * 5.0,
                        1.6,
                        with_alpha(Color::new(0.34, 0.22, 0.11, 1.0), fade),
                    );
                }
            } else if kind == BuildKind::StoneWall {
                for (ox, oy) in [(-4.0, -4.0), (4.0, -4.0), (-4.0, 4.0), (4.0, 4.0)] {
                    draw_rectangle(
                        c.x + ox - 3.0,
                        c.y + oy - 3.0,
                        6.0,
                        6.0,
                        with_alpha(Color::new(0.72, 0.75, 0.77, 1.0), fade),
                    );
                }
            } else {
                draw_line(c.x - 7.0, c.y - 7.0, c.x + 7.0, c.y + 7.0, 2.0, OUTLINE);
                draw_line(c.x + 7.0, c.y - 7.0, c.x - 7.0, c.y + 7.0, 2.0, OUTLINE);
                for (ox, oy) in [(-7.0, -7.0), (7.0, -7.0), (-7.0, 7.0), (7.0, 7.0)] {
                    draw_circle(c.x + ox, c.y + oy, 1.8, with_alpha(CYAN, fade));
                }
            }
        }
        BuildKind::Door | BuildKind::StoneGate | BuildKind::MetalGate => {
            draw_rectangle(c.x - 10.0, c.y - 11.0, 20.0, 22.0, OUTLINE);
            draw_rectangle(c.x - 8.0, c.y - 9.0, 16.0, 18.0, col);
            draw_rectangle_lines(
                c.x - 3.0,
                c.y - 9.0,
                6.0,
                18.0,
                1.5,
                with_alpha(Color::new(0.25, 0.20, 0.16, 0.9), fade),
            );
            draw_circle(c.x + 4.0, c.y + 1.0, 1.8, with_alpha(GOLD, fade));
        }
        BuildKind::SpikeTrap | BuildKind::MetalSpike => {
            draw_rectangle(c.x - 10.0, c.y - 10.0, 20.0, 20.0, OUTLINE);
            draw_rectangle(c.x - 8.0, c.y - 8.0, 16.0, 16.0, col);
            let spike = if kind == BuildKind::MetalSpike {
                with_alpha(CYAN, fade)
            } else {
                with_alpha(Color::new(0.82, 0.84, 0.86, 1.0), fade)
            };
            for (ox, oy) in [(-4.5, 2.0), (4.0, 2.0), (0.0, -2.0)] {
                draw_poly(c.x + ox, c.y + oy, 3, 4.5, -PI * 0.5, OUTLINE);
                draw_poly(c.x + ox, c.y + oy, 3, 3.2, -PI * 0.5, spike);
            }
        }
        BuildKind::Turret => {
            draw_line(c.x - 2.0, c.y - 2.0, c.x + 11.0, c.y - 11.0, 6.0, OUTLINE);
            draw_line(
                c.x - 2.0,
                c.y - 2.0,
                c.x + 10.0,
                c.y - 10.0,
                3.0,
                with_alpha(Color::new(0.68, 0.71, 0.74, 1.0), fade),
            );
            draw_circle(c.x - 2.0, c.y + 2.0, 9.0, OUTLINE);
            draw_circle(c.x - 2.0, c.y + 2.0, 7.0, col);
            draw_circle(c.x - 2.0, c.y + 2.0, 3.0, with_alpha(GOLD, fade));
        }
        BuildKind::Campfire => {
            draw_line(c.x - 9.0, c.y + 8.0, c.x + 9.0, c.y + 8.0, 5.0, OUTLINE);
            draw_line(
                c.x - 8.0,
                c.y + 7.0,
                c.x + 8.0,
                c.y + 7.0,
                2.5,
                with_alpha(Color::new(0.50, 0.32, 0.17, 1.0), fade),
            );
            draw_poly(c.x, c.y + 2.0, 3, 10.0, -PI * 0.5, OUTLINE);
            draw_poly(c.x, c.y + 3.0, 3, 8.0, -PI * 0.5, col);
            draw_poly(
                c.x,
                c.y + 5.0,
                3,
                4.0,
                -PI * 0.5,
                with_alpha(Color::new(1.0, 0.90, 0.44, 1.0), fade),
            );
        }
    }
}

fn draw_item_icon(
    item: SlotItem,
    progress: &Progression,
    player: &Player,
    c: Vec2,
    unlocked: bool,
) {
    let fade = if unlocked { 1.0 } else { 0.55 };
    match item {
        SlotItem::Melee => draw_melee_icon(player.melee_level, c, fade),
        SlotItem::Ranged => draw_bow_icon(player.ranged_level, c, fade),
        SlotItem::AutoRifle => draw_gun_icon(c, fade, false),
        SlotItem::Shotgun => draw_gun_icon(c, fade, true),
        SlotItem::Food => {
            draw_circle(c.x - 5.0, c.y + 2.0, 6.0, OUTLINE);
            draw_circle(
                c.x - 5.0,
                c.y + 2.0,
                4.4,
                with_alpha(Color::new(0.90, 0.26, 0.33, 1.0), fade),
            );
            draw_circle(c.x + 5.0, c.y + 4.0, 5.5, OUTLINE);
            draw_circle(
                c.x + 5.0,
                c.y + 4.0,
                4.0,
                with_alpha(Color::new(0.96, 0.36, 0.38, 1.0), fade),
            );
            draw_circle(c.x, c.y - 5.0, 5.5, OUTLINE);
            draw_circle(
                c.x,
                c.y - 5.0,
                4.0,
                with_alpha(Color::new(0.82, 0.20, 0.28, 1.0), fade),
            );
            draw_rot_rect_icon(
                vec2(c.x + 5.0, c.y - 11.0),
                9.0,
                4.0,
                -0.6,
                with_alpha(Color::new(0.36, 0.72, 0.34, 1.0), fade),
            );
        }
        SlotItem::Build(_) => {
            let kind = item.build_kind(progress).unwrap_or(BuildKind::WoodWall);
            draw_build_icon(kind, c, fade);
        }
    }
}

fn draw_rot_rect_icon(center: Vec2, w: f32, h: f32, angle: f32, color: Color) {
    let a = center + rot_vec(vec2(-w * 0.5, -h * 0.5), angle);
    let b = center + rot_vec(vec2(w * 0.5, -h * 0.5), angle);
    let c = center + rot_vec(vec2(w * 0.5, h * 0.5), angle);
    let d = center + rot_vec(vec2(-w * 0.5, h * 0.5), angle);
    draw_triangle(a, b, c, color);
    draw_triangle(a, c, d, color);
}

// ---------------------------------------------------------------------------
// HUD
// ---------------------------------------------------------------------------

fn draw_stats(player: &Player, elapsed: f32, grid_lock: bool) {
    let x = 24.0;
    let y = 24.0;
    let w = 338.0;
    // Taller than before: bigger type needs room to breathe under the bars.
    let h = 166.0;
    draw_panel(x, y, w, h, PANEL);
    draw_rectangle(x, y, 4.0, h, GOLD);

    draw_rectangle(x + 18.0, y + 36.0, 20.0, 7.0, RED);
    draw_rectangle(x + 24.5, y + 29.5, 7.0, 20.0, RED);
    label("HEALTH", x + 50.0, y + 31.0, 13.0, Color::new(0.86, 0.90, 0.94, 1.0));
    let hp_frac = player.health / player.max_health;
    let hp_fill = if hp_frac > 0.55 {
        Color::new(0.36, 0.85, 0.46, 1.0)
    } else if hp_frac > 0.25 {
        Color::new(0.95, 0.72, 0.28, 1.0)
    } else {
        RED
    };
    bar(x + 50.0, y + 39.0, 216.0, 15.0, hp_frac, hp_fill);
    let hp_text = format!("{:.0}/{:.0}", player.health.ceil(), player.max_health);
    // Keep the readout on the HEALTH line: the bar itself is too narrow to hold
    // bigger type without the numbers running across the fill.
    label(
        &hp_text,
        x + w - 18.0 - text_width(&hp_text, 12.0),
        y + 31.0,
        12.0,
        Color::new(0.96, 1.0, 0.96, 1.0),
    );

    draw_circle(x + 27.0, y + 80.0, 9.0, OUTLINE);
    draw_circle(x + 27.0, y + 80.0, 7.0, Color::new(0.85, 0.58, 0.22, 1.0));
    draw_line(x + 32.0, y + 75.0, x + 39.0, y + 70.0, 3.5, OUTLINE);
    label("HUNGER", x + 50.0, y + 73.0, 13.0, Color::new(0.86, 0.90, 0.94, 1.0));
    bar(
        x + 50.0,
        y + 81.0,
        216.0,
        13.0,
        player.hunger / player.max_hunger,
        Color::new(0.92, 0.62, 0.24, 1.0),
    );

    let survival = format!(
        "{}  ·  MELEE Lv {}  ·  RANGED {}",
        format_time(elapsed),
        player.melee_level,
        if player.ranged_level == 0 {
            "locked".to_string()
        } else {
            format!("Lv {}", player.ranged_level)
        }
    );
    label(
        &survival,
        x + 18.0,
        y + 124.0,
        12.0,
        Color::new(0.86, 0.89, 0.92, 1.0),
    );
    let enemy_scale = 1.0
        + (player.melee_level.max(player.ranged_level).max(1) - 1) as f32 * 0.18;
    let grid = format!(
        "{}  ·  FOE HP x{:.2}  ·  ARM {:.0}%",
        if grid_lock { "GRID ON [G]" } else { "GRID OFF [G]" },
        enemy_scale,
        player.armor_fraction() * 100.0
    );
    label(
        &grid,
        x + 18.0,
        y + h - 12.0,
        12.0,
        if grid_lock { GOLD } else { Color::new(0.78, 0.81, 0.85, 1.0) },
    );
}

fn draw_resources(player: &Player) {
    let w = 216.0;
    let row_h = 32.0;
    let rows = [
        ("Wood", player.wood, Color::new(0.74, 0.52, 0.28, 1.0)),
        ("Stone", player.stone, Color::new(0.74, 0.77, 0.80, 1.0)),
        ("Metal", player.metal, Color::new(0.48, 0.88, 0.94, 1.0)),
        ("Food", player.food, Color::new(0.94, 0.38, 0.42, 1.0)),
    ];
    let h = row_h * rows.len() as f32 + 18.0;
    let x = screen_width() - w - 24.0;
    let y = screen_height() - h - 24.0;
    draw_panel(x, y, w, h, PANEL);
    draw_rectangle(x, y, 4.0, h, GOLD);
    for (i, (name, value, color)) in rows.iter().enumerate() {
        let cy = y + 9.0 + i as f32 * row_h + row_h * 0.5;
        draw_circle(x + 23.0, cy, 9.5, OUTLINE);
        draw_circle(x + 23.0, cy, 7.5, *color);
        label(name, x + 42.0, cy + 6.0, 16.0, Color::new(0.94, 0.96, 0.98, 1.0));
        let val = format!("{:.0}", value);
        label(
            &val,
            x + w - 16.0 - text_width(&val, 18.0),
            cy + 7.0,
            18.0,
            Color::new(1.0, 1.0, 0.96, 1.0),
        );
    }
}

fn draw_minimap(world: &World, buildings: &BuildingManager, player: &Player) {
    let size = 180.0;
    let x = screen_width() - size - 24.0;
    let y = 24.0;
    draw_panel(x, y, size, size, Color::new(0.05, 0.07, 0.10, 0.92));
    let inner = size - 16.0;
    let ix = x + 8.0;
    let iy = y + 8.0;
    draw_rectangle(ix, iy, inner, inner, Color::new(0.13, 0.26, 0.15, 1.0));
    let s = inner / WORLD_SIZE;

    for n in world.nodes.iter() {
        let p = vec2(ix + n.pos.x * s, iy + n.pos.y * s);
        draw_rectangle(p.x - 1.0, p.y - 1.0, 2.0, 2.0, with_alpha(n.kind.resource().color(), 0.9));
    }
    for b in buildings.items.iter() {
        let p = vec2(ix + b.pos.x * s, iy + b.pos.y * s);
        draw_rectangle(p.x - 1.5, p.y - 1.5, 3.0, 3.0, GOLD);
    }
    for e in world.enemies.iter() {
        let p = vec2(ix + e.pos.x * s, iy + e.pos.y * s);
        draw_circle(p.x, p.y, 2.2, Color::new(1.0, 0.28, 0.28, 1.0));
    }
    for a in world.arrows.iter() {
        let p = vec2(ix + a.pos.x * s, iy + a.pos.y * s);
        draw_rectangle(p.x - 1.0, p.y - 1.0, 2.0, 2.0, CYAN);
    }

    let pp = vec2(ix + player.pos.x * s, iy + player.pos.y * s);
    let d = vec2(player.aim.cos(), player.aim.sin()) * 7.0;
    draw_triangle(
        pp + d,
        pp + rot_vec(d, 2.5) * 0.55,
        pp + rot_vec(d, -2.5) * 0.55,
        WHITE,
    );

    draw_rectangle_lines(ix, iy, inner, inner, 1.5, Color::new(0.0, 0.0, 0.0, 0.55));
    label("MAP", x + 12.0, y + size - 8.0, 12.0, Color::new(0.84, 0.89, 0.84, 0.95));
}

fn draw_hotbar(player: &Player, progress: &Progression, mouse: Vec2) {
    let (slot, gap, x0, y0) = hotbar_geometry();
    let count = hotbar().len() as f32;
    let total = count * slot + (count - 1.0) * gap;
    draw_rectangle(x0 - 10.0, y0 - 10.0, total + 20.0, slot + 20.0, Color::new(0.04, 0.06, 0.09, 0.88));
    draw_rectangle_lines(
        x0 - 10.0,
        y0 - 10.0,
        total + 20.0,
        slot + 20.0,
        2.0,
        Color::new(1.0, 1.0, 1.0, 0.10),
    );

    let items = hotbar();
    for (i, item) in items.iter().enumerate() {
        let x = x0 + i as f32 * (slot + gap);
        let selected = i == player.selected;
        let y = if selected { y0 - 6.0 } else { y0 };
        let unlocked = item.unlocked(progress);
        let hover = in_rect(mouse, (x, y, slot, slot));

        let bg = if selected {
            Color::new(0.22, 0.20, 0.13, 0.96)
        } else {
            Color::new(0.10, 0.13, 0.18, 0.94)
        };
        draw_rectangle(x, y, slot, slot, bg);
        if selected {
            draw_rectangle(x, y, slot, 3.0, GOLD);
            draw_rectangle_lines(x, y, slot, slot, 3.0, GOLD);
        } else {
            draw_rectangle_lines(
                x,
                y,
                slot,
                slot,
                2.0,
                if hover {
                    Color::new(0.8, 0.85, 0.9, 0.75)
                } else {
                    Color::new(0.0, 0.0, 0.0, 0.55)
                },
            );
        }

        draw_item_icon(
            *item,
            progress,
            player,
            vec2(x + slot * 0.5, y + slot * 0.5 + 1.0),
            unlocked,
        );
        label(
            slot_key_label(i),
            x + 6.0,
            y + 17.0,
            14.0,
            if selected { GOLD } else { Color::new(0.86, 0.89, 0.93, 0.95) },
        );

        match item {
            SlotItem::Melee => {
                let lv = format!("Lv{}", player.melee_level);
                label(
                    &lv,
                    x + slot - 5.0 - text_width(&lv, 12.0),
                    y + slot - 6.0,
                    12.0,
                    GOLD,
                );
            }
            SlotItem::Ranged if player.ranged_level > 0 => {
                let lv = format!("Lv{}", player.ranged_level);
                label(
                    &lv,
                    x + slot - 5.0 - text_width(&lv, 12.0),
                    y + slot - 6.0,
                    12.0,
                    CYAN,
                );
            }
            _ => {}
        }

        if !unlocked {
            draw_rectangle(x, y, slot, slot, Color::new(0.02, 0.02, 0.03, 0.58));
            draw_lock(x + slot * 0.5, y + slot * 0.5 + 2.0);
        }
    }
}

pub fn pause_button_rect() -> (f32, f32, f32, f32) {
    let w = 104.0;
    let h = 34.0;
    (screen_width() - w - 24.0, 226.0, w, h)
}

fn draw_pause_button(mouse: Vec2) {
    let (x, y, w, h) = pause_button_rect();
    let hover = in_rect(mouse, (x, y, w, h));
    draw_panel(
        x,
        y,
        w,
        h,
        if hover {
            Color::new(0.16, 0.20, 0.26, 0.96)
        } else {
            Color::new(0.06, 0.09, 0.13, 0.92)
        },
    );
    draw_rectangle(x, y, 3.0, h, GOLD);
    draw_rectangle(x + 14.0, y + 10.0, 5.0, 14.0, GOLD);
    draw_rectangle(x + 23.0, y + 10.0, 5.0, 14.0, GOLD);
    label(
        "PAUSE",
        x + 38.0,
        y + 23.0,
        14.0,
        Color::new(0.96, 0.97, 0.94, 1.0),
    );
}

pub fn resume_button_rect() -> (f32, f32, f32, f32) {
    let w = 320.0;
    let h = 56.0;
    (
        (screen_width() - w) * 0.5,
        screen_height() * 0.5 - 90.0,
        w,
        h,
    )
}

pub fn save_button_rect() -> (f32, f32, f32, f32) {
    let w = 320.0;
    (
        (screen_width() - w) * 0.5,
        screen_height() * 0.5 - 28.0,
        w,
        38.0,
    )
}

pub fn pause_quit_rect() -> (f32, f32, f32, f32) {
    let w = 320.0;
    (
        (screen_width() - w) * 0.5,
        screen_height() * 0.5 + 20.0,
        w,
        38.0,
    )
}

pub fn damage_toggle_rect() -> (f32, f32, f32, f32) {
    (
        screen_width() * 0.5 - 160.0,
        screen_height() * 0.5 + 64.0,
        154.0,
        36.0,
    )
}

pub fn shake_toggle_rect() -> (f32, f32, f32, f32) {
    (
        screen_width() * 0.5 + 6.0,
        screen_height() * 0.5 + 64.0,
        154.0,
        36.0,
    )
}

pub fn draw_pause_overlay(
    mouse: Vec2,
    damage_numbers: bool,
    screen_shake: bool,
    notice: &str,
) {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.01, 0.02, 0.04, 0.62));

    let pw = 500.0;
    let ph = 400.0;
    let px = sw * 0.5 - pw * 0.5;
    let py = sh * 0.5 - ph * 0.5;
    draw_panel(px, py, pw, ph, Color::new(0.05, 0.07, 0.11, 0.97));
    draw_rectangle(px, py, pw, 5.0, GOLD);

    draw_text_center(
        "PAUSED",
        sw * 0.5,
        py + 60.0,
        54.0,
        GOLD,
        Some(OUTLINE),
    );
    draw_text_center(
        "Simulation frozen. Your base and the horde are safe.",
        sw * 0.5,
        py + 90.0,
        15.0,
        Color::new(0.86, 0.90, 0.92, 1.0),
        Some(OUTLINE),
    );

    let resume = resume_button_rect();
    menu_button(resume, "RESUME", in_rect(mouse, resume), true, 28.0);

    let save = save_button_rect();
    menu_button(save, "SAVE GAME", in_rect(mouse, save), false, 18.0);

    let quit = pause_quit_rect();
    menu_button(quit, "SAVE & QUIT TO MENU", in_rect(mouse, quit), false, 17.0);

    let dmg = damage_toggle_rect();
    let dmg_hover = in_rect(mouse, dmg);
    draw_panel(
        dmg.0,
        dmg.1,
        dmg.2,
        dmg.3,
        if dmg_hover {
            Color::new(0.16, 0.22, 0.24, 1.0)
        } else {
            Color::new(0.07, 0.11, 0.13, 1.0)
        },
    );
    draw_text_center(
        &format!("DAMAGE NUMBERS: {}", if damage_numbers { "ON" } else { "OFF" }),
        dmg.0 + dmg.2 * 0.5,
        dmg.1 + dmg.3 * 0.5,
        12.0,
        if damage_numbers { GOLD } else { Color::new(0.84, 0.88, 0.92, 1.0) },
        None,
    );

    let shake = shake_toggle_rect();
    let shake_hover = in_rect(mouse, shake);
    draw_panel(
        shake.0,
        shake.1,
        shake.2,
        shake.3,
        if shake_hover {
            Color::new(0.16, 0.22, 0.24, 1.0)
        } else {
            Color::new(0.07, 0.11, 0.13, 1.0)
        },
    );
    draw_text_center(
        &format!("SCREEN SHAKE: {}", if screen_shake { "ON" } else { "OFF" }),
        shake.0 + shake.2 * 0.5,
        shake.1 + shake.3 * 0.5,
        12.0,
        if screen_shake { GOLD } else { Color::new(0.84, 0.88, 0.92, 1.0) },
        None,
    );

    if !notice.is_empty() {
        draw_text_center(
            notice,
            sw * 0.5,
            py + ph - 66.0,
            13.0,
            Color::new(0.72, 0.92, 0.74, 1.0),
            Some(OUTLINE),
        );
    }
    draw_text_center(
        "ESC / P / ENTER resumes  ·  N damage numbers  ·  B shake",
        sw * 0.5,
        py + ph - 38.0,
        12.0,
        Color::new(0.92, 0.94, 0.90, 1.0),
        Some(OUTLINE),
    );
}

fn draw_item_info(
    player: &Player,
    progress: &Progression,
    buildings: &BuildingManager,
    ghost: Option<&Ghost>,
) {
    let item = player.selected_item();
    let text = match item {
        SlotItem::Melee => {
            let m = player.melee();
            format!(
                "{}   ·   Lv {}   ·   {:.0} dmg   ·   reach {:.0}   ·   gather x{:.1}",
                m.name, player.melee_level, m.damage, m.reach, m.gather
            )
        }
        SlotItem::Ranged => {
            if player.ranged_level == 0 {
                format!(
                    "Ranged locked   ·   reach Melee Lv {}   ·   Tab for mastery",
                    RANGED_UNLOCK_MELEE_LEVEL
                )
            } else {
                let r = player.ranged();
                format!(
                    "{}   ·   Lv {}   ·   {:.0} dmg   ·   range {:.0}",
                    r.name, player.ranged_level, r.damage, r.range
                )
            }
        }
        SlotItem::AutoRifle => format!(
            "{}   ·   Lv {}   ·   {:.1} dmg   ·   {:.1} rounds/s",
            player.gun_name(false),
            player.combat_level(),
            player.gun_damage(RIFLE_BASE_DAMAGE),
            1.0 / player.gun_cooldown(0.10),
        ),
        SlotItem::Shotgun => format!(
            "{}   ·   Lv {}   ·   9 pellets x {:.1} dmg   ·   {:.2}s per shot",
            player.gun_name(true),
            player.combat_level(),
            player.gun_damage(SHOTGUN_PELLET_DAMAGE),
            player.gun_cooldown(0.90),
        ),
        SlotItem::Food => format!(
            "Berries x{:.0}   ·   Q or click: -10 Food, +25 Health, +30 Hunger",
            player.food
        ),
        SlotItem::Build(_) => match item.build_kind(progress) {
            Some(kind) => format!(
                "{}   ·   {}   ·   {:.0} HP",
                kind.name(),
                kind.cost_label(),
                kind.max_hp()
            ),
            None => "Structure locked".to_string(),
        },
    };

    let mode = format!(
        "  |  {}  |  ROT {}° [R]",
        if buildings.grid_lock { "GRID ON [G]" } else { "GRID OFF [G]" },
        buildings.rotation as i32 * 90
    );
    let text = text + &mode;
    let status = match ghost {
        Some(g) if g.valid => ("Click to place", GREEN),
        Some(g) => (g.reason, RED),
        None => ("", GREEN),
    };

    let w = (text_width(&text, 15.0) + 36.0).max(420.0).min(screen_width() - 40.0);
    let (_, _, _, y0) = hotbar_geometry();
    let x = screen_width() * 0.5 - w * 0.5;
    let y = y0 - 52.0;
    draw_rectangle(x, y, w, 50.0, Color::new(0.04, 0.06, 0.09, 0.92));
    draw_rectangle_lines(x, y, w, 50.0, 2.0, Color::new(1.0, 1.0, 1.0, 0.16));
    label(&text, x + 16.0, y + 33.0, 15.0, Color::new(0.96, 0.98, 1.0, 1.0));
    if !status.0.is_empty() {
        label(
            status.0,
            x + w - 14.0 - text_width(status.0, 14.0),
            y - 9.0,
            14.0,
            status.1,
        );
    }
}

fn draw_banners(progress: &Progression) {
    for (i, b) in progress.banners.iter().enumerate() {
        let a = b.alpha();
        let w = (text_width(&b.text, 24.0) + 80.0)
            .max(340.0)
            .min(screen_width() - 80.0);
        let x = screen_width() * 0.5 - w * 0.5;
        let y = 18.0 + i as f32 * 70.0;
        draw_rectangle(
            x,
            y,
            w,
            60.0,
            with_alpha(Color::new(0.05, 0.07, 0.10, 1.0), a * 0.95),
        );
        draw_rectangle_lines(x, y, w, 60.0, 2.0, with_alpha(GOLD, a));
        draw_rectangle(x, y, 4.0, 60.0, with_alpha(GOLD, a));
        draw_text_center(
            &b.headline,
            screen_width() * 0.5,
            y + 17.0,
            13.0,
            with_alpha(GOLD, a),
            Some(with_alpha(OUTLINE, a)),
        );
        draw_text_center(
            &b.text,
            screen_width() * 0.5,
            y + 41.0,
            24.0,
            with_alpha(WHITE, a),
            Some(with_alpha(OUTLINE, a)),
        );
    }
}

fn draw_crosshair(mouse: Vec2) {
    draw_circle_lines(mouse.x, mouse.y, 10.0, 1.5, Color::new(1.0, 1.0, 1.0, 0.75));
    draw_circle(mouse.x, mouse.y, 2.0, Color::new(1.0, 0.95, 0.72, 1.0));
    for (dx, dy) in [(0.0, 15.0), (0.0, -15.0), (15.0, 0.0), (-15.0, 0.0)] {
        draw_line(
            mouse.x,
            mouse.y,
            mouse.x + dx,
            mouse.y + dy,
            1.5,
            Color::new(1.0, 1.0, 1.0, 0.5),
        );
    }
}

/// Edge arrows for threats outside the viewport, so nothing ambushes from
/// off-screen without warning.
pub fn draw_enemy_indicators(world: &World, camera_target: Vec2) {
    let sw = screen_width();
    let sh = screen_height();
    let center = vec2(sw * 0.5, sh * 0.5);
    let half = vec2(sw * 0.5 - 48.0, sh * 0.5 - 48.0);
    for e in world.enemies.iter() {
        let rel = e.pos - camera_target;
        if rel.x.abs() <= sw * 0.5 - 40.0 && rel.y.abs() <= sh * 0.5 - 40.0 {
            continue;
        }
        let sx = if rel.x.abs() > 0.001 {
            half.x / rel.x.abs()
        } else {
            f32::INFINITY
        };
        let sy = if rel.y.abs() > 0.001 {
            half.y / rel.y.abs()
        } else {
            f32::INFINITY
        };
        let scale = sx.min(sy);
        if !scale.is_finite() {
            continue;
        }
        let p = center + rel * scale;
        let dir = rel.normalize_or_zero();
        let perp = vec2(-dir.y, dir.x);
        let color = Color::new(1.0, 0.34, 0.30, 0.90);
        draw_triangle(
            p + dir * 11.0,
            p - dir * 6.0 + perp * 7.0,
            p - dir * 6.0 - perp * 7.0,
            color,
        );
        draw_circle(p.x, p.y, 2.5, with_alpha(color, 0.35));
    }
}

pub fn draw_hud(
    player: &Player,
    world: &World,
    buildings: &BuildingManager,
    progress: &Progression,
    mouse: Vec2,
    elapsed: f32,
    ghost: Option<&Ghost>,
) {
    draw_stats(player, elapsed, buildings.grid_lock);
    draw_minimap(world, buildings, player);
    draw_resources(player);
    draw_item_info(player, progress, buildings, ghost);
    draw_hotbar(player, progress, mouse);
    draw_banners(progress);
    if !progress.tech_open {
        draw_pause_button(mouse);
        draw_crosshair(mouse);
    }
    if progress.tech_open {
        draw_tech_tree(progress);
    }
}

// ---------------------------------------------------------------------------
// Mastery tech tree
// ---------------------------------------------------------------------------

const SECTION_TITLES: [&str; 4] = [
    "MELEE MASTERY",
    "RANGED WEAPONS",
    "WALLS & GATES",
    "DEFENSES",
];
const SECTION_COLS: [usize; 4] = [0, 1, 2, 2];
const SECTION_ROWS: [usize; 4] = [10, 10, 5, 3];
const WALL_UNLOCKS: [UnlockId; 5] = [
    UnlockId::StoneWall,
    UnlockId::MetalWall,
    UnlockId::WoodGate,
    UnlockId::StoneGate,
    UnlockId::MetalGate,
];
const DEFENSE_UNLOCKS: [UnlockId; 3] = [
    UnlockId::MetalSpike,
    UnlockId::Turret,
    UnlockId::Campfire,
];

struct TechLayout {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    col_w: f32,
    cols: [f32; 3],
    content_top: f32,
}

fn tech_layout() -> TechLayout {
    let w = 1220.0_f32.min(screen_width() - 40.0);
    let h = 680.0_f32.min(screen_height() - 40.0);
    let x = (screen_width() - w) * 0.5;
    let y = (screen_height() - h) * 0.5;
    let margin = 22.0;
    let gap = 18.0;
    let col_w = (w - margin * 2.0 - gap * 2.0) / 3.0;
    let cols = [
        x + margin,
        x + margin + col_w + gap,
        x + margin + (col_w + gap) * 2.0,
    ];
    TechLayout {
        x,
        y,
        w,
        h,
        col_w,
        cols,
        content_top: y + 104.0,
    }
}

struct SectionLayout {
    index: usize,
    x: f32,
    y: f32,
    w: f32,
    rows_y: Vec<f32>,
}

fn section_layouts(progress: &Progression) -> Vec<SectionLayout> {
    let l = tech_layout();
    let mut cursors = [l.content_top; 3];
    let mut out = Vec::new();
    for index in 0..4 {
        let col = SECTION_COLS[index];
        let x = l.cols[col];
        let y = cursors[col];
        let collapsed = progress.collapsed[index];
        let mut rows_y = Vec::new();
        let mut height = 32.0;
        if !collapsed {
            for row in 0..SECTION_ROWS[index] {
                rows_y.push(y + 32.0 + 6.0 + row as f32 * 46.0);
            }
            height = 32.0 + 6.0 + SECTION_ROWS[index] as f32 * 46.0 - 4.0;
        }
        out.push(SectionLayout {
            index,
            x,
            y,
            w: l.col_w,
            rows_y,
        });
        cursors[col] = y + height + 12.0;
    }
    out
}

pub fn tech_tree_click(progress: &mut Progression, mouse: Vec2) -> bool {
    if !progress.tech_open {
        return false;
    }
    let layouts = section_layouts(progress);
    for layout in layouts.iter() {
        if in_rect(mouse, (layout.x, layout.y, layout.w, 32.0)) {
            progress.collapsed[layout.index] = !progress.collapsed[layout.index];
            return true;
        }
    }
    let l = tech_layout();
    in_rect(mouse, (l.x, l.y, l.w, l.h))
}

struct RowInfo {
    title: String,
    detail: String,
    status: String,
    frac: f32,
    unlocked: bool,
}

fn melee_row_info(index: usize, progress: &Progression) -> RowInfo {
    let tier = &MELEE_TIERS[index];
    let unlocked = progress.melee_level() > index;
    let threshold = MELEE_THRESHOLDS[index];
    let points = progress.melee_points();
    let frac = if unlocked {
        1.0
    } else if threshold == 0 {
        1.0
    } else {
        (points as f32 / threshold as f32).clamp(0.0, 1.0)
    };
    let detail = if unlocked && index == 9 {
        let current = crate::player::melee_tier(progress.melee_level());
        format!(
            "Endless Lv {} · {:.0} dmg · {:.0} reach · x{:.1} gather",
            progress.melee_level(),
            current.damage,
            current.reach,
            current.gather
        )
    } else if unlocked {
        format!(
            "{:.0} dmg · reach {:.0} · x{:.1} gather",
            tier.damage, tier.reach, tier.gather
        )
    } else {
        format!("{} / {} mastery points", points, threshold)
    };
    RowInfo {
        title: format!("Lv {}  {}", index + 1, tier.name),
        detail,
        status: if unlocked { "UNLOCKED" } else { "LOCKED" }.to_string(),
        frac,
        unlocked,
    }
}

fn ranged_row_info(index: usize, progress: &Progression) -> RowInfo {
    let tier = &RANGED_TIERS[index];
    let unlocked = progress.ranged_level() > index;
    let threshold = RANGED_THRESHOLDS[index];
    let points = progress.ranged_points();
    let frac = if unlocked {
        1.0
    } else if !progress.ranged_unlocked() {
        let (f, _) = progress.ranged_next();
        f
    } else if threshold == 0 {
        1.0
    } else {
        (points as f32 / threshold as f32).clamp(0.0, 1.0)
    };
    let detail = if unlocked && index == 9 {
        let current = crate::player::ranged_tier(progress.ranged_level());
        format!(
            "Endless Lv {} · {:.0} dmg · {:.0} range · {:.2}s cooldown",
            progress.ranged_level(),
            current.damage,
            current.range,
            current.cooldown
        )
    } else if unlocked {
        format!(
            "{:.0} dmg · range {:.0} · {:.2}s cooldown",
            tier.damage, tier.range, tier.cooldown
        )
    } else if !progress.ranged_unlocked() {
        format!(
            "Requires Melee Lv {}  ·  hit with melee to unlock",
            RANGED_UNLOCK_MELEE_LEVEL
        )
    } else {
        format!("{} / {} ranged points", points, threshold)
    };
    RowInfo {
        title: format!("Lv {}  {}", index + 1, tier.name),
        detail,
        status: if unlocked {
            "UNLOCKED"
        } else if !progress.ranged_unlocked() {
            "MELEE LV 3"
        } else {
            "LOCKED"
        }
        .to_string(),
        frac,
        unlocked,
    }
}

fn unlock_row_info(id: UnlockId, progress: &Progression) -> RowInfo {
    let unlocked = progress.is_unlocked(id);
    let (frac, detail) = if unlocked {
        (1.0, id.hint().to_string())
    } else {
        let (f, d) = progress.progress(id);
        (f, format!("{}  ·  Cost: {}", d, id.cost_label()))
    };
    RowInfo {
        title: id.name().to_string(),
        detail,
        status: if unlocked { "UNLOCKED" } else { "LOCKED" }.to_string(),
        frac,
        unlocked,
    }
}

fn row_info(section: usize, row: usize, progress: &Progression) -> RowInfo {
    match section {
        0 => melee_row_info(row, progress),
        1 => ranged_row_info(row, progress),
        2 => unlock_row_info(WALL_UNLOCKS[row], progress),
        _ => unlock_row_info(DEFENSE_UNLOCKS[row], progress),
    }
}

fn section_unlocked(section: usize, progress: &Progression) -> usize {
    match section {
        0 => progress.melee_level(),
        1 => progress.ranged_level(),
        2 => WALL_UNLOCKS
            .iter()
            .filter(|id| progress.is_unlocked(**id))
            .count(),
        _ => DEFENSE_UNLOCKS
            .iter()
            .filter(|id| progress.is_unlocked(**id))
            .count(),
    }
}

fn section_total(section: usize) -> usize {
    match section {
        0 => 10,
        1 => 10,
        2 => 5,
        _ => 3,
    }
}

fn draw_tech_row(x: f32, y: f32, w: f32, info: &RowInfo) {
    let bg = if info.unlocked {
        Color::new(0.13, 0.23, 0.16, 0.96)
    } else {
        Color::new(0.12, 0.14, 0.19, 0.96)
    };
    draw_rectangle(x, y, w, 43.0, bg);
    draw_rectangle_lines(x, y, w, 43.0, 1.0, Color::new(1.0, 1.0, 1.0, 0.10));
    draw_rectangle(
        x,
        y,
        3.0,
        43.0,
        if info.unlocked {
            GREEN
        } else {
            Color::new(0.48, 0.50, 0.58, 1.0)
        },
    );

    label(
        &info.title,
        x + 10.0,
        y + 16.0,
        14.0,
        if info.unlocked {
            Color::new(0.96, 0.99, 0.94, 1.0)
        } else {
            Color::new(0.88, 0.91, 0.94, 1.0)
        },
    );
    label(
        &info.status,
        x + w - 9.0 - text_width(&info.status, 11.0),
        y + 15.0,
        11.0,
        if info.unlocked {
            Color::new(0.55, 1.0, 0.62, 1.0)
        } else {
            GOLD
        },
    );

    let bar_x = x + 10.0;
    let bar_w = w - 20.0;
    draw_rectangle(bar_x, y + 19.0, bar_w, 5.0, Color::new(0.04, 0.05, 0.08, 1.0));
    draw_rectangle(
        bar_x,
        y + 19.0,
        bar_w * info.frac.clamp(0.0, 1.0),
        5.0,
        if info.unlocked { GREEN } else { GOLD },
    );
    draw_rectangle_lines(bar_x, y + 19.0, bar_w, 5.0, 1.0, Color::new(0.0, 0.0, 0.0, 0.6));

    label(
        &info.detail,
        x + 10.0,
        y + 39.0,
        12.0,
        Color::new(0.86, 0.89, 0.92, 1.0),
    );
}

fn draw_section_header(x: f32, y: f32, w: f32, index: usize, progress: &Progression) {
    draw_rectangle(x, y, w, 32.0, Color::new(0.17, 0.20, 0.26, 1.0));
    draw_rectangle(x, y, 4.0, 32.0, GOLD);
    let cx = x + 16.0;
    let cy = y + 16.0;
    if progress.collapsed[index] {
        draw_triangle(
            vec2(cx - 4.0, cy - 6.0),
            vec2(cx - 4.0, cy + 6.0),
            vec2(cx + 6.0, cy),
            GOLD,
        );
    } else {
        draw_triangle(
            vec2(cx - 6.0, cy - 4.0),
            vec2(cx + 6.0, cy - 4.0),
            vec2(cx, cy + 6.0),
            GOLD,
        );
    }
    label(
        SECTION_TITLES[index],
        x + 30.0,
        y + 22.0,
        15.0,
        Color::new(0.98, 0.97, 0.88, 1.0),
    );
    let counter = if index < 2 {
        format!("Lv {} / endless", section_unlocked(index, progress))
    } else {
        format!(
            "{} / {}",
            section_unlocked(index, progress),
            section_total(index)
        )
    };
    label(
        &counter,
        x + w - 10.0 - text_width(&counter, 12.0),
        y + 21.0,
        12.0,
        Color::new(0.86, 0.90, 0.94, 1.0),
    );
}

fn draw_tech_tree(progress: &Progression) {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.02, 0.03, 0.05, 0.74),
    );
    let l = tech_layout();
    draw_panel(l.x, l.y, l.w, l.h, Color::new(0.06, 0.08, 0.12, 0.98));
    draw_rectangle(l.x, l.y, l.w, 5.0, GOLD);

    label(
        "MASTERY & UNLOCKS",
        l.x + 24.0,
        l.y + 42.0,
        30.0,
        Color::new(0.99, 0.95, 0.80, 1.0),
    );
    label(
        "TAB to close  ·  click a section header to collapse it  ·  every bar fills through play",
        l.x + 26.0,
        l.y + 68.0,
        14.0,
        Color::new(0.82, 0.87, 0.91, 1.0),
    );
    let header = format!(
        "MELEE Lv {}  ·  RANGED {}  ·  UNLOCKS {}/8",
        progress.melee_level(),
        if progress.ranged_unlocked() {
            format!("Lv {}", progress.ranged_level())
        } else {
            format!("unlocks at Melee Lv {}", RANGED_UNLOCK_MELEE_LEVEL)
        },
        progress.unlocked_count()
    );
    label(
        &header,
        l.x + l.w - 24.0 - text_width(&header, 15.0),
        l.y + 40.0,
        15.0,
        GOLD,
    );

    let layouts = section_layouts(progress);
    for layout in layouts.iter() {
        draw_section_header(layout.x, layout.y, layout.w, layout.index, progress);
        if progress.collapsed[layout.index] {
            continue;
        }
        for (row, row_y) in layout.rows_y.iter().enumerate() {
            let info = row_info(layout.index, row, progress);
            draw_tech_row(layout.x, *row_y, layout.w, &info);
        }
    }

    let (_, melee_next) = progress.melee_next();
    let (_, ranged_next) = progress.ranged_next();
    label(
        &format!(
            "MELEE NEXT: {}   ·   RANGED NEXT: {}   ·   Sapphire is the last name, not the last level: stats keep growing forever",
            melee_next, ranged_next
        ),
        l.x + 24.0,
        l.y + l.h - 16.0,
        12.0,
        Color::new(0.92, 0.96, 1.0, 1.0),
    );
}

// ---------------------------------------------------------------------------
// Menus
// ---------------------------------------------------------------------------

fn menu_button(
    rect: (f32, f32, f32, f32),
    text: &str,
    hover: bool,
    primary: bool,
    size: f32,
) {
    let (x, y, w, h) = rect;
    draw_rectangle(x - 4.0, y - 4.0, w + 8.0, h + 8.0, OUTLINE);
    let fill = if primary {
        if hover {
            Color::new(0.92, 0.74, 0.30, 1.0)
        } else {
            Color::new(0.72, 0.56, 0.22, 1.0)
        }
    } else if hover {
        Color::new(0.20, 0.34, 0.30, 1.0)
    } else {
        Color::new(0.09, 0.18, 0.17, 1.0)
    };
    draw_rectangle(x, y, w, h, fill);
    draw_rectangle_lines(
        x,
        y,
        w,
        h,
        if hover { 2.0 } else { 1.0 },
        with_alpha(GOLD, if hover { 0.95 } else { 0.55 }),
    );
    if primary {
        draw_rectangle(x, y, w, 4.0, Color::new(1.0, 0.94, 0.72, 1.0));
    }
    draw_text_center(
        text,
        x + w * 0.5,
        y + h * 0.5,
        size,
        if primary {
            OUTLINE
        } else {
            Color::new(0.96, 0.94, 0.84, 1.0)
        },
        None,
    );
}

pub fn start_button_rect(has_save: bool) -> (f32, f32, f32, f32) {
    let h = 74.0;
    if has_save {
        (screen_width() * 0.5 - 310.0, 300.0, 300.0, h)
    } else {
        let w = 360.0;
        ((screen_width() - w) * 0.5, 300.0, w, h)
    }
}

pub fn continue_button_rect() -> (f32, f32, f32, f32) {
    (screen_width() * 0.5 + 10.0, 300.0, 300.0, 74.0)
}

pub fn draw_start_menu(time: f32, has_save: bool) {
    let sw = screen_width();
    let sh = screen_height();
    let (mx, my) = mouse_position();
    let mouse = vec2(mx, my);

    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.05, 0.10, 0.08, 1.0));
    for i in 0..8 {
        let r = 170.0 + i as f32 * 105.0 + (time * 14.0) % 105.0;
        draw_circle_lines(
            sw * 0.5,
            sh * 0.52,
            r,
            2.0,
            Color::new(0.40, 0.80, 0.52, 0.05),
        );
    }
    for i in 0..22 {
        let seed = i as f32 * 0.618;
        let x = (seed.sin() * 0.5 + 0.5) * sw + (time * 0.6 + i as f32).sin() * 24.0;
        let y = ((time * (9.0 + (i % 5) as f32 * 3.0) + i as f32 * 83.0) % (sh + 120.0)) - 60.0;
        draw_circle(
            x,
            y,
            2.5 + (i % 3) as f32 * 1.8,
            Color::new(0.36, 0.70, 0.42, 0.18),
        );
    }

    draw_text_center("WILDLANDS", sw * 0.5, 140.0, 92.0, GOLD, Some(OUTLINE));
    draw_text_center(
        "SURVIVE   ·   GATHER   ·   BUILD",
        sw * 0.5,
        202.0,
        22.0,
        Color::new(0.76, 0.92, 0.78, 1.0),
        Some(OUTLINE),
    );
    draw_text_center(
        "10 melee + 10 ranged tiers  ·  Sapphire scales forever  ·  walls, gates & turrets",
        sw * 0.5,
        230.0,
        16.0,
        Color::new(0.76, 0.89, 0.80, 1.0),
        Some(OUTLINE),
    );
    draw_line(sw * 0.5 - 210.0, 250.0, sw * 0.5 + 210.0, 250.0, 2.0, with_alpha(GOLD, 0.45));

    let btn = start_button_rect(has_save);
    let hover = in_rect(mouse, btn);
    menu_button(
        btn,
        if has_save { "NEW GAME" } else { "PLAY" },
        hover,
        true,
        if has_save { 26.0 } else { 36.0 },
    );
    if has_save {
        let cont = continue_button_rect();
        menu_button(
            cont,
            "CONTINUE",
            in_rect(mouse, cont),
            false,
            30.0,
        );
        draw_text_center(
            "ENTER continues your last run  ·  N starts a fresh world",
            sw * 0.5,
            400.0,
            13.0,
            Color::new(0.82, 0.90, 0.82, 1.0),
            Some(OUTLINE),
        );
    } else {
        draw_text_center(
            "ENTER starts a fresh run",
            sw * 0.5,
            400.0,
            13.0,
            Color::new(0.82, 0.90, 0.82, 1.0),
            Some(OUTLINE),
        );
    }

    let px = sw * 0.5 - 450.0;
    let py = 422.0;
    let pw = 900.0;
    let ph = 204.0;
    draw_panel(px, py, pw, ph, Color::new(0.05, 0.08, 0.10, 0.86));
    label("CONTROLS", px + 20.0, py + 26.0, 15.0, GOLD);
    let rows = [
        ("WASD / Arrows", "Move"),
        ("Mouse", "Aim"),
        ("LMB / RMB / Space", "Act / demolish (hold)"),
        ("1 - 9, 0, -, =, [, ]", "Select hotbar"),
        ("N / B", "Damage numbers / shake"),
        ("3 / 4", "Auto rifle / birdshot"),
        ("Q", "Eat food"),
        ("G / R / H", "Grid / rotate / repair"),
        ("Tab", "Mastery tech tree"),
        ("P / Esc", "Pause / resume"),
    ];
    for (i, (key, action)) in rows.iter().enumerate() {
        let col = i / 5;
        let row = i % 5;
        let cx = px + 24.0 + col as f32 * 450.0;
        let cy = py + 52.0 + row as f32 * 30.0;
        label(key, cx, cy, 14.0, Color::new(0.99, 0.93, 0.72, 1.0));
        // Wide gap: the longer key strings would otherwise run into the action
        // column now that the type is roughly a third larger.
        label(
            action,
            cx + 232.0,
            cy,
            14.0,
            Color::new(0.88, 0.92, 0.94, 1.0),
        );
    }

    draw_text_center(
        "Hunt, harvest and fortify. Better weapons and structures are earned by using what you own.",
        sw * 0.5,
        py + ph + 32.0,
        14.0,
        Color::new(0.70, 0.82, 0.74, 1.0),
        Some(OUTLINE),
    );
    draw_text_center(
        "100% procedural vector art · no external assets",
        sw * 0.5,
        sh - 24.0,
        13.0,
        Color::new(0.58, 0.68, 0.62, 1.0),
        None,
    );
}

pub fn restart_button_rect() -> (f32, f32, f32, f32) {
    let w = 320.0;
    let h = 66.0;
    ((screen_width() - w) * 0.5, screen_height() * 0.5 + 148.0, w, h)
}

pub fn draw_game_over(progress: &Progression, elapsed: f32, mouse: Vec2, time: f32) {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.07, 0.02, 0.03, 0.80));
    for i in 0..6 {
        let a = 0.05 + 0.02 * (time * 1.7 + i as f32).sin().abs();
        draw_circle_lines(
            sw * 0.5,
            sh * 0.45,
            160.0 + i as f32 * 90.0,
            3.0,
            Color::new(0.9, 0.25, 0.25, a),
        );
    }

    draw_text_center(
        "YOU DIED",
        sw * 0.5,
        sh * 0.23,
        78.0,
        Color::new(0.96, 0.34, 0.32, 1.0),
        Some(OUTLINE),
    );
    draw_text_center(
        "The wildlands reclaim everything.",
        sw * 0.5,
        sh * 0.23 + 58.0,
        21.0,
        Color::new(0.90, 0.80, 0.78, 1.0),
        Some(OUTLINE),
    );

    let pw = 560.0;
    let ph = 238.0;
    let px = sw * 0.5 - pw * 0.5;
    let py = sh * 0.5 - 96.0;
    draw_panel(px, py, pw, ph, Color::new(0.05, 0.07, 0.10, 0.94));
    draw_rectangle(px, py, pw, 4.0, Color::new(0.92, 0.32, 0.30, 1.0));

    let stats = [
        ("Survived", format_time(elapsed)),
        ("Enemies defeated", format!("{}", progress.kills)),
        ("Structures built", format!("{}", progress.total_placed)),
        (
            "Mastery",
            format!(
                "Melee Lv {}  ·  Ranged {}",
                progress.melee_level(),
                if progress.ranged_unlocked() {
                    format!("Lv {}", progress.ranged_level())
                } else {
                    "locked".to_string()
                }
            ),
        ),
        (
            "Gathered",
            format!(
                "{} wood · {} stone · {} metal",
                progress.total_wood, progress.total_stone, progress.total_metal
            ),
        ),
    ];
    for (i, (name, value)) in stats.iter().enumerate() {
        let y = py + 36.0 + i as f32 * 34.0;
        label(name, px + 26.0, y, 16.0, Color::new(0.80, 0.85, 0.88, 1.0));
        label(
            value,
            px + pw - 26.0 - text_width(value, 17.0),
            y,
            17.0,
            Color::new(0.99, 0.99, 0.94, 1.0),
        );
        draw_line(
            px + 24.0,
            y + 11.0,
            px + pw - 24.0,
            y + 11.0,
            1.0,
            Color::new(1.0, 1.0, 1.0, 0.07),
        );
    }

    let btn = restart_button_rect();
    let hover = in_rect(mouse, btn) || is_key_pressed(KeyCode::Enter);
    draw_rectangle(btn.0 - 4.0, btn.1 - 4.0, btn.2 + 8.0, btn.3 + 8.0, OUTLINE);
    draw_rectangle(
        btn.0,
        btn.1,
        btn.2,
        btn.3,
        if hover {
            Color::new(0.92, 0.74, 0.30, 1.0)
        } else {
            Color::new(0.72, 0.56, 0.22, 1.0)
        },
    );
    draw_text_center(
        "RESTART",
        sw * 0.5,
        btn.1 + btn.3 * 0.5,
        30.0,
        OUTLINE,
        None,
    );
    draw_text_center(
        "or press ENTER",
        sw * 0.5,
        btn.1 + btn.3 + 22.0,
        14.0,
        Color::new(0.82, 0.78, 0.72, 1.0),
        Some(OUTLINE),
    );
}
