//! The endless wildlands: procedurally streamed chunks of terrain, water,
//! resource nodes and hostile wildlife, plus collision and combat resolution.

use macroquad::prelude::*;
use macroquad::rand::gen_range;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use crate::building::BuildingManager;
use crate::player::{Player, RIFLE_BASE_DAMAGE, SHOTGUN_PELLET_DAMAGE};
use crate::progression::Progression;

/// Size of the home clearing, used to keep the camp centred on the map.
pub const WORLD_SIZE: f32 = 3200.0;

/// Where the survivor starts: the camp, the practice range and the starter
/// resources all sit here. The rest of the world continues forever.
pub fn home() -> Vec2 {
    vec2(WORLD_SIZE * 0.5, WORLD_SIZE * 0.5)
}

/// Side length of one streamed chunk of world.
pub const CHUNK: f32 = 640.0;

/// How many chunks are kept loaded in each direction around the survivor.
const CHUNK_MARGIN: i32 = 2;

/// Which chunk contains a point.
pub fn chunk_of(pos: Vec2) -> (i32, i32) {
    (
        (pos.x / CHUNK).floor() as i32,
        (pos.y / CHUNK).floor() as i32,
    )
}

/// Stable identity for a generated node, so mining is remembered when a chunk
/// unloads and is generated again later.
pub fn node_key(pos: Vec2) -> (i32, i32) {
    (pos.x.round() as i32, pos.y.round() as i32)
}

/// Deterministic per-chunk random source: the same chunk always grows the same
/// plants, pools and outcrops, whenever it is streamed back in.
struct ChunkRng(u64);

impl ChunkRng {
    fn new(cx: i32, cy: i32) -> Self {
        let mut h: u64 = 0x9E37_79B9_7F4A_7C15;
        h ^= (cx as i64 as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93);
        h ^= (cy as i64 as u64).wrapping_mul(0xA24B_AED4_963E_E407);
        Self(h | 1)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }
}
pub const OUTLINE: Color = Color::new(0.05, 0.06, 0.09, 1.0);

/// Global text scale applied by every text helper. Larger type plus a heavy
/// dark outline keeps the HUD readable over grass, panels and combat effects.
pub const TEXT_SCALE: f32 = 1.7;

/// Nothing is allowed to render smaller than this, in pixels. The window is a
/// fixed 1280x720 buffer that macOS then upscales, so small type smears into
/// mush; every text helper routes its size through here.
pub const MIN_TEXT_PX: f32 = 17.0;

/// Final on-screen font size for a requested base size.
pub fn text_px(size: f32) -> f32 {
    (size * TEXT_SCALE).max(MIN_TEXT_PX)
}

// ---------------------------------------------------------------------------
// Small drawing helpers shared by every module.
// ---------------------------------------------------------------------------

pub fn with_alpha(color: Color, alpha: f32) -> Color {
    Color::new(color.r, color.g, color.b, alpha.clamp(0.0, 1.0))
}

pub fn rot_vec(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    vec2(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// Signed shortest difference between two angles.
pub fn angle_delta(a: f32, b: f32) -> f32 {
    let mut d = (a - b) % TAU;
    if d > PI {
        d -= TAU;
    }
    if d < -PI {
        d += TAU;
    }
    d
}

pub fn rot_rect_points(center: Vec2, w: f32, h: f32, angle: f32) -> [Vec2; 4] {
    let hw = w * 0.5;
    let hh = h * 0.5;
    [
        center + rot_vec(vec2(-hw, -hh), angle),
        center + rot_vec(vec2(hw, -hh), angle),
        center + rot_vec(vec2(hw, hh), angle),
        center + rot_vec(vec2(-hw, hh), angle),
    ]
}

pub fn draw_rot_rect(center: Vec2, w: f32, h: f32, angle: f32, color: Color) {
    let p = rot_rect_points(center, w, h, angle);
    draw_triangle(p[0], p[1], p[2], color);
    draw_triangle(p[0], p[2], p[3], color);
}

pub fn draw_rot_rect_outlined(center: Vec2, w: f32, h: f32, angle: f32, color: Color) {
    draw_rot_rect(center, w + 5.0, h + 5.0, angle, OUTLINE);
    draw_rot_rect(center, w, h, angle, color);
}

pub fn outlined_circle(pos: Vec2, radius: f32, fill: Color) {
    draw_circle(pos.x, pos.y, radius + 3.0, OUTLINE);
    draw_circle(pos.x, pos.y, radius, fill);
}

pub fn outlined_poly(center: Vec2, sides: u8, radius: f32, rotation: f32, fill: Color) {
    draw_poly(center.x, center.y, sides, radius + 3.0, rotation, OUTLINE);
    draw_poly(center.x, center.y, sides, radius, rotation, fill);
}

pub fn shadow(pos: Vec2, radius: f32) {
    draw_circle(
        pos.x,
        pos.y + radius * 0.55,
        radius * 0.95,
        Color::new(0.02, 0.05, 0.03, 0.22),
    );
}

/// Shared body plan for the four-legged animals, so a jackal, a jaguar, a lion
/// and a buffalo read as different creatures without four copies of the art.
fn draw_quadruped(
    e: &Enemy,
    dir: Vec2,
    side: Vec2,
    len: f32,
    wid: f32,
    head_r: f32,
    body: Color,
    dark: Color,
    accent: Color,
    mane: bool,
    tusks: bool,
    stripes: bool,
) {
    for s in [-1.0, 1.0] {
        let leg = e.pos + side * (wid * 0.42 * s) + dir * (len * 0.04);
        draw_rot_rect_outlined(leg, wid * 0.26, 15.0, e.facing, dark);
        draw_circle(leg.x + dir.x * 5.0, leg.y + dir.y * 5.0, 3.4, dark);
    }
    let tail = e.pos - dir * (len * 0.52);
    let tail_tip = tail - dir * (len * 0.22) + side * (wid * 0.26);
    draw_line(tail.x, tail.y, tail_tip.x, tail_tip.y, 3.0, dark);
    if mane {
        draw_circle(tail_tip.x, tail_tip.y, 4.0, accent);
    }
    draw_rot_rect_outlined(e.pos, len, wid, e.facing, body);
    draw_circle(
        e.pos.x - dir.x * len * 0.06 - side.x * wid * 0.12,
        e.pos.y - dir.y * len * 0.06 - side.y * wid * 0.12,
        wid * 0.28,
        with_alpha(accent, 0.35),
    );
    if stripes {
        for k in -1..=1 {
            let base = e.pos + dir * (k as f32 * len * 0.18);
            draw_line(
                base.x + side.x * wid * 0.5,
                base.y + side.y * wid * 0.5,
                base.x - side.x * wid * 0.5,
                base.y - side.y * wid * 0.5,
                2.4,
                with_alpha(dark, 0.55),
            );
        }
    }
    let head = e.pos + dir * (len * 0.52);
    if mane {
        outlined_circle(head - dir * 2.0, head_r * 1.55, accent);
    }
    for s in [-1.0, 1.0] {
        outlined_circle(
            head + side * (head_r * 0.62 * s) - dir * (head_r * 0.15),
            head_r * 0.4,
            dark,
        );
    }
    outlined_circle(head, head_r, body);
    draw_circle(
        head.x + dir.x * head_r * 0.55,
        head.y + dir.y * head_r * 0.55,
        head_r * 0.5,
        with_alpha(accent, 0.9),
    );
    draw_circle(
        head.x + dir.x * head_r * 0.95,
        head.y + dir.y * head_r * 0.95,
        head_r * 0.28,
        OUTLINE,
    );
    if tusks {
        for s in [-1.0, 1.0] {
            let base = head + side * (head_r * 0.5 * s) + dir * (head_r * 0.2);
            draw_triangle(
                base,
                base + side * (2.4 * s) - dir * 3.0,
                base + dir * 9.0,
                Color::new(0.88, 0.84, 0.68, 1.0),
            );
        }
    }
    for s in [-1.0, 1.0] {
        let eye = head + side * (head_r * 0.42 * s) + dir * (head_r * 0.3);
        draw_circle(eye.x, eye.y, head_r * 0.3, Color::new(1.0, 0.76, 0.24, 0.16));
        draw_circle(eye.x, eye.y, head_r * 0.16, Color::new(1.0, 0.88, 0.35, 1.0));
    }
}

pub fn draw_bar(top_left_center: Vec2, width: f32, height: f32, frac: f32, fill: Color) {
    let x = top_left_center.x - width * 0.5;
    let y = top_left_center.y;
    draw_rectangle(x - 2.0, y - 2.0, width + 4.0, height + 4.0, OUTLINE);
    draw_rectangle(x, y, width, height, Color::new(0.13, 0.14, 0.18, 1.0));
    draw_rectangle(
        x,
        y,
        width * frac.clamp(0.0, 1.0),
        height,
        fill,
    );
}

pub fn text_width(text: &str, size: f32) -> f32 {
    measure_text(text, None, text_px(size) as u16, 1.0).width
}

pub fn draw_text_outlined(text: &str, x: f32, y: f32, size: f32, color: Color, outline: Color) {
    let size = text_px(size);
    let stroke = (size * 0.09).clamp(1.5, 3.5);
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

pub fn draw_text_center(
    text: &str,
    center_x: f32,
    center_y: f32,
    size: f32,
    color: Color,
    outline: Option<Color>,
) {
    let x = center_x - text_width(text, size) * 0.5;
    let y = center_y + text_px(size) * 0.35;
    match outline {
        Some(o) => {
            draw_text_outlined(text, x, y, size, color, o);
        }
        None => {
            draw_text(text, x, y, size, color);
        }
    }
}

pub fn view_bounds(center: Vec2) -> (Vec2, Vec2) {
    let half = vec2(screen_width() * 0.5, screen_height() * 0.5);
    (center - half, center + half)
}

fn on_screen(p: Vec2, min: Vec2, max: Vec2, margin: f32) -> bool {
    p.x > min.x - margin
        && p.x < max.x + margin
        && p.y > min.y - margin
        && p.y < max.y + margin
}

// ---------------------------------------------------------------------------
// Resources & nodes
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Biomes and water
// ---------------------------------------------------------------------------

/// The camp sits in a green meadow. Beyond it the map is split into three wild
/// regions, each with its own ground, plants and animals.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    Meadow,
    Desert,
    Rainforest,
    Savanna,
}

/// Radius of the starting meadow around the centre of the map.
pub const MEADOW_RADIUS: f32 = 900.0;

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Biome::Meadow => "MEADOW",
            Biome::Desert => "DESERT",
            Biome::Rainforest => "RAINFOREST",
            Biome::Savanna => "SAVANNA",
        }
    }

    /// Flavour line shown when the survivor crosses into the region.
    pub fn subtitle(self) -> &'static str {
        match self {
            Biome::Meadow => "Home ground",
            Biome::Desert => "Sand, thorn and jackals",
            Biome::Rainforest => "Wet green tangle",
            Biome::Savanna => "Dry grass and big cats",
        }
    }

    /// Ground paint for this region.
    pub fn ground(self) -> Color {
        match self {
            Biome::Meadow => Color::new(0.20, 0.33, 0.19, 1.0),
            Biome::Desert => Color::new(0.66, 0.55, 0.33, 1.0),
            Biome::Rainforest => Color::new(0.12, 0.27, 0.15, 1.0),
            Biome::Savanna => Color::new(0.48, 0.44, 0.23, 1.0),
        }
    }

    /// Which animals live here. Raiders are people, so they turn up anywhere.
    pub fn animals(self) -> &'static [EnemyKind] {
        match self {
            Biome::Meadow => &[EnemyKind::Wolf, EnemyKind::Bear],
            Biome::Desert => &[EnemyKind::Jackal, EnemyKind::Scorpion],
            Biome::Rainforest => &[EnemyKind::Jaguar, EnemyKind::Boa],
            Biome::Savanna => &[EnemyKind::Lion, EnemyKind::Buffalo],
        }
    }
}

/// Which biome covers a point of the map: a meadow disc around the camp, then
/// three 120 degree wedges of savanna, rainforest and desert.
pub fn biome_at(pos: Vec2) -> Biome {
    let d = pos - vec2(WORLD_SIZE * 0.5, WORLD_SIZE * 0.5);
    if d.length() < MEADOW_RADIUS {
        return Biome::Meadow;
    }
    let mut a = d.y.atan2(d.x);
    if a < 0.0 {
        a += TAU;
    }
    let third = TAU / 3.0;
    if a < third {
        Biome::Savanna
    } else if a < third * 2.0 {
        Biome::Rainforest
    } else {
        Biome::Desert
    }
}

/// A pond, oasis or stretch of river. Water cannot be walked through or built
/// on, though arrows and rounds still fly over it.
pub struct Water {
    pub pos: Vec2,
    pub radius: f32,
    pub seed: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resource {
    Wood,
    Stone,
    Metal,
    Food,
}

impl Resource {
    pub fn label(self) -> &'static str {
        match self {
            Resource::Wood => "Wood",
            Resource::Stone => "Stone",
            Resource::Metal => "Metal",
            Resource::Food => "Food",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Resource::Wood => Color::new(0.72, 0.50, 0.27, 1.0),
            Resource::Stone => Color::new(0.72, 0.75, 0.78, 1.0),
            Resource::Metal => Color::new(0.45, 0.86, 0.92, 1.0),
            Resource::Food => Color::new(0.92, 0.35, 0.38, 1.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum NodeKind {
    Berry,
    Tree,
    Rock,
    Iron,
}

impl NodeKind {
    pub fn resource(self) -> Resource {
        match self {
            NodeKind::Berry => Resource::Food,
            NodeKind::Tree => Resource::Wood,
            NodeKind::Rock => Resource::Stone,
            NodeKind::Iron => Resource::Metal,
        }
    }

    pub fn radius(self) -> f32 {
        match self {
            NodeKind::Berry => 20.0,
            NodeKind::Tree => 26.0,
            NodeKind::Rock => 23.0,
            NodeKind::Iron => 24.0,
        }
    }

    pub fn max_hp(self) -> f32 {
        match self {
            NodeKind::Berry => 40.0,
            NodeKind::Tree => 70.0,
            NodeKind::Rock => 100.0,
            NodeKind::Iron => 130.0,
        }
    }

    pub fn yield_per_hit(self) -> f32 {
        match self {
            NodeKind::Berry => 6.0,
            NodeKind::Tree => 5.0,
            NodeKind::Rock => 5.0,
            NodeKind::Iron => 3.0,
        }
    }

    pub fn solid(self) -> bool {
        matches!(self, NodeKind::Tree | NodeKind::Rock | NodeKind::Iron)
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ResourceNode {
    #[serde(with = "crate::save::vec2_serde")]
    pub pos: Vec2,
    pub kind: NodeKind,
    pub hp: f32,
    pub max_hp: f32,
    pub radius: f32,
    pub shake: f32,
    pub seed: f32,
}

// ---------------------------------------------------------------------------
// Enemies
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum EnemyKind {
    Wolf,
    Bear,
    Raider,
    Jackal,
    Scorpion,
    Jaguar,
    Boa,
    Lion,
    Buffalo,
}

impl EnemyKind {
    pub fn max_hp(self) -> f32 {
        match self {
            EnemyKind::Wolf => 80.0,
            EnemyKind::Bear => 210.0,
            EnemyKind::Raider => 140.0,
            EnemyKind::Jackal => 65.0,
            EnemyKind::Scorpion => 55.0,
            EnemyKind::Jaguar => 150.0,
            EnemyKind::Boa => 190.0,
            EnemyKind::Lion => 240.0,
            EnemyKind::Buffalo => 300.0,
        }
    }

    pub fn speed(self) -> f32 {
        match self {
            EnemyKind::Wolf => 150.0,
            EnemyKind::Bear => 106.0,
            EnemyKind::Raider => 124.0,
            EnemyKind::Jackal => 172.0,
            EnemyKind::Scorpion => 92.0,
            EnemyKind::Jaguar => 178.0,
            EnemyKind::Boa => 84.0,
            EnemyKind::Lion => 164.0,
            EnemyKind::Buffalo => 112.0,
        }
    }

    pub fn damage(self) -> f32 {
        match self {
            EnemyKind::Wolf => 7.0,
            EnemyKind::Bear => 15.0,
            EnemyKind::Raider => 11.0,
            EnemyKind::Jackal => 6.0,
            EnemyKind::Scorpion => 12.0,
            EnemyKind::Jaguar => 13.0,
            EnemyKind::Boa => 16.0,
            EnemyKind::Lion => 16.0,
            EnemyKind::Buffalo => 18.0,
        }
    }

    /// How close the survivor can get before this creature notices them. Sight
    /// still has to be clear of trees and boulders.
    pub fn sight_range(self) -> f32 {
        match self {
            EnemyKind::Wolf => 720.0,
            EnemyKind::Bear => 560.0,
            EnemyKind::Raider => 700.0,
            EnemyKind::Jackal => 740.0,
            EnemyKind::Scorpion => 380.0,
            EnemyKind::Jaguar => 760.0,
            EnemyKind::Boa => 300.0,
            EnemyKind::Lion => 780.0,
            EnemyKind::Buffalo => 420.0,
        }
    }

    /// Noise carries through cover: inside this radius the survivor is heard
    /// even with trees and boulders in the way.
    pub fn hearing_range(self) -> f32 {
        match self {
            EnemyKind::Wolf => 340.0,
            EnemyKind::Bear => 300.0,
            EnemyKind::Raider => 380.0,
            EnemyKind::Jackal => 360.0,
            EnemyKind::Scorpion => 220.0,
            EnemyKind::Jaguar => 340.0,
            EnemyKind::Boa => 240.0,
            EnemyKind::Lion => 360.0,
            EnemyKind::Buffalo => 300.0,
        }
    }

    pub fn radius(self) -> f32 {
        match self {
            EnemyKind::Wolf => 16.0,
            EnemyKind::Bear => 23.0,
            EnemyKind::Raider => 17.0,
            EnemyKind::Jackal => 15.0,
            EnemyKind::Scorpion => 12.0,
            EnemyKind::Jaguar => 18.0,
            EnemyKind::Boa => 20.0,
            EnemyKind::Lion => 22.0,
            EnemyKind::Buffalo => 25.0,
        }
    }

    pub fn loot(self) -> (Resource, f32) {
        match self {
            EnemyKind::Wolf => (Resource::Food, 5.0),
            EnemyKind::Bear => (Resource::Food, 12.0),
            EnemyKind::Raider => (Resource::Metal, 6.0),
            EnemyKind::Jackal => (Resource::Food, 4.0),
            EnemyKind::Scorpion => (Resource::Food, 3.0),
            EnemyKind::Jaguar => (Resource::Food, 9.0),
            EnemyKind::Boa => (Resource::Food, 10.0),
            EnemyKind::Lion => (Resource::Food, 14.0),
            EnemyKind::Buffalo => (Resource::Food, 16.0),
        }
    }

}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Enemy {
    #[serde(with = "crate::save::vec2_serde")]
    pub pos: Vec2,
    pub kind: EnemyKind,
    pub hp: f32,
    pub max_hp: f32,
    pub facing: f32,
    pub attack_cd: f32,
    pub hit_flash: f32,
    pub bleed: f32,
    /// Raiders only: cooldown before the next thrown spear.
    pub throw_cd: f32,
    /// -1 or 1: which side this enemy prefers to flank from.
    pub flank: f32,
    /// Animals ignore the survivor until they have actually spotted them.
    /// Defaulted so saves written before detection existed still load.
    #[serde(default)]
    pub alerted: bool,
    /// Seconds since this animal last had eyes on the survivor.
    #[serde(default)]
    pub lost_timer: f32,
    /// Where this animal lives. It wanders around this point and never leaves
    /// its biome on its own; raiders ignore it and roam. Defaulted to zero for
    /// saves written before biomes existed, then set to the enemy position.
    #[serde(default, with = "crate::save::vec2_serde")]
    pub home: Vec2,
}

/// How long an animal keeps hunting after it loses sight of the survivor
/// before it gives up and goes back to grazing.
const LOSE_INTEREST_TIME: f32 = 12.0;

/// Never more than this many wolves, bears and raiders alive at once.
const MAX_ENEMIES: usize = 10;

impl Enemy {
    pub fn radius(&self) -> f32 {
        self.kind.radius()
    }
}

pub struct Floater {
    pub pos: Vec2,
    pub text: String,
    pub color: Color,
    pub life: f32,
    pub max_life: f32,
}

/// An arrow fired by the survivor's ranged weapon.
pub struct Arrow {
    pub pos: Vec2,
    pub vel: Vec2,
    pub damage: f32,
    pub life: f32,
}

/// A spear thrown by a raider.
struct Particle {
    pos: Vec2,
    vel: Vec2,
    life: f32,
    max_life: f32,
    size: f32,
    color: Color,
    drag: f32,
    gravity: f32,
}

fn burst(
    pos: Vec2,
    color: Color,
    count: usize,
    speed: f32,
    size: f32,
    gravity: f32,
) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count);
    for _ in 0..count {
        let angle = gen_range(0.0, TAU);
        let velocity = gen_range(speed * 0.35, speed);
        let life = gen_range(0.28, 0.68);
        particles.push(Particle {
            pos,
            vel: vec2(angle.cos(), angle.sin()) * velocity,
            life,
            max_life: life,
            size: gen_range(size * 0.55, size),
            color,
            drag: gen_range(2.5, 5.5),
            gravity,
        });
    }
    particles
}

pub struct EnemyShot {
    pub pos: Vec2,
    pub vel: Vec2,
    pub damage: f32,
    pub life: f32,
}

/// A rifle round or shotgun pellet fired by the survivor.
pub struct Bullet {
    pub pos: Vec2,
    pub vel: Vec2,
    pub damage: f32,
    pub life: f32,
    pub tracer: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DecorKind {
    Tuft,
    Flower,
    Stone,
    Patch,
}

struct Decor {
    pos: Vec2,
    size: f32,
    kind: DecorKind,
    tint: f32,
}

/// A straw archery target. It soaks every arrow and round and never breaks, so
/// there is always something to aim at between fights and harvest runs.
pub struct PracticeTarget {
    pub pos: Vec2,
    /// Counts down after a hit so the face flashes on impact.
    pub flash: f32,
    /// Impacts landed on this target during the current frame. A shotgun
    /// shell puts nine pellets into the face at once, so they are merged into
    /// a single label instead of stacking nine of them.
    pending_hits: u32,
    pending_bullseye: bool,
}

// ---------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------

pub struct World {
    pub nodes: Vec<ResourceNode>,
    pub enemies: Vec<Enemy>,
    pub arrows: Vec<Arrow>,
    pub bullets: Vec<Bullet>,
    pub enemy_shots: Vec<EnemyShot>,
    pub floaters: Vec<Floater>,
    particles: Vec<Particle>,
    decor: Vec<Decor>,
    pub targets: Vec<PracticeTarget>,
    /// Ponds, oases and river stretches. Impassable, and impossible to build on.
    pub water: Vec<Water>,
    /// Chunk the loaded content was last built around.
    stream_center: Option<(i32, i32)>,
    /// Hit points remembered for nodes outside the loaded block, keyed by their
    /// rounded world position. Zero means that spot has been mined out.
    node_hp: HashMap<(i32, i32), f32>,
    /// Biome the survivor is standing in, so entering a region can be announced.
    last_biome: Biome,
    /// Session-only practice score, cleared from the range sign.
    pub practice_hits: u32,
    pub practice_bullseyes: u32,
    pub spawn_timer: f32,
    pub elapsed: f32,
    /// Highest mastery level that living enemies have been scaled to.
    hp_level: usize,
    footstep_timer: f32,
}

impl World {
    pub fn new() -> Self {
        let mut world = Self {
            nodes: Vec::new(),
            enemies: Vec::new(),
            arrows: Vec::new(),
            bullets: Vec::new(),
            enemy_shots: Vec::new(),
            floaters: Vec::new(),
            particles: Vec::new(),
            decor: Vec::new(),
            targets: Vec::new(),
            water: Vec::new(),
            stream_center: None,
            node_hp: HashMap::new(),
            last_biome: Biome::Meadow,
            practice_hits: 0,
            practice_bullseyes: 0,
            spawn_timer: 4.0,
            elapsed: 0.0,
            hp_level: 1,
            footstep_timer: 0.0,
        };
        world.generate();
        world
    }

    pub fn from_saved(
        nodes: Vec<ResourceNode>,
        enemies: Vec<Enemy>,
        elapsed: f32,
        hp_level: usize,
        node_hp: Vec<((i32, i32), f32)>,
        player_pos: Vec2,
    ) -> Self {
        let mut world = Self::new();
        // Saves from before the map streamed carried a flat list of nodes. Keep
        // whatever damage they remember as overrides for the generator.
        for n in nodes {
            if n.hp < n.max_hp {
                world.node_hp.insert(node_key(n.pos), n.hp);
            }
        }
        for (key, hp) in node_hp {
            world.node_hp.insert(key, hp);
        }
        world.enemies = enemies;
        // Saves from before biomes have no home point; treat where they are
        // standing as where they live.
        for e in world.enemies.iter_mut() {
            if e.home.length_squared() < 1.0 {
                e.home = e.pos;
            }
        }
        world.arrays_clear();
        world.elapsed = elapsed;
        world.hp_level = hp_level.max(1);
        world.spawn_timer = 4.0;
        world.footstep_timer = 0.0;
        // Rebuild the ground around wherever the survivor was left.
        world.stream_center = None;
        world.stream(player_pos);
        world
    }

    fn arrays_clear(&mut self) {
        self.arrows.clear();
        self.bullets.clear();
        self.enemy_shots.clear();
        self.floaters.clear();
        self.particles.clear();
    }

    pub fn saved_hp_level(&self) -> usize {
        self.hp_level
    }

    /// Every node whose condition differs from its pristine state, so mining
    /// survives a reload even if the chunk is not currently loaded.
    pub fn node_overrides(&self) -> Vec<((i32, i32), f32)> {
        let mut all = self.node_hp.clone();
        for n in self.nodes.iter() {
            if n.hp < n.max_hp {
                all.insert(node_key(n.pos), n.hp);
            }
        }
        all.into_iter().collect()
    }

    pub fn float_text(&mut self, pos: Vec2, text: &str, color: Color) {
        self.floaters.push(Floater {
            pos,
            text: text.to_string(),
            color,
            life: 1.0,
            max_life: 1.0,
        });
    }

    pub fn is_water(&self, pos: Vec2) -> bool {
        self.water.iter().any(|w| w.pos.distance(pos) < w.radius)
    }

    fn generate(&mut self) {
        // The practice range is the only fixed landmark. Everything else is
        // streamed in chunk by chunk around the survivor, forever.
        let center = home();
        self.targets.clear();
        for off in [vec2(300.0, -170.0), vec2(330.0, 0.0), vec2(300.0, 170.0)] {
            self.targets.push(PracticeTarget {
                pos: center + off,
                flash: 0.0,
                pending_hits: 0,
                pending_bullseye: false,
            });
        }
        self.stream_center = None;
        self.stream(center);
    }

    /// Rebuild the world around a point when the survivor reaches a new chunk.
    /// Chunks that fall out of range are dropped, with any mining remembered so
    /// a depleted outcrop never quietly grows back.
    pub fn stream(&mut self, around: Vec2) {
        let cc = chunk_of(around);
        if self.stream_center == Some(cc) {
            return;
        }
        for n in self.nodes.iter() {
            if n.hp < n.max_hp {
                self.node_hp.insert(node_key(n.pos), n.hp);
            }
        }
        self.stream_center = Some(cc);
        self.nodes.clear();
        self.decor.clear();
        self.water.clear();
        for dy in -CHUNK_MARGIN..=CHUNK_MARGIN {
            for dx in -CHUNK_MARGIN..=CHUNK_MARGIN {
                self.load_chunk(cc.0 + dx, cc.1 + dy);
            }
        }
        // Keep the practice range clear of trees and boulders.
        let spots: Vec<Vec2> = self.targets.iter().map(|t| t.pos).collect();
        self.nodes.retain(|n| {
            !n.kind.solid() || spots.iter().all(|p| p.distance(n.pos) > n.radius * 0.7 + 34.0)
        });
    }

    /// Generate one chunk. Everything comes from the chunk coordinates, so
    /// walking away and back finds exactly the same trees, pools and rocks.
    fn load_chunk(&mut self, cx: i32, cy: i32) {
        let mut rng = ChunkRng::new(cx, cy);
        let origin = vec2(cx as f32 * CHUNK, cy as f32 * CHUNK);

        // Water: some chunks hold a pool, a few hold a short winding river.
        if rng.f32() < 0.22 {
            let pos = origin
                + vec2(rng.range(130.0, CHUNK - 130.0), rng.range(130.0, CHUNK - 130.0));
            let radius = rng.range(95.0, 175.0);
            let seed = rng.range(0.0, 10.0);
            // Never flood the camp or the practice range.
            if pos.distance(home()) > 470.0 + radius {
                self.water.push(Water { pos, radius, seed });
            }
        } else if rng.f32() < 0.30 {
            let start = origin
                + vec2(rng.range(120.0, CHUNK - 120.0), rng.range(120.0, CHUNK - 120.0));
            let dir = vec2(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalize_or_zero();
            for k in 0..3 {
                let pos = start + dir * (k as f32 * 135.0);
                let radius = rng.range(85.0, 122.0);
                let seed = rng.range(0.0, 10.0);
                if pos.distance(home()) > 470.0 + radius {
                    self.water.push(Water { pos, radius, seed });
                }
            }
        }

        // Ground clutter, chosen to suit the region.
        for _ in 0..44 {
            let pos = origin
                + vec2(rng.range(8.0, CHUNK - 8.0), rng.range(8.0, CHUNK - 8.0));
            let roll = rng.f32();
            let size = rng.range(6.0, 20.0);
            let tint = rng.f32();
            if self.is_water(pos) {
                continue;
            }
            let kind = match biome_at(pos) {
                Biome::Meadow => {
                    if roll < 0.56 {
                        DecorKind::Tuft
                    } else if roll < 0.72 {
                        DecorKind::Patch
                    } else if roll < 0.89 {
                        DecorKind::Flower
                    } else {
                        DecorKind::Stone
                    }
                }
                Biome::Desert => {
                    if roll < 0.30 {
                        DecorKind::Tuft
                    } else if roll < 0.56 {
                        DecorKind::Stone
                    } else {
                        DecorKind::Patch
                    }
                }
                Biome::Rainforest => {
                    if roll < 0.60 {
                        DecorKind::Tuft
                    } else if roll < 0.72 {
                        DecorKind::Patch
                    } else if roll < 0.96 {
                        DecorKind::Flower
                    } else {
                        DecorKind::Stone
                    }
                }
                Biome::Savanna => {
                    if roll < 0.50 {
                        DecorKind::Tuft
                    } else if roll < 0.80 {
                        DecorKind::Patch
                    } else if roll < 0.90 {
                        DecorKind::Flower
                    } else {
                        DecorKind::Stone
                    }
                }
            };
            self.decor.push(Decor {
                pos,
                size,
                kind,
                tint,
            });
        }

        // Resource nodes, spaced out inside the chunk.
        let mut spots: Vec<Vec2> = Vec::new();
        for _ in 0..7 {
            let pos = origin
                + vec2(rng.range(70.0, CHUNK - 70.0), rng.range(70.0, CHUNK - 70.0));
            let roll = rng.f32();
            let seed = rng.range(0.0, 10.0);
            if self.is_water(pos) || pos.distance(home()) < 210.0 {
                continue;
            }
            if spots.iter().any(|p| p.distance(pos) < 78.0)
                || self.nodes.iter().any(|n| n.pos.distance(pos) < 78.0)
            {
                continue;
            }
            spots.push(pos);
            let kind = match biome_at(pos) {
                Biome::Meadow => {
                    if roll < 0.24 {
                        NodeKind::Berry
                    } else if roll < 0.58 {
                        NodeKind::Tree
                    } else if roll < 0.84 {
                        NodeKind::Rock
                    } else {
                        NodeKind::Iron
                    }
                }
                Biome::Desert => {
                    if roll < 0.34 {
                        NodeKind::Berry
                    } else if roll < 0.72 {
                        NodeKind::Rock
                    } else {
                        NodeKind::Iron
                    }
                }
                Biome::Rainforest => {
                    if roll < 0.34 {
                        NodeKind::Berry
                    } else if roll < 0.82 {
                        NodeKind::Tree
                    } else if roll < 0.94 {
                        NodeKind::Rock
                    } else {
                        NodeKind::Iron
                    }
                }
                Biome::Savanna => {
                    if roll < 0.30 {
                        NodeKind::Berry
                    } else if roll < 0.50 {
                        NodeKind::Tree
                    } else if roll < 0.86 {
                        NodeKind::Rock
                    } else {
                        NodeKind::Iron
                    }
                }
            };
            self.push_generated_node(pos, kind, seed);
        }

        // The camp keeps its friendly starter cluster.
        if (cx, cy) == chunk_of(home()) {
            let c = home();
            let starters = [
                (vec2(-150.0, -70.0), NodeKind::Tree),
                (vec2(130.0, 55.0), NodeKind::Berry),
                (vec2(-60.0, 160.0), NodeKind::Rock),
                (vec2(200.0, -120.0), NodeKind::Tree),
                (vec2(-220.0, 90.0), NodeKind::Iron),
                (vec2(70.0, -200.0), NodeKind::Berry),
                (vec2(240.0, 190.0), NodeKind::Rock),
            ];
            for (i, (off, kind)) in starters.into_iter().enumerate() {
                let pos = c + off;
                if !self.nodes.iter().any(|n| n.pos.distance(pos) < 64.0) {
                    self.push_generated_node(pos, kind, i as f32 * 1.7);
                }
            }
        }
    }

    /// Add a generated node unless its spot has already been mined out.
    fn push_generated_node(&mut self, pos: Vec2, kind: NodeKind, seed: f32) {
        let stored = self.node_hp.get(&node_key(pos)).copied();
        if matches!(stored, Some(hp) if hp <= 0.0) {
            return;
        }
        let max_hp = kind.max_hp();
        self.nodes.push(ResourceNode {
            pos,
            kind,
            hp: stored.unwrap_or(max_hp).min(max_hp),
            max_hp,
            radius: kind.radius(),
            shake: 0.0,
            seed,
        });
    }

    fn push_node(&mut self, pos: Vec2, kind: NodeKind) {
        self.nodes.push(ResourceNode {
            pos,
            kind,
            hp: kind.max_hp(),
            max_hp: kind.max_hp(),
            radius: kind.radius(),
            shake: 0.0,
            seed: gen_range(0.0, 10.0),
        });
    }

    /// True when nothing solid - tree, boulder or iron outcrop - sits on the
    /// line between two points.
    fn clear_path(&self, from: Vec2, to: Vec2) -> bool {
        let delta = to - from;
        let len_sq = delta.length_squared();
        if len_sq < 1.0 {
            return true;
        }
        for n in self.nodes.iter() {
            if !n.kind.solid() {
                continue;
            }
            let t = ((n.pos - from).dot(delta) / len_sq).clamp(0.0, 1.0);
            let closest = from + delta * t;
            if n.pos.distance(closest) < n.radius * 0.7 + 12.0 {
                return false;
            }
        }
        true
    }

    fn spawn_enemy(&mut self, player: &Player) {
        let player_pos = player.pos;
        let angle: f32 = gen_range(0.0, TAU);
        let dist: f32 = gen_range(640.0, 960.0);
        let mut pos = player_pos + vec2(angle.cos() * dist, angle.sin() * dist);
        if pos.distance(player_pos) < 430.0 {
            let inward = (home() - player_pos).normalize_or_zero();
            let inward = if inward.length_squared() < 0.5 {
                vec2(1.0, 0.0)
            } else {
                inward
            };
            pos = player_pos + inward * 640.0;
        }
        // Nothing spawns in a lake or river.
        if let Some(w) = self
            .water
            .iter()
            .find(|w| w.pos.distance(pos) < w.radius + 50.0)
        {
            let away = pos - w.pos;
            let away = if away.length_squared() < 0.5 {
                vec2(1.0, 0.0)
            } else {
                away.normalize()
            };
            pos = w.pos + away * (w.radius + 50.0);
        }
        // Raiders are people and turn up anywhere; animals come from whichever
        // region the spawn point landed in.
        let roll: f32 = gen_range(0.0, 1.0);
        let biome = biome_at(pos);
        let mut kind = if self.elapsed > 80.0 && roll > 0.7 {
            EnemyKind::Raider
        } else {
            let roster = biome.animals();
            let pick = (gen_range(0.0, roster.len() as f32) as usize).min(roster.len() - 1);
            roster[pick]
        };
        // Wolves are the common early threat in the meadow; bears start
        // turning up once the survivor has had a chance to arm up.
        if kind == EnemyKind::Bear && self.elapsed < 45.0 {
            kind = EnemyKind::Wolf;
        }
        let level = player.melee_level.max(player.ranged_level).max(1);
        let scale = 1.0 + (level.saturating_sub(1)) as f32 * 0.18;
        let hp = kind.max_hp() * scale;
        self.enemies.push(Enemy {
            pos,
            kind,
            hp,
            max_hp: hp,
            facing: 0.0,
            attack_cd: gen_range(0.3, 1.0),
            hit_flash: 0.0,
            bleed: 0.0,
            throw_cd: gen_range(2.4, 3.8),
            flank: if gen_range(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 },
            alerted: false,
            lost_timer: 0.0,
            home: pos,
        });
    }

    /// Every mastery level the survivor gains also hardens the animals that are
    /// already on the map, not just future spawns.
    fn scale_enemy_health(&mut self, player: &Player) {
        let level = player.melee_level.max(player.ranged_level).max(1);
        if level <= self.hp_level {
            return;
        }
        let old_scale = 1.0 + (self.hp_level - 1) as f32 * 0.18;
        let new_scale = 1.0 + (level - 1) as f32 * 0.18;
        let ratio = new_scale / old_scale;
        for e in self.enemies.iter_mut() {
            e.max_hp *= ratio;
            e.hp = (e.hp * ratio).min(e.max_hp);
        }
        self.hp_level = level;
    }

    // -----------------------------------------------------------------------
    // Simulation
    // -----------------------------------------------------------------------

    pub fn update(
        &mut self,
        dt: f32,
        player: &mut Player,
        buildings: &mut BuildingManager,
        progress: &mut Progression,
    ) {
        self.elapsed += dt;

        // Announce the region the survivor walks into.
        let here = biome_at(player.pos);
        if here != self.last_biome {
            self.last_biome = here;
            progress.show_banner(here.name(), here.subtitle());
        }

        // Stream fresh ground in as the survivor explores.
        self.stream(player.pos);

        for node in self.nodes.iter_mut() {
            if node.shake > 0.0 {
                node.shake = (node.shake - dt).max(0.0);
            }
        }

        for t in self.targets.iter_mut() {
            if t.flash > 0.0 {
                t.flash = (t.flash - dt).max(0.0);
            }
        }

        for f in self.floaters.iter_mut() {
            f.life -= dt;
            f.pos.y -= 38.0 * dt;
        }
        self.floaters.retain(|f| f.life > 0.0);

        for p in self.particles.iter_mut() {
            p.life -= dt;
            p.vel *= (1.0 - p.drag * dt).clamp(0.0, 1.0);
            p.vel.y += p.gravity * dt;
            p.pos += p.vel * dt;
        }
        self.particles.retain(|p| p.life > 0.0);

        self.footstep_timer -= dt;
        if player.moving && self.footstep_timer <= 0.0 {
            self.footstep_timer = 0.17;
            let side = vec2(-player.aim.sin(), player.aim.cos());
            let step_side = player.walk_phase.sin().signum() * 5.0;
            self.particles.extend(burst(
                player.pos + side * step_side + vec2(0.0, 9.0),
                Color::new(0.68, 0.61, 0.43, 0.45),
                1,
                26.0,
                4.2,
                -12.0,
            ));
        }

        self.update_arrows(dt, player, progress);
        self.update_bullets(dt, player, progress);
        self.update_enemy_shots(dt, player);
        self.flush_target_hits();
        self.scale_enemy_health(player);

        self.spawn_timer -= dt;
        // Hard ceiling on how many creatures can be alive at once.
        let cap = MAX_ENEMIES;
        if self.spawn_timer <= 0.0 && self.enemies.len() < cap {
            // Slow, long-game trickle: one creature every 10s early on, never
            // faster than one every 3.5s no matter how long you survive.
            self.spawn_timer = (10.0 - self.elapsed * 0.012).max(3.5);
            self.spawn_enemy(player);
        }

        let mut new_shots: Vec<EnemyShot> = Vec::new();
        for i in 0..self.enemies.len() {
            let epos = self.enemies[i].pos;
            let kind = self.enemies[i].kind;
            let er = kind.radius();
            let flank = self.enemies[i].flank;
            let to_player = player.pos - epos;
            let dist_player = to_player.length();
            let dir_to_player = to_player.normalize_or_zero();
            let perp = vec2(-dir_to_player.y, dir_to_player.x);

            // Animals only become a threat once they have spotted you: inside
            // their sight range and with a clear line through the trees. Break
            // that line for long enough and they lose interest and wander off.
            let sees_player =
                dist_player < kind.sight_range() && self.clear_path(epos, player.pos);
            // Crashing through the undergrowth is loud; cover only hides you
            // from a distance.
            let hears_player = dist_player < kind.hearing_range();
            // At touching distance there is nothing left to hide behind.
            let pressed = dist_player < er + player.radius + 60.0;
            if sees_player || hears_player || pressed {
                self.enemies[i].lost_timer = 0.0;
                if !self.enemies[i].alerted {
                    self.enemies[i].alerted = true;
                    if kind != EnemyKind::Raider {
                        self.floaters.push(Floater {
                            pos: epos + vec2(0.0, -30.0),
                            text: "!".to_string(),
                            color: Color::new(1.0, 0.85, 0.35, 1.0),
                            life: 0.8,
                            max_life: 0.8,
                        });
                    }
                }
            } else if self.enemies[i].alerted && kind != EnemyKind::Raider {
                self.enemies[i].lost_timer += dt;
                if self.enemies[i].lost_timer > LOSE_INTEREST_TIME {
                    // Hunt over: back to grazing until spotted again.
                    self.enemies[i].alerted = false;
                    self.enemies[i].lost_timer = 0.0;
                }
            }
            let hunt = kind == EnemyKind::Raider || self.enemies[i].alerted;

            // Only barriers genuinely on the path are worth breaking. Anything
            // beside the route is stepped around, like a person would do.
            let mut path_blocked = false;
            let mut blocking: Option<usize> = None;
            let mut blocking_dist = f32::MAX;
            for (bi, b) in buildings.items.iter().enumerate() {
                if b.hp <= 0.0 || !b.blocks(false) {
                    continue;
                }
                let ab = b.pos - epos;
                if ab.dot(dir_to_player) < -er {
                    continue;
                }
                let t = if to_player.length_squared() > 1.0 {
                    (ab.dot(to_player) / to_player.length_squared()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let closest = epos + to_player * t;
                if b.pos.distance(closest) < er + 34.0 {
                    path_blocked = true;
                    let d = ab.length();
                    if d < blocking_dist {
                        blocking_dist = d;
                        blocking = Some(bi);
                    }
                }
            }

            // Attacks still need eyes on the survivor: something heard through
            // the trees will close in, but it cannot hit what it cannot see.
            let attack_player = hunt && sees_player && dist_player < er + player.radius + 12.0;
            let mut attack_building: Option<usize> = None;
            if hunt && !attack_player {
                if let Some(bi) = blocking {
                    if blocking_dist < er + 44.0 {
                        attack_building = Some(bi);
                    }
                }
            }

            let smart = kind == EnemyKind::Raider;
            let mut desired = Vec2::ZERO;

            if !hunt {
                // Unaware animals graze inside their home range: a lazy orbit
                // around the spot they spawned on. If anything pushes them into
                // another biome they walk back home instead of drifting on.
                let home = self.enemies[i].home;
                if biome_at(epos) != biome_at(home) {
                    desired = (home - epos).normalize_or_zero() * kind.speed();
                } else {
                    let t = self.elapsed * 0.6 + i as f32 * 2.1;
                    let mut target = home + vec2(t.sin(), (t * 1.3).cos()) * 190.0;
                    // Grazing spots never lead into a lake or a neighbouring
                    // biome; in that case the animal simply drifts home.
                    if self.is_water(target) || biome_at(target) != biome_at(home) {
                        target = home;
                    }
                    let to_target = target - epos;
                    desired = if to_target.length() < 26.0 {
                        Vec2::ZERO
                    } else {
                        to_target.normalize_or_zero() * kind.speed()
                    };
                }
            } else if attack_player {
                self.enemies[i].facing = dir_to_player.y.atan2(dir_to_player.x);
            } else if let Some(bi) = attack_building {
                let bpos = buildings.items[bi].pos;
                self.enemies[i].facing = (bpos.y - epos.y).atan2(bpos.x - epos.x);
            } else {
                let flank_offset = perp * flank * if smart { 84.0 } else { 40.0 };
                let mut target = player.pos + flank_offset;
                if dist_player < 120.0 {
                    target = player.pos;
                }
                let mut steer = (target - epos).normalize_or_zero();

                // Raiders are people: they keep their distance, circle and
                // throw. Wolves and bears commit to a flanking charge.
                if smart && !path_blocked {
                    if dist_player < 155.0 {
                        steer = (-dir_to_player * 0.8 + perp * flank * 0.7).normalize_or_zero();
                    } else if dist_player < 330.0 {
                        steer = (perp * flank * 0.9 + dir_to_player * 0.15).normalize_or_zero();
                    }
                }

                // Slide around walls and gates instead of grinding into them.
                for b in buildings.items.iter() {
                    if b.hp <= 0.0 || !b.blocks(false) {
                        continue;
                    }
                    let off = epos - b.pos;
                    let safe = er + 40.0;
                    if off.length() < safe {
                        let tangent = vec2(-off.y, off.x).normalize_or_zero();
                        let sign = if tangent.dot(steer) >= 0.0 { 1.0 } else { -1.0 };
                        steer = (steer + tangent * sign * 1.5).normalize_or_zero();
                    } else if path_blocked {
                        let ahead = b.pos - epos;
                        if ahead.dot(steer) > 0.0 && ahead.length() < 96.0 {
                            let tangent = vec2(-ahead.y, ahead.x).normalize_or_zero() * flank;
                            steer = (steer * 0.5 + tangent).normalize_or_zero();
                        }
                    }
                }

                // Trees, boulders and iron outcrops are solid for animals too.
                for n in self.nodes.iter() {
                    if !n.kind.solid() {
                        continue;
                    }
                    let off = epos - n.pos;
                    let safe = er + n.radius * 0.7 + 18.0;
                    if off.length_squared() > 0.01 && off.length() < safe {
                        let tangent = vec2(-off.y, off.x).normalize_or_zero();
                        let sign = if tangent.dot(steer) >= 0.0 { 1.0 } else { -1.0 };
                        steer = (steer + tangent * sign * 1.4).normalize_or_zero();
                    }
                }

                // Personal space: nobody stacks on top of a packmate.
                for (j, other) in self.enemies.iter().enumerate() {
                    if j == i {
                        continue;
                    }
                    let off = epos - other.pos;
                    let min = er + other.radius() + 8.0;
                    if off.length_squared() > 0.01 && off.length() < min {
                        let push = 1.0 - off.length() / min;
                        steer = (steer + off.normalize() * push * 1.3).normalize_or_zero();
                    }
                }

                desired = steer * kind.speed();
            }

            if desired.length_squared() > 1.0 {
                let pace = if hunt { 1.0 } else { 0.35 };
                let step = desired.normalize() * kind.speed() * pace * dt;
                let mut np = epos + step;
                for b in buildings.items.iter() {
                    if b.blocks(false) {
                        let off = np - b.pos;
                        let min_dist = er + 24.0;
                        if off.length_squared() < min_dist * min_dist {
                            let push = if off.length_squared() > 0.01 {
                                off.normalize()
                            } else {
                                vec2(1.0, 0.0)
                            };
                            np = b.pos + push * min_dist;
                        }
                    }
                }
                // Hard collision with solid resource nodes. The same radii the
                // player uses, so no animal can walk through a tree or boulder.
                for n in self.nodes.iter() {
                    if !n.kind.solid() {
                        continue;
                    }
                    let off = np - n.pos;
                    let min_dist = er + n.radius * 0.7;
                    if off.length_squared() < min_dist * min_dist {
                        let push = if off.length_squared() > 0.01 {
                            off.normalize()
                        } else {
                            vec2(1.0, 0.0)
                        };
                        np = n.pos + push * min_dist;
                    }
                }
                // Wandering animals never leave their biome. Water stops
                // everyone, hunter or not; nothing here can swim.
                if !hunt && biome_at(np) != biome_at(self.enemies[i].home) {
                    // Never wander out of the home biome, but allow a displaced
                    // animal to walk back toward its home point.
                    let home = self.enemies[i].home;
                    if np.distance(home) >= epos.distance(home) {
                        np = epos;
                    }
                }
                // Water is impassable; slide along the bank instead of stopping
                // dead when only one axis is wet.
                if self.is_water(np) {
                    let slide_x = vec2(np.x, epos.y);
                    let slide_y = vec2(epos.x, np.y);
                    np = if !self.is_water(slide_x) {
                        slide_x
                    } else if !self.is_water(slide_y) {
                        slide_y
                    } else {
                        epos
                    };
                }
                self.enemies[i].pos = np;
                self.enemies[i].facing = desired.y.atan2(desired.x);
            }

            if self.enemies[i].attack_cd > 0.0 {
                self.enemies[i].attack_cd -= dt;
            }
            if self.enemies[i].hit_flash > 0.0 {
                self.enemies[i].hit_flash -= dt;
            }
            if self.enemies[i].bleed > 0.0 {
                self.enemies[i].bleed -= dt;
                self.enemies[i].hp -= 4.5 * dt;
            }

            if attack_player && self.enemies[i].attack_cd <= 0.0 {
                self.enemies[i].attack_cd = 0.8;
                let dmg = kind.damage() * player.enemy_damage_multiplier();
                let lost = player.take_damage(dmg);
                if lost > 0.0 {
                    self.floaters.push(Floater {
                        pos: player.pos + vec2(0.0, -34.0),
                        text: format!("-{:.0}", lost),
                        color: Color::new(1.0, 0.34, 0.34, 1.0),
                        life: 0.8,
                        max_life: 0.8,
                    });
                }
            }

            if let Some(bi) = attack_building {
                if self.enemies[i].attack_cd <= 0.0 {
                    self.enemies[i].attack_cd = 1.0;
                    let dmg = kind.damage() * 1.4;
                    buildings.items[bi].hp -= dmg;
                    buildings.items[bi].flash = 0.22;
                    let bpos = buildings.items[bi].pos;
                    self.enemies[i].facing = (bpos.y - epos.y).atan2(bpos.x - epos.x);
                    self.floaters.push(Floater {
                        pos: bpos + vec2(0.0, -32.0),
                        text: format!("-{}", dmg.round() as i32),
                        color: Color::new(1.0, 0.6, 0.28, 1.0),
                        life: 0.7,
                        max_life: 0.7,
                    });
                }
            }

            // A raider with a clear line throws a spear instead of closing in.
            if smart && !attack_player && attack_building.is_none() {
                let e = &mut self.enemies[i];
                e.throw_cd -= dt;
                if e.throw_cd <= 0.0
                    && sees_player
                    && dist_player < 430.0
                    && dist_player > 70.0
                    && !path_blocked
                {
                    e.throw_cd = 3.4;
                    let aim = (player.pos - epos).normalize_or_zero();
                    e.facing = aim.y.atan2(aim.x);
                    new_shots.push(EnemyShot {
                        pos: epos + aim * 18.0,
                        vel: aim * 440.0,
                        damage: 8.0 * player.enemy_damage_multiplier(),
                        life: 1.3,
                    });
                }
            }
        }
        self.enemy_shots.extend(new_shots);

        // Anything reduced to zero health (bleed, traps, turrets) dies here.
        let dead: Vec<usize> = (0..self.enemies.len())
            .filter(|&i| self.enemies[i].hp <= 0.0)
            .collect();
        for &i in dead.iter().rev() {
            let enemy = self.enemies.remove(i);
            let (res, loot) = enemy.kind.loot();
            player.add_resource(res, loot);
            progress.register_gather(res, loot);
            progress.register_other_kill();
            self.floaters.push(Floater {
                pos: enemy.pos,
                text: format!("+{:.0} {}", loot, res.label()),
                color: res.color(),
                life: 1.1,
                max_life: 1.1,
            });
        }

        // Rubble where a structure was destroyed by the horde.
        let destroyed: Vec<Vec2> = buildings
            .items
            .iter()
            .filter(|b| b.hp <= 0.0)
            .map(|b| b.pos)
            .collect();
        if !destroyed.is_empty() {
            buildings.items.retain(|b| b.hp > 0.0);
            for p in destroyed {
                self.floaters.push(Floater {
                    pos: p,
                    text: "DESTROYED".to_string(),
                    color: Color::new(1.0, 0.52, 0.3, 1.0),
                    life: 1.2,
                    max_life: 1.2,
                });
            }
        }
    }

    /// Fire the equipped ranged weapon toward the cursor.
    pub fn fire_arrow(&mut self, player: &mut Player) -> bool {
        if player.ranged_level == 0 || player.ranged_cd > 0.0 {
            return false;
        }
        let tier = player.ranged();
        player.ranged_cd = tier.cooldown;
        player.shot_flash = 0.18;
        let dir = vec2(player.aim.cos(), player.aim.sin());
        self.arrows.push(Arrow {
            pos: player.pos + dir * 22.0,
            vel: dir * tier.speed,
            damage: tier.damage,
            life: tier.range / tier.speed,
        });
        true
    }

    /// Hold-to-fire rifle: one accurate round per short cooldown.
    pub fn fire_auto(&mut self, player: &mut Player) -> bool {
        if player.ranged_cd > 0.0 {
            return false;
        }
        player.ranged_cd = player.gun_cooldown(0.10);
        player.shot_flash = 0.06;
        let speed_scale = 1.0 + (player.combat_level() - 1) as f32 * 0.012;
        let angle = player.aim + gen_range(-0.055, 0.055);
        let dir = vec2(angle.cos(), angle.sin());
        self.bullets.push(Bullet {
            pos: player.pos + dir * 26.0,
            vel: dir * 1180.0 * speed_scale,
            damage: player.gun_damage(RIFLE_BASE_DAMAGE),
            life: 0.62,
            tracer: 1.0,
        });
        true
    }

    /// Birdshot: a short-range cloud of pellets with a wide spread.
    pub fn fire_shotgun(&mut self, player: &mut Player) -> bool {
        if player.ranged_cd > 0.0 {
            return false;
        }
        player.ranged_cd = player.gun_cooldown(0.90);
        player.shot_flash = 0.14;
        let speed_scale = 1.0 + (player.combat_level() - 1) as f32 * 0.012;
        for _ in 0..9 {
            let angle = player.aim + gen_range(-0.34, 0.34);
            let speed = gen_range(760.0, 900.0) * speed_scale;
            let dir = vec2(angle.cos(), angle.sin());
            self.bullets.push(Bullet {
                pos: player.pos + dir * 24.0,
                vel: dir * speed,
                damage: player.gun_damage(SHOTGUN_PELLET_DAMAGE),
                life: 300.0 / speed,
                tracer: 0.45,
            });
        }
        true
    }

    fn update_bullets(&mut self, dt: f32, player: &mut Player, progress: &mut Progression) {
        for bi in 0..self.bullets.len() {
            let pos = self.bullets[bi].pos + self.bullets[bi].vel * dt;
            self.bullets[bi].pos = pos;
            self.bullets[bi].life -= dt;
            if self.bullets[bi].life <= 0.0 {
                continue;
            }
            let mut hit_enemy: Option<usize> = None;
            for (ei, e) in self.enemies.iter().enumerate() {
                if e.hp > 0.0 && e.pos.distance(pos) < e.radius() + 5.0 {
                    hit_enemy = Some(ei);
                    break;
                }
            }
            if let Some(ei) = hit_enemy {
                let dmg = self.bullets[bi].damage;
                self.enemies[ei].hp -= dmg;
                self.enemies[ei].hit_flash = 0.12;
                // Being shot wakes an animal up, clear line of sight or not.
                self.enemies[ei].alerted = true;
                self.bullets[bi].life = 0.0;
                progress.register_ranged_hit();
                let epos = self.enemies[ei].pos;
                self.particles.extend(burst(
                    epos,
                    Color::new(1.0, 0.72, 0.28, 0.9),
                    5,
                    170.0,
                    2.6,
                    160.0,
                ));
                self.floaters.push(Floater {
                    pos: epos + vec2(0.0, -18.0),
                    text: format!("{:.0}", dmg),
                    color: Color::new(1.0, 0.86, 0.42, 1.0),
                    life: 0.55,
                    max_life: 0.55,
                });
                if self.enemies[ei].hp <= 0.0 {
                    let enemy = self.enemies.remove(ei);
                    let (res, loot) = enemy.kind.loot();
                    self.particles.extend(burst(
                        enemy.pos,
                        Color::new(0.55, 0.08, 0.10, 0.85),
                        10,
                        190.0,
                        3.8,
                        170.0,
                    ));
                    player.add_resource(res, loot);
                    progress.register_gather(res, loot);
                    progress.register_ranged_kill();
                    self.floaters.push(Floater {
                        pos: enemy.pos,
                        text: format!("+{:.0} {}", loot, res.label()),
                        color: res.color(),
                        life: 1.1,
                        max_life: 1.1,
                    });
                }
            }
        }
        // Practice targets stop any round that reaches them.
        for bi in 0..self.bullets.len() {
            if self.bullets[bi].life <= 0.0 {
                continue;
            }
            let pos = self.bullets[bi].pos;
            if self.hit_target_at(pos, Color::new(0.98, 0.84, 0.4, 0.9)) {
                self.bullets[bi].life = 0.0;
            }
        }
        self.bullets.retain(|b| b.life > 0.0);
    }

    fn update_arrows(&mut self, dt: f32, player: &mut Player, progress: &mut Progression) {
        for ai in 0..self.arrows.len() {
            let pos = self.arrows[ai].pos + self.arrows[ai].vel * dt;
            self.arrows[ai].pos = pos;
            self.arrows[ai].life -= dt;
            if self.arrows[ai].life <= 0.0 {
                continue;
            }
            let mut hit_enemy: Option<usize> = None;
            for (ei, e) in self.enemies.iter().enumerate() {
                if e.hp > 0.0 && e.pos.distance(pos) < e.radius() + 6.0 {
                    hit_enemy = Some(ei);
                    break;
                }
            }
            if let Some(ei) = hit_enemy {
                let dmg = self.arrows[ai].damage;
                self.enemies[ei].hp -= dmg;
                self.enemies[ei].hit_flash = 0.16;
                self.enemies[ei].alerted = true;
                self.arrows[ai].life = 0.0;
                progress.register_ranged_hit();
                let epos = self.enemies[ei].pos;
                self.particles.extend(burst(
                    epos,
                    Color::new(0.62, 0.94, 1.0, 0.9),
                    5,
                    130.0,
                    2.4,
                    120.0,
                ));
                self.floaters.push(Floater {
                    pos: epos + vec2(0.0, -18.0),
                    text: format!("{:.0}", dmg),
                    color: Color::new(0.72, 0.94, 1.0, 1.0),
                    life: 0.7,
                    max_life: 0.7,
                });
                if self.enemies[ei].hp <= 0.0 {
                    let enemy = self.enemies.remove(ei);
                    let (res, loot) = enemy.kind.loot();
                    self.particles.extend(burst(
                        enemy.pos,
                        Color::new(0.55, 0.08, 0.10, 0.85),
                        10,
                        190.0,
                        3.8,
                        170.0,
                    ));
                    player.add_resource(res, loot);
                    progress.register_gather(res, loot);
                    progress.register_ranged_kill();
                    self.floaters.push(Floater {
                        pos: enemy.pos,
                        text: format!("+{:.0} {}", loot, res.label()),
                        color: res.color(),
                        life: 1.1,
                        max_life: 1.1,
                    });
                }
            }
        }
        for ai in 0..self.arrows.len() {
            if self.arrows[ai].life <= 0.0 {
                continue;
            }
            let pos = self.arrows[ai].pos;
            if self.hit_target_at(pos, Color::new(0.62, 0.94, 1.0, 0.9)) {
                self.arrows[ai].life = 0.0;
            }
        }
        self.arrows.retain(|a| a.life > 0.0);
    }

    /// Registers a hit on a practice target, returning true when one was struck.
    /// Centre hits shout BULLSEYE, everything else just HIT.
    fn hit_target_at(&mut self, pos: Vec2, spark: Color) -> bool {
        let hit = self
            .targets
            .iter()
            .position(|t| t.pos.distance(pos) < 21.0);
        let Some(ti) = hit else {
            return false;
        };
        let tpos = self.targets[ti].pos;
        self.targets[ti].flash = 0.25;
        self.targets[ti].pending_hits += 1;
        let bullseye = tpos.distance(pos) < 7.5;
        self.targets[ti].pending_bullseye |= bullseye;
        self.practice_hits += 1;
        if bullseye {
            self.practice_bullseyes += 1;
        }
        self.particles.extend(burst(pos, spark, 4, 120.0, 2.2, 140.0));
        true
    }

    /// One label per target per frame: a shell that lands nine pellets reads as
    /// a single impact, not nine stacked words.
    fn flush_target_hits(&mut self) {
        for ti in 0..self.targets.len() {
            let hits = self.targets[ti].pending_hits;
            if hits == 0 {
                continue;
            }
            self.targets[ti].pending_hits = 0;
            let bullseye = std::mem::replace(&mut self.targets[ti].pending_bullseye, false);
            let tpos = self.targets[ti].pos;
            self.floaters.push(Floater {
                pos: tpos + vec2(0.0, -34.0),
                text: match (hits, bullseye) {
                    (1, true) => "BULLSEYE".to_string(),
                    (1, false) => "HIT".to_string(),
                    (n, _) => format!("{} HITS", n),
                },
                color: if bullseye {
                    Color::new(1.0, 0.86, 0.35, 1.0)
                } else {
                    Color::new(0.92, 0.94, 0.98, 1.0)
                },
                life: 0.7,
                max_life: 0.7,
            });
        }
    }

    fn update_enemy_shots(&mut self, dt: f32, player: &mut Player) {
        for s in self.enemy_shots.iter_mut() {
            s.pos += s.vel * dt;
            s.life -= dt;
            if s.life > 0.0 && s.pos.distance(player.pos) < player.radius + 7.0 {
                let lost = player.take_damage(s.damage);
                if lost > 0.0 {
                    self.floaters.push(Floater {
                        pos: player.pos + vec2(0.0, -36.0),
                        text: format!("-{:.0}", lost),
                        color: Color::new(1.0, 0.44, 0.32, 1.0),
                        life: 0.85,
                        max_life: 0.85,
                    });
                }
                s.life = 0.0;
            }
        }
        self.enemy_shots.retain(|s| s.life > 0.0);
    }

    /// Depleted nodes are removed and a fresh node of the same kind is placed
    /// further out in the world, keeping the ground stocked as you explore.
    fn spawn_replacement(&mut self, kind: NodeKind, player_pos: Vec2) {
        for _ in 0..160 {
            let angle = gen_range(0.0, TAU);
            let dist = gen_range(430.0, 1250.0);
            let pos = player_pos + vec2(angle.cos() * dist, angle.sin() * dist);
            if self.is_water(pos) {
                continue;
            }
            if self.nodes.iter().any(|n| n.pos.distance(pos) < 78.0) {
                continue;
            }
            self.push_node(pos, kind);
            return;
        }
    }

    /// Swing the equipped weapon: damage enemies in the arc and harvest the
    /// closest resource node in front of the player.
    pub fn attack(&mut self, player: &mut Player, progress: &mut Progression) -> bool {
        if player.swing_cd > 0.0 {
            return false;
        }
        let stats = player.melee();
        player.swing_cd = stats.cooldown;
        player.swing_timer = 0.22;
        player.swing_angle = player.aim;

        let aim = player.aim;
        let origin = player.pos;
        let mut dead: Vec<usize> = Vec::new();

        for (i, e) in self.enemies.iter_mut().enumerate() {
            let off = e.pos - origin;
            let dist = off.length();
            if dist <= stats.reach + e.radius()
                && angle_delta(off.y.atan2(off.x), aim).abs() <= stats.arc
            {
                let dmg = stats.damage * gen_range(0.9, 1.12);
                e.hp -= dmg;
                e.hit_flash = 0.18;
                e.alerted = true;
                if dist > 1.0 {
                    e.pos += off / dist * 8.0;
                }
                self.particles.extend(burst(
                    e.pos,
                    Color::new(0.62, 0.10, 0.12, 0.85),
                    6,
                    145.0,
                    3.2,
                    150.0,
                ));
                let num = dmg.round() as i32;
                self.floaters.push(Floater {
                    pos: e.pos + vec2(0.0, -e.radius() - 10.0),
                    text: format!("{}", num),
                    color: Color::new(1.0, 0.86, 0.32, 1.0),
                    life: 0.75,
                    max_life: 0.75,
                });
                progress.register_melee_hit();
                if e.hp <= 0.0 {
                    dead.push(i);
                }
            }
        }

        for &i in dead.iter().rev() {
            let enemy = self.enemies.remove(i);
            let (res, loot) = enemy.kind.loot();
            player.add_resource(res, loot);
            progress.register_gather(res, loot);
            progress.register_melee_kill();
            self.particles.extend(burst(
                enemy.pos,
                Color::new(0.55, 0.08, 0.10, 0.85),
                10,
                190.0,
                3.8,
                170.0,
            ));
            self.floaters.push(Floater {
                pos: enemy.pos,
                text: format!("+{:.0} {}", loot, res.label()),
                color: res.color(),
                life: 1.1,
                max_life: 1.1,
            });
        }

        let mut target: Option<usize> = None;
        let mut best = f32::MAX;
        for (i, n) in self.nodes.iter().enumerate() {
            let off = n.pos - origin;
            let dist = off.length();
            if dist <= stats.reach + n.radius
                && angle_delta(off.y.atan2(off.x), aim).abs() <= stats.arc
                && dist < best
            {
                best = dist;
                target = Some(i);
            }
        }

        if let Some(i) = target {
            let kind = self.nodes[i].kind;
            let amount = kind.yield_per_hit() * stats.gather;
            let res = kind.resource();
            let depleted;
            {
                let node = &mut self.nodes[i];
                node.hp -= stats.damage * 1.15 + 8.0;
                node.shake = 0.2;
                depleted = node.hp <= 0.0;
            }
            let npos = self.nodes[i].pos;
            self.particles.extend(burst(
                npos,
                with_alpha(res.color(), 0.9),
                7,
                120.0,
                3.0,
                220.0,
            ));
            player.add_resource(res, amount);
            progress.register_gather(res, amount);
            progress.register_melee_hit();
            self.floaters.push(Floater {
                pos: npos + vec2(0.0, -20.0),
                text: format!("+{:.0} {}", amount, res.label()),
                color: res.color(),
                life: 0.9,
                max_life: 0.9,
            });
            if depleted {
                self.floaters.push(Floater {
                    pos: npos + vec2(0.0, 8.0),
                    text: "RESPAWNED".to_string(),
                    color: Color::new(0.80, 0.96, 0.75, 1.0),
                    life: 1.2,
                    max_life: 1.2,
                });
                // Remember the spot as mined out so it never grows back.
                self.node_hp.insert(node_key(npos), 0.0);
                self.nodes.remove(i);
                self.spawn_replacement(kind, player.pos);
            }
        }
        true
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    pub fn draw_ground(&self, camera_target: Vec2) {
        let (min, max) = view_bounds(camera_target);

        // Biome paint: one small tile per patch of ground, so the meadow, the
        // desert, the rainforest and the savanna each read at a glance. The map
        // is endless, so tiles simply run past the edge of the screen.
        const TILE: f32 = 40.0;
        let x0 = (min.x / TILE).floor() * TILE;
        let y0 = (min.y / TILE).floor() * TILE;
        let x1 = max.x;
        let y1 = max.y;
        let mut ty = y0;
        while ty < y1 {
            let mut tx = x0;
            while tx < x1 {
                let base = biome_at(vec2(tx + TILE * 0.5, ty + TILE * 0.5)).ground();
                // Deterministic jitter so the paint is not a flat slab.
                let shade = ((tx * 0.137 + ty * 0.311).sin() * 0.5 + 0.5) * 0.12 + 0.94;
                draw_rectangle(
                    tx,
                    ty,
                    TILE + 1.0,
                    TILE + 1.0,
                    Color::new(base.r * shade, base.g * shade, base.b * shade, 1.0),
                );
                tx += TILE;
            }
            ty += TILE;
        }

        // Water sits over the ground but under the plants.
        let time = get_time() as f32;
        for w in self.water.iter() {
            if !on_screen(w.pos, min, max, w.radius + 70.0) {
                continue;
            }
            // Wet bank, then the water itself.
            draw_circle(
                w.pos.x,
                w.pos.y,
                w.radius + 14.0,
                Color::new(0.30, 0.28, 0.18, 0.45),
            );
            draw_circle(
                w.pos.x,
                w.pos.y,
                w.radius + 6.0,
                Color::new(0.52, 0.45, 0.27, 0.80),
            );
            draw_circle(w.pos.x, w.pos.y, w.radius, Color::new(0.08, 0.22, 0.28, 1.0));
            draw_circle(
                w.pos.x,
                w.pos.y,
                w.radius * 0.84,
                Color::new(0.11, 0.32, 0.39, 1.0),
            );
            draw_circle(
                w.pos.x - w.radius * 0.18,
                w.pos.y - w.radius * 0.18,
                w.radius * 0.52,
                Color::new(0.18, 0.44, 0.52, 0.55),
            );
            // Slow ripples drifting across the surface.
            for k in 0..3 {
                let phase = (time * 0.7 + w.seed + k as f32 * 1.9).sin() * 0.5 + 0.5;
                let rr = w.radius * (0.32 + 0.22 * k as f32 + phase * 0.06);
                draw_circle_lines(
                    w.pos.x,
                    w.pos.y,
                    rr,
                    1.6,
                    Color::new(0.62, 0.86, 0.90, 0.16),
                );
            }
            // Reeds along the bank.
            for k in 0..7 {
                let a = w.seed + k as f32 * 0.9;
                let rp = w.pos + vec2(a.cos(), a.sin()) * (w.radius - 2.0);
                draw_line(
                    rp.x,
                    rp.y,
                    rp.x + a.cos() * 3.0,
                    rp.y - 11.0,
                    1.8,
                    Color::new(0.30, 0.46, 0.26, 0.9),
                );
            }
        }

        for d in self.decor.iter() {
            if !on_screen(d.pos, min, max, 60.0) {
                continue;
            }
            match d.kind {
                DecorKind::Patch => {
                    let c = if d.tint > 0.5 {
                        Color::new(0.44, 0.42, 0.24, 0.10)
                    } else {
                        Color::new(0.20, 0.36, 0.20, 0.12)
                    };
                    draw_circle(d.pos.x, d.pos.y, d.size * 3.2, c);
                }
                DecorKind::Tuft => {
                    let c = if d.tint > 0.5 {
                        Color::new(0.24, 0.45, 0.22, 0.9)
                    } else {
                        Color::new(0.30, 0.52, 0.26, 0.9)
                    };
                    let h = d.size * 0.7;
                    draw_circle(
                        d.pos.x + 2.0,
                        d.pos.y + 1.0,
                        d.size * 0.38,
                        Color::new(0.08, 0.16, 0.08, 0.10),
                    );
                    for i in -2i32..=2 {
                        let bx = d.pos.x + i as f32 * d.size * 0.22;
                        let tall = h * (0.72 + (i.abs() as f32) * 0.10);
                        draw_line(
                            bx,
                            d.pos.y,
                            bx + i as f32 * 2.8,
                            d.pos.y - tall,
                            1.6,
                            c,
                        );
                        draw_line(
                            bx + i as f32 * 1.2,
                            d.pos.y - tall * 0.25,
                            bx + i as f32 * 3.0,
                            d.pos.y - tall * 0.85,
                            1.0,
                            Color::new(0.46, 0.62, 0.30, 0.55),
                        );
                    }
                }
                DecorKind::Flower => {
                    let petal = if d.tint > 0.6 {
                        Color::new(0.95, 0.85, 0.35, 1.0)
                    } else if d.tint > 0.3 {
                        Color::new(0.93, 0.62, 0.75, 1.0)
                    } else {
                        Color::new(0.85, 0.88, 0.95, 1.0)
                    };
                    draw_line(
                        d.pos.x,
                        d.pos.y + 1.0,
                        d.pos.x + 0.5,
                        d.pos.y + 6.0,
                        1.2,
                        Color::new(0.20, 0.38, 0.18, 0.85),
                    );
                    draw_line(
                        d.pos.x - 0.5,
                        d.pos.y + 5.0,
                        d.pos.x - 3.0,
                        d.pos.y + 3.0,
                        1.4,
                        Color::new(0.30, 0.52, 0.24, 0.8),
                    );
                    for i in 0..6 {
                        let a = i as f32 / 6.0 * TAU;
                        draw_circle(
                            d.pos.x + a.cos() * 3.0,
                            d.pos.y + a.sin() * 3.0,
                            2.0,
                            petal,
                        );
                    }
                    draw_circle(
                        d.pos.x - 0.8,
                        d.pos.y - 0.8,
                        1.0,
                        Color::new(1.0, 0.96, 0.74, 0.85),
                    );
                    draw_circle(d.pos.x, d.pos.y, 1.7, Color::new(0.75, 0.55, 0.15, 1.0));
                }
                DecorKind::Stone => {
                    draw_circle(
                        d.pos.x + 3.0,
                        d.pos.y + 2.0,
                        d.size * 0.32,
                        Color::new(0.08, 0.16, 0.08, 0.12),
                    );
                    draw_circle(
                        d.pos.x,
                        d.pos.y,
                        d.size * 0.32,
                        Color::new(0.56, 0.58, 0.54, 1.0),
                    );
                    draw_circle(
                        d.pos.x - d.size * 0.09,
                        d.pos.y - d.size * 0.09,
                        d.size * 0.16,
                        Color::new(0.74, 0.76, 0.72, 0.55),
                    );
                    draw_circle_lines(
                        d.pos.x,
                        d.pos.y,
                        d.size * 0.32,
                        1.5,
                        Color::new(0.30, 0.33, 0.30, 1.0),
                    );
                }
            }
        }

    }

    pub fn draw_nodes(&self, camera_target: Vec2) {
        let (min, max) = view_bounds(camera_target);
        let time = get_time() as f32;

        for n in self.nodes.iter() {
            if !on_screen(n.pos, min, max, 90.0) {
                continue;
            }
            let shake = if n.shake > 0.0 {
                vec2(
                    gen_range(-1.0, 1.0) * n.shake * 16.0,
                    gen_range(-1.0, 1.0) * n.shake * 16.0,
                )
            } else {
                Vec2::ZERO
            };
            let p = n.pos + shake;

            let bob = (time * 1.6 + n.seed).sin() * 1.2;

            match n.kind {
                NodeKind::Berry => {
                    let p = p + vec2(0.0, bob);
                    shadow(p + vec2(2.0, 5.0), 26.0);
                    for (ox, oy, r) in [
                        (-12.0, 2.0, 14.0),
                        (12.0, 3.0, 13.0),
                        (0.0, -8.0, 16.0),
                    ] {
                        outlined_circle(p + vec2(ox, oy), r, Color::new(0.20, 0.44, 0.22, 1.0));
                    }
                    for (ox, oy) in [
                        (-10.0, -4.0),
                        (2.0, -10.0),
                        (13.0, 0.0),
                        (-2.0, 4.0),
                        (7.0, 7.0),
                    ] {
                        draw_circle(p.x + ox, p.y + oy, 3.2, Color::new(0.90, 0.24, 0.30, 1.0));
                    }
                }
                NodeKind::Tree => {
                    draw_circle(
                        p.x + 16.0,
                        p.y + 20.0,
                        58.0,
                        Color::new(0.02, 0.06, 0.03, 0.10),
                    );
                    shadow(p + vec2(5.0, 6.0), 40.0);
                    let sway = bob * 0.5;
                    draw_rectangle(p.x - 7.0, p.y + 4.0, 14.0, 28.0, OUTLINE);
                    draw_rectangle(
                        p.x - 5.0,
                        p.y + 4.0,
                        10.0,
                        27.0,
                        Color::new(0.40, 0.27, 0.15, 1.0),
                    );
                    for (ox, oy, r, shade) in [
                        (-18.0, -12.0, 26.0, 0.16),
                        (18.0, -10.0, 25.0, 0.20),
                        (0.0, -30.0, 30.0, 0.26),
                        (0.0, -8.0, 30.0, 0.22),
                    ] {
                        outlined_circle(
                            p + vec2(ox + sway, oy),
                            r,
                            Color::new(shade, shade + 0.24, shade + 0.02, 1.0),
                        );
                    }
                    draw_circle(
                        p.x - 10.0 + sway,
                        p.y - 32.0,
                        9.0,
                        Color::new(0.58, 0.82, 0.40, 0.38),
                    );
                }
                NodeKind::Rock => {
                    shadow(p + vec2(4.0, 5.0), 25.0);
                    outlined_poly(p, 6, 21.0, 0.35 + n.seed, Color::new(0.56, 0.59, 0.60, 1.0));
                    draw_poly(
                        p.x - 2.0,
                        p.y - 5.0,
                        6,
                        12.0,
                        0.55 + n.seed,
                        Color::new(0.73, 0.75, 0.74, 1.0),
                    );
                    draw_circle(
                        p.x - 7.0,
                        p.y - 8.0,
                        5.5,
                        Color::new(0.82, 0.84, 0.82, 0.30),
                    );
                    draw_circle(
                        p.x + 9.0,
                        p.y + 7.0,
                        5.0,
                        Color::new(0.34, 0.47, 0.27, 0.34),
                    );
                    draw_line(
                        p.x - 6.0,
                        p.y + 4.0,
                        p.x + 4.0,
                        p.y + 12.0,
                        2.0,
                        Color::new(0.33, 0.36, 0.37, 0.9),
                    );
                    draw_line(
                        p.x + 2.0,
                        p.y - 12.0,
                        p.x + 7.0,
                        p.y - 1.0,
                        1.5,
                        Color::new(0.38, 0.41, 0.42, 0.75),
                    );
                }
                NodeKind::Iron => {
                    shadow(p + vec2(4.0, 5.0), 26.0);
                    let glow = 0.55 + 0.45 * (time * 2.0 + n.seed).sin();
                    draw_circle(
                        p.x,
                        p.y,
                        48.0,
                        Color::new(0.30, 0.86, 1.0, 0.035 + glow * 0.045),
                    );
                    outlined_poly(p, 7, 22.0, 0.2 + n.seed, Color::new(0.32, 0.36, 0.44, 1.0));
                    draw_circle(
                        p.x - 7.0,
                        p.y - 7.0,
                        7.0,
                        Color::new(0.48, 0.54, 0.62, 0.30),
                    );
                    for (ox, oy, size, rot) in [
                        (-7.0, -4.0, 9.0, -1.2),
                        (5.0, -8.0, 7.0, -0.4),
                        (3.0, 6.0, 6.0, 1.4),
                    ] {
                        let c = Color::new(0.42, 0.86, 0.92, 1.0);
                        outlined_poly(p + vec2(ox, oy), 3, size, rot, c);
                        draw_circle(
                            p.x + ox - 1.0,
                            p.y + oy - 1.0,
                            2.0,
                            Color::new(0.85, 1.0, 1.0, glow),
                        );
                    }
                }
            }

            if n.hp < n.max_hp {
                let top = match n.kind {
                    NodeKind::Tree => 158.0,
                    NodeKind::Berry => 74.0,
                    NodeKind::Rock => 54.0,
                    NodeKind::Iron => 58.0,
                };
                draw_bar(
                    p + vec2(0.0, -top),
                    46.0,
                    6.0,
                    n.hp / n.max_hp,
                    n.kind.resource().color(),
                );
            }
        }
    }

    /// Where the range sign stands: a step east of the middle target.
    pub fn sign_pos(&self) -> Vec2 {
        let mid = self
            .targets
            .get(1)
            .map(|t| t.pos)
            .unwrap_or_else(home);
        mid + vec2(130.0, 0.0)
    }

    pub fn draw_targets(&self, camera_target: Vec2) {
        let (min, max) = view_bounds(camera_target);
        for t in self.targets.iter() {
            if !on_screen(t.pos, min, max, 80.0) {
                continue;
            }
            let p = t.pos;
            shadow(p + vec2(3.0, 8.0), 24.0);
            // Timber post holding the straw face up off the grass.
            draw_rectangle(p.x - 4.0, p.y + 4.0, 8.0, 28.0, OUTLINE);
            draw_rectangle(
                p.x - 2.0,
                p.y + 4.0,
                4.0,
                27.0,
                Color::new(0.42, 0.30, 0.17, 1.0),
            );
            // Straw face: cream and red rings with a pale centre dot.
            outlined_circle(p, 20.0, Color::new(0.86, 0.78, 0.58, 1.0));
            draw_circle(p.x, p.y, 15.0, Color::new(0.78, 0.20, 0.22, 1.0));
            draw_circle(p.x, p.y, 10.0, Color::new(0.92, 0.86, 0.66, 1.0));
            draw_circle(p.x, p.y, 5.0, Color::new(0.78, 0.20, 0.22, 1.0));
            draw_circle(p.x, p.y, 1.6, Color::new(1.0, 0.96, 0.86, 1.0));
            if t.flash > 0.0 {
                let a = (t.flash / 0.25).clamp(0.0, 1.0);
                draw_circle_lines(p.x, p.y, 23.0, 2.0, Color::new(1.0, 0.9, 0.5, a));
            }
        }

        // Scoreboard on the back edge of the range.
        let sign = self.sign_pos();
        if on_screen(sign, min, max, 160.0) {
            shadow(sign + vec2(4.0, 10.0), 70.0);
            for dx in [-84.0, 84.0] {
                draw_rectangle(sign.x + dx - 4.0, sign.y - 4.0, 8.0, 60.0, OUTLINE);
                draw_rectangle(
                    sign.x + dx - 2.0,
                    sign.y - 4.0,
                    4.0,
                    58.0,
                    Color::new(0.42, 0.30, 0.17, 1.0),
                );
            }
            draw_rectangle(sign.x - 104.0, sign.y - 58.0, 208.0, 84.0, OUTLINE);
            draw_rectangle(
                sign.x - 100.0,
                sign.y - 54.0,
                200.0,
                76.0,
                Color::new(0.31, 0.23, 0.15, 1.0),
            );
            draw_rectangle(
                sign.x - 100.0,
                sign.y - 54.0,
                200.0,
                3.0,
                Color::new(0.46, 0.34, 0.21, 1.0),
            );
            draw_text_center(
                "PRACTICE RANGE",
                sign.x,
                sign.y - 44.0,
                9.0,
                Color::new(0.95, 0.88, 0.62, 1.0),
                Some(OUTLINE),
            );
            draw_text_center(
                &format!("HITS  {}", self.practice_hits),
                sign.x,
                sign.y - 22.0,
                10.0,
                Color::new(0.98, 0.96, 0.9, 1.0),
                Some(OUTLINE),
            );
            draw_text_center(
                &format!("BULLSEYES  {}", self.practice_bullseyes),
                sign.x,
                sign.y,
                10.0,
                Color::new(1.0, 0.85, 0.4, 1.0),
                Some(OUTLINE),
            );
            draw_text_center(
                "T  RESET",
                sign.x,
                sign.y + 22.0,
                8.0,
                Color::new(0.74, 0.79, 0.83, 1.0),
                Some(OUTLINE),
            );
        }
    }

    pub fn draw_enemies(&self, camera_target: Vec2) {
        let (min, max) = view_bounds(camera_target);
        for e in self.enemies.iter() {
            if !on_screen(e.pos, min, max, 100.0) {
                continue;
            }
            let dir = vec2(e.facing.cos(), e.facing.sin());
            let side = vec2(-dir.y, dir.x);
            shadow(e.pos, e.kind.radius() + 3.0);

            match e.kind {
                EnemyKind::Wolf => {
                    let gray = Color::new(0.55, 0.56, 0.62, 1.0);
                    let dark = Color::new(0.40, 0.41, 0.48, 1.0);
                    draw_line(
                        (e.pos - dir * 20.0).x,
                        (e.pos - dir * 20.0).y,
                        (e.pos - dir * 30.0).x,
                        (e.pos - dir * 30.0).y,
                        5.0,
                        OUTLINE,
                    );
                    draw_line(
                        (e.pos - dir * 19.0).x,
                        (e.pos - dir * 19.0).y,
                        (e.pos - dir * 29.0).x,
                        (e.pos - dir * 29.0).y,
                        3.0,
                        dark,
                    );
                    for s in [-1.0, 1.0] {
                        let leg = e.pos + side * (9.0 * s) + dir * 4.0;
                        draw_rot_rect_outlined(leg, 7.0, 15.0, e.facing, dark);
                        draw_circle(
                            leg.x + dir.x * 5.0,
                            leg.y + dir.y * 5.0,
                            3.4,
                            Color::new(0.33, 0.34, 0.40, 1.0),
                        );
                    }
                    draw_rot_rect_outlined(e.pos, 40.0, 24.0, e.facing, gray);
                    draw_circle(
                        e.pos.x - dir.x * 3.0 - side.x * 2.0,
                        e.pos.y - dir.y * 3.0 - side.y * 2.0,
                        9.0,
                        Color::new(0.68, 0.69, 0.74, 0.34),
                    );
                    for s in [-1.0, 0.0, 1.0] {
                        draw_line(
                            e.pos.x - side.x * 5.0 + dir.x * s * 7.0,
                            e.pos.y - side.y * 5.0 + dir.y * s * 7.0,
                            e.pos.x - side.x * 9.0 + dir.x * s * 7.0,
                            e.pos.y - side.y * 9.0 + dir.y * s * 7.0,
                            1.5,
                            Color::new(0.35, 0.36, 0.42, 0.75),
                        );
                    }
                    let head = e.pos + dir * 19.0;
                    draw_triangle(
                        head + side * 7.0 + dir * 2.0,
                        head + side * 2.0 + dir * 2.0,
                        head + side * 6.0 - dir * 4.0,
                        OUTLINE,
                    );
                    draw_triangle(
                        head - side * 7.0 + dir * 2.0,
                        head - side * 2.0 + dir * 2.0,
                        head - side * 6.0 - dir * 4.0,
                        OUTLINE,
                    );
                    outlined_circle(head, 11.0, gray);
                    draw_triangle(
                        head + dir * 7.0 + side * 5.0,
                        head + dir * 7.0 - side * 5.0,
                        head + dir * 19.0,
                        dark,
                    );
                    for s in [-1.0, 1.0] {
                        let eye = head + side * (4.5 * s) + dir * 4.0;
                        draw_circle(eye.x, eye.y, 4.2, Color::new(1.0, 0.74, 0.22, 0.16));
                        draw_circle(eye.x, eye.y, 2.1, Color::new(1.0, 0.88, 0.35, 1.0));
                        draw_circle(eye.x - 0.5, eye.y - 0.6, 0.7, WHITE);
                    }
                }
                EnemyKind::Bear => {
                    let brown = Color::new(0.46, 0.31, 0.20, 1.0);
                    let dark = Color::new(0.34, 0.22, 0.14, 1.0);
                    for s in [-1.0, 1.0] {
                        let leg = e.pos + side * (13.0 * s) - dir * 4.0;
                        draw_rot_rect_outlined(leg, 10.0, 18.0, e.facing, dark);
                        for c in [-1.0, 1.0] {
                            let claw = leg + dir * 9.0 + side * (2.2 * c);
                            draw_line(
                                claw.x,
                                claw.y,
                                claw.x + dir.x * 5.0,
                                claw.y + dir.y * 5.0,
                                1.6,
                                Color::new(0.82, 0.78, 0.66, 0.9),
                            );
                        }
                    }
                    outlined_circle(e.pos, 24.0, brown);
                    outlined_circle(e.pos - dir * 4.0, 15.0, Color::new(0.53, 0.37, 0.24, 1.0));
                    draw_circle(
                        e.pos.x - dir.x * 6.0 - side.x * 5.0,
                        e.pos.y - dir.y * 6.0 - side.y * 5.0,
                        8.0,
                        Color::new(0.62, 0.45, 0.29, 0.38),
                    );
                    for s in [-1.0, 0.0, 1.0] {
                        draw_line(
                            e.pos.x - side.x * 7.0 + dir.x * s * 10.0,
                            e.pos.y - side.y * 7.0 + dir.y * s * 10.0,
                            e.pos.x - side.x * 13.0 + dir.x * s * 10.0,
                            e.pos.y - side.y * 13.0 + dir.y * s * 10.0,
                            2.0,
                            Color::new(0.32, 0.21, 0.14, 0.75),
                        );
                    }
                    let head = e.pos + dir * 21.0;
                    for s in [-1.0, 1.0] {
                        outlined_circle(
                            head + side * (10.0 * s) - dir * 3.0,
                            6.0,
                            dark,
                        );
                    }
                    outlined_circle(head, 15.0, brown);
                    outlined_circle(head + dir * 7.0, 6.5, Color::new(0.62, 0.47, 0.33, 1.0));
                    draw_circle(
                        head.x + dir.x * 11.0,
                        head.y + dir.y * 11.0,
                        3.0,
                        OUTLINE,
                    );
                    for s in [-1.0, 1.0] {
                        let eye = head + side * (6.0 * s) + dir * 5.0;
                        draw_circle(eye.x, eye.y, 4.4, Color::new(1.0, 0.70, 0.22, 0.16));
                        draw_circle(eye.x, eye.y, 2.2, Color::new(1.0, 0.84, 0.35, 1.0));
                        draw_circle(eye.x - 0.5, eye.y - 0.6, 0.7, WHITE);
                    }
                    draw_circle(
                        head.x + dir.x * 8.0 - side.x * 2.0,
                        head.y + dir.y * 8.0 - side.y * 2.0,
                        3.0,
                        Color::new(0.20, 0.14, 0.11, 0.85),
                    );
                }
                EnemyKind::Raider => {
                    let cloth = Color::new(0.55, 0.20, 0.24, 1.0);
                    for s in [-1.0, 1.0] {
                        let leg = e.pos + side * (7.0 * s) + vec2(0.0, 9.0);
                        draw_rot_rect_outlined(leg, 7.0, 12.0, 0.0, Color::new(0.22, 0.19, 0.22, 1.0));
                    }
                    outlined_circle(e.pos, 16.0, cloth);
                    draw_rot_rect_outlined(
                        e.pos + side * 15.0 + dir * 4.0,
                        9.0,
                        25.0,
                        e.facing + 1.1,
                        Color::new(0.30, 0.27, 0.30, 1.0),
                    );
                    let head = e.pos + dir * 6.0;
                    draw_triangle(
                        head + side * 12.0 - dir * 2.0,
                        head - side * 12.0 - dir * 2.0,
                        head - dir * 13.0,
                        OUTLINE,
                    );
                    draw_triangle(
                        head + side * 9.5 - dir * 2.0,
                        head - side * 9.5 - dir * 2.0,
                        head - dir * 10.0,
                        Color::new(0.32, 0.11, 0.14, 1.0),
                    );
                    outlined_circle(head, 10.0, Color::new(0.88, 0.72, 0.56, 1.0));
                    draw_rot_rect_outlined(
                        head + dir * 3.0,
                        16.0,
                        7.0,
                        e.facing,
                        Color::new(0.20, 0.20, 0.24, 1.0),
                    );
                    for s in [-1.0, 1.0] {
                        let eye = head + side * (3.6 * s) + dir * 5.0;
                        draw_circle(eye.x, eye.y, 3.8, Color::new(1.0, 0.42, 0.20, 0.18));
                        draw_circle(eye.x, eye.y, 1.9, Color::new(1.0, 0.55, 0.30, 1.0));
                    }
                    draw_line(
                        e.pos.x + side.x * 12.0,
                        e.pos.y + side.y * 12.0,
                        e.pos.x - side.x * 12.0,
                        e.pos.y - side.y * 12.0,
                        3.0,
                        Color::new(0.24, 0.16, 0.16, 0.9),
                    );
                    let hand = e.pos + side * 16.0 + dir * 12.0;
                    draw_rot_rect_outlined(hand, 6.0, 30.0, e.facing - 0.8, Color::new(0.50, 0.34, 0.19, 1.0));
                    let spear_dir = vec2((e.facing - 0.8).cos(), (e.facing - 0.8).sin());
                    let spear_side = vec2(-spear_dir.y, spear_dir.x);
                    let base = hand + spear_dir * 9.0;
                    draw_triangle(
                        base + spear_side * 5.0,
                        base - spear_side * 5.0,
                        hand + spear_dir * 25.0,
                        OUTLINE,
                    );
                    draw_triangle(
                        base + spear_side * 3.0,
                        base - spear_side * 3.0,
                        hand + spear_dir * 22.0,
                        Color::new(0.78, 0.86, 0.90, 1.0),
                    );
                }
                EnemyKind::Jackal => {
                    draw_quadruped(
                        e,
                        dir,
                        side,
                        36.0,
                        18.0,
                        9.0,
                        Color::new(0.70, 0.55, 0.32, 1.0),
                        Color::new(0.44, 0.32, 0.19, 1.0),
                        Color::new(0.60, 0.47, 0.27, 1.0),
                        false,
                        false,
                        false,
                    );
                }
                EnemyKind::Jaguar => {
                    draw_quadruped(
                        e,
                        dir,
                        side,
                        44.0,
                        26.0,
                        11.0,
                        Color::new(0.88, 0.64, 0.24, 1.0),
                        Color::new(0.26, 0.19, 0.12, 1.0),
                        Color::new(0.96, 0.82, 0.44, 1.0),
                        false,
                        true,
                        true,
                    );
                }
                EnemyKind::Lion => {
                    draw_quadruped(
                        e,
                        dir,
                        side,
                        48.0,
                        28.0,
                        12.0,
                        Color::new(0.82, 0.64, 0.32, 1.0),
                        Color::new(0.42, 0.28, 0.14, 1.0),
                        Color::new(0.46, 0.28, 0.14, 1.0),
                        true,
                        false,
                        false,
                    );
                }
                EnemyKind::Buffalo => {
                    draw_quadruped(
                        e,
                        dir,
                        side,
                        52.0,
                        34.0,
                        13.0,
                        Color::new(0.28, 0.27, 0.29, 1.0),
                        Color::new(0.16, 0.15, 0.17, 1.0),
                        Color::new(0.40, 0.38, 0.36, 1.0),
                        false,
                        true,
                        false,
                    );
                }
                EnemyKind::Scorpion => {
                    let shell = Color::new(0.34, 0.25, 0.18, 1.0);
                    let plate = Color::new(0.52, 0.39, 0.23, 1.0);
                    // Curled tail with a bright sting.
                    for k in 0..4 {
                        let p = e.pos - dir * (11.0 + k as f32 * 6.0)
                            + side * ((k as f32 * 1.5 + 0.6).sin() * 5.0);
                        draw_circle(p.x, p.y, 4.6 - k as f32 * 0.5, plate);
                    }
                    let sting = e.pos - dir * 35.0 + side * 2.0;
                    draw_circle(sting.x, sting.y, 4.2, Color::new(0.86, 0.32, 0.22, 1.0));
                    // Legs.
                    for s in [-1.0, 1.0] {
                        for k in 0..3 {
                            let base = e.pos + dir * (6.0 - k as f32 * 7.0) + side * (7.0 * s);
                            draw_line(
                                base.x,
                                base.y,
                                base.x + side.x * 9.0 * s + dir.x * 3.0,
                                base.y + side.y * 9.0 * s + dir.y * 3.0,
                                2.0,
                                shell,
                            );
                        }
                    }
                    // Pincers held out front.
                    for s in [-1.0, 1.0] {
                        let arm = e.pos + dir * 8.0 + side * (9.0 * s);
                        draw_line(
                            e.pos.x + side.x * 5.0 * s,
                            e.pos.y + side.y * 5.0 * s,
                            arm.x,
                            arm.y,
                            3.0,
                            shell,
                        );
                        let claw = arm + dir * 8.0 + side * (3.0 * s);
                        outlined_circle(claw, 5.6, plate);
                        draw_line(
                            claw.x - side.x * 4.0 * s,
                            claw.y - side.y * 4.0 * s,
                            claw.x + dir.x * 8.0,
                            claw.y + dir.y * 8.0,
                            2.0,
                            shell,
                        );
                    }
                    outlined_circle(e.pos, 11.0, shell);
                    draw_circle(
                        e.pos.x - dir.x * 2.0,
                        e.pos.y - dir.y * 2.0,
                        6.0,
                        plate,
                    );
                    for s in [-1.0, 1.0] {
                        let eye = e.pos + dir * 8.0 + side * (4.0 * s);
                        draw_circle(eye.x, eye.y, 1.8, Color::new(1.0, 0.42, 0.24, 1.0));
                    }
                }
                EnemyKind::Boa => {
                    let coil = Color::new(0.30, 0.45, 0.22, 1.0);
                    let dark = Color::new(0.19, 0.29, 0.14, 1.0);
                    let band = Color::new(0.74, 0.66, 0.30, 1.0);
                    for k in 0..6 {
                        let t = k as f32;
                        let p = e.pos - dir * (t * 11.0)
                            + side * ((t * 1.1 + e.facing).sin() * 7.0);
                        outlined_circle(p, 9.5 - t * 0.7, if k % 2 == 0 { coil } else { dark });
                        if k % 3 == 1 {
                            draw_circle(p.x, p.y, 3.6, with_alpha(band, 0.8));
                        }
                    }
                    let head = e.pos + dir * 10.0;
                    outlined_circle(head, 10.0, coil);
                    draw_circle(
                        head.x + dir.x * 6.0,
                        head.y + dir.y * 6.0,
                        4.6,
                        dark,
                    );
                    for s in [-1.0, 1.0] {
                        let eye = head + side * (4.6 * s) + dir * 2.0;
                        draw_circle(eye.x, eye.y, 2.4, Color::new(1.0, 0.82, 0.26, 1.0));
                        draw_circle(eye.x, eye.y, 0.9, OUTLINE);
                    }
                    let tongue = head + dir * 12.0;
                    draw_line(
                        head.x + dir.x * 8.0,
                        head.y + dir.y * 8.0,
                        tongue.x,
                        tongue.y,
                        1.4,
                        Color::new(0.92, 0.28, 0.32, 1.0),
                    );
                }
            }

            if e.hit_flash > 0.0 {
                let a = (e.hit_flash / 0.18).clamp(0.0, 1.0) * 0.55;
                draw_circle(
                    e.pos.x,
                    e.pos.y,
                    e.kind.radius() + 5.0,
                    Color::new(1.0, 0.9, 0.9, a),
                );
            }
            if e.bleed > 0.0 {
                draw_circle(
                    e.pos.x + 7.0,
                    e.pos.y + 15.0,
                    3.0,
                    Color::new(0.72, 0.12, 0.14, 0.85),
                );
            }
            if e.hp < e.max_hp {
                draw_bar(
                    e.pos + vec2(0.0, -e.kind.radius() - 24.0),
                    44.0,
                    6.0,
                    e.hp / e.max_hp,
                    Color::new(0.94, 0.3, 0.30, 1.0),
                );
            }
        }
    }

    pub fn draw_particles(&self) {
        for p in self.particles.iter() {
            let a = (p.life / p.max_life).clamp(0.0, 1.0);
            let size = p.size * (0.35 + a * 0.65);
            draw_circle(p.pos.x, p.pos.y, size, with_alpha(p.color, p.color.a * a));
        }
    }

    pub fn draw_arrows(&self) {
        for a in self.arrows.iter() {
            let dir = a.vel.normalize_or_zero();
            let side = vec2(-dir.y, dir.x);
            let back = a.pos - dir * 18.0;
            draw_line(back.x, back.y, a.pos.x, a.pos.y, 4.0, OUTLINE);
            draw_line(
                back.x,
                back.y,
                a.pos.x,
                a.pos.y,
                2.0,
                Color::new(0.86, 0.74, 0.44, 1.0),
            );
            draw_triangle(
                a.pos + dir * 5.0,
                a.pos + side * 4.0 - dir * 5.0,
                a.pos - side * 4.0 - dir * 5.0,
                Color::new(0.62, 0.94, 1.0, 1.0),
            );
        }
    }

    pub fn draw_bullets(&self) {
        for b in self.bullets.iter() {
            let dir = b.vel.normalize_or_zero();
            let back = b.pos - dir * (12.0 + b.tracer * 14.0);
            draw_line(back.x, back.y, b.pos.x, b.pos.y, 4.0, OUTLINE);
            draw_line(
                back.x,
                back.y,
                b.pos.x,
                b.pos.y,
                2.0,
                Color::new(1.0, 0.76, 0.28, 1.0),
            );
            draw_circle(b.pos.x, b.pos.y, 2.4, Color::new(1.0, 0.98, 0.82, 1.0));
        }
    }

    pub fn draw_enemy_shots(&self) {
        for s in self.enemy_shots.iter() {
            let dir = s.vel.normalize_or_zero();
            let back = s.pos - dir * 24.0;
            draw_line(back.x, back.y, s.pos.x, s.pos.y, 5.0, OUTLINE);
            draw_line(
                back.x,
                back.y,
                s.pos.x,
                s.pos.y,
                2.5,
                Color::new(0.62, 0.48, 0.30, 1.0),
            );
            draw_triangle(
                s.pos + dir * 6.0,
                s.pos + vec2(-dir.y, dir.x) * 4.0 - dir * 3.0,
                s.pos - vec2(-dir.y, dir.x) * 4.0 - dir * 3.0,
                Color::new(0.86, 0.90, 0.94, 1.0),
            );
        }
    }

    pub fn draw_floaters(&self) {
        for f in self.floaters.iter() {
            let a = (f.life / f.max_life).clamp(0.0, 1.0);
            draw_text_center(
                &f.text,
                f.pos.x,
                f.pos.y,
                20.0,
                with_alpha(f.color, a),
                Some(with_alpha(OUTLINE, a * 0.9)),
            );
        }
    }
}
