//! The 3200x3200 world: procedural terrain, resource nodes, hostile wildlife,
//! collision helpers and combat resolution.

use macroquad::prelude::*;
use macroquad::rand::gen_range;
use std::f32::consts::{PI, TAU};

use crate::building::BuildingManager;
use crate::player::{Player, RIFLE_BASE_DAMAGE, SHOTGUN_PELLET_DAMAGE};
use crate::progression::Progression;

pub const WORLD_SIZE: f32 = 3200.0;
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
}

impl EnemyKind {
    pub fn max_hp(self) -> f32 {
        match self {
            EnemyKind::Wolf => 80.0,
            EnemyKind::Bear => 210.0,
            EnemyKind::Raider => 140.0,
        }
    }

    pub fn speed(self) -> f32 {
        match self {
            EnemyKind::Wolf => 132.0,
            EnemyKind::Bear => 92.0,
            EnemyKind::Raider => 116.0,
        }
    }

    pub fn damage(self) -> f32 {
        match self {
            EnemyKind::Wolf => 7.0,
            EnemyKind::Bear => 15.0,
            EnemyKind::Raider => 11.0,
        }
    }

    /// How close the survivor can get before this creature notices them. Sight
    /// still has to be clear of trees and boulders.
    pub fn sight_range(self) -> f32 {
        match self {
            EnemyKind::Wolf => 470.0,
            EnemyKind::Bear => 340.0,
            EnemyKind::Raider => 540.0,
        }
    }

    pub fn radius(self) -> f32 {
        match self {
            EnemyKind::Wolf => 16.0,
            EnemyKind::Bear => 23.0,
            EnemyKind::Raider => 17.0,
        }
    }

    pub fn loot(self) -> (Resource, f32) {
        match self {
            EnemyKind::Wolf => (Resource::Food, 5.0),
            EnemyKind::Bear => (Resource::Food, 12.0),
            EnemyKind::Raider => (Resource::Metal, 6.0),
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
}

/// How long an animal keeps hunting after it loses sight of the survivor
/// before it gives up and goes back to grazing.
const LOSE_INTEREST_TIME: f32 = 5.0;

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
    ) -> Self {
        let mut world = Self::new();
        world.nodes = nodes;
        world.enemies = enemies;
        world.arrays_clear();
        world.elapsed = elapsed;
        world.hp_level = hp_level.max(1);
        world.spawn_timer = 4.0;
        world.footstep_timer = 0.0;
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

    pub fn float_text(&mut self, pos: Vec2, text: &str, color: Color) {
        self.floaters.push(Floater {
            pos,
            text: text.to_string(),
            color,
            life: 1.0,
            max_life: 1.0,
        });
    }

    fn generate(&mut self) {
        self.decor.clear();
        for _ in 0..1500 {
            let pos = vec2(
                gen_range(20.0, WORLD_SIZE - 20.0),
                gen_range(20.0, WORLD_SIZE - 20.0),
            );
            let roll: f32 = gen_range(0.0, 1.0);
            let kind = if roll < 0.56 {
                DecorKind::Tuft
            } else if roll < 0.72 {
                DecorKind::Patch
            } else if roll < 0.89 {
                DecorKind::Flower
            } else {
                DecorKind::Stone
            };
            self.decor.push(Decor {
                pos,
                size: gen_range(6.0, 20.0),
                kind,
                tint: gen_range(0.0, 1.0),
            });
        }

        self.nodes.clear();
        let center = vec2(WORLD_SIZE * 0.5, WORLD_SIZE * 0.5);
        let mut attempts = 0;
        while self.nodes.len() < 185 && attempts < 9000 {
            attempts += 1;
            let pos = vec2(
                gen_range(110.0, WORLD_SIZE - 110.0),
                gen_range(110.0, WORLD_SIZE - 110.0),
            );
            if pos.distance(center) < 150.0 {
                continue;
            }
            if self
                .nodes
                .iter()
                .any(|n| n.pos.distance(pos) < 78.0)
            {
                continue;
            }
            let roll: f32 = gen_range(0.0, 1.0);
            let kind = if roll < 0.24 {
                NodeKind::Berry
            } else if roll < 0.58 {
                NodeKind::Tree
            } else if roll < 0.84 {
                NodeKind::Rock
            } else {
                NodeKind::Iron
            };
            self.push_node(pos, kind);
        }

        // A friendly starter cluster so the first minute is never empty.
        let starters = [
            (vec2(-150.0, -70.0), NodeKind::Tree),
            (vec2(130.0, 55.0), NodeKind::Berry),
            (vec2(-60.0, 160.0), NodeKind::Rock),
            (vec2(200.0, -120.0), NodeKind::Tree),
            (vec2(-220.0, 90.0), NodeKind::Iron),
            (vec2(70.0, -200.0), NodeKind::Berry),
            (vec2(240.0, 190.0), NodeKind::Rock),
        ];
        for (off, kind) in starters {
            let pos = center + off;
            if !self.nodes.iter().any(|n| n.pos.distance(pos) < 64.0) {
                self.push_node(pos, kind);
            }
        }
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
        let mut kind = EnemyKind::Wolf;
        let roll: f32 = gen_range(0.0, 1.0);
        if self.elapsed > 80.0 && roll > 0.7 {
            kind = EnemyKind::Raider;
        } else if self.elapsed > 40.0 && roll > 0.46 {
            kind = EnemyKind::Bear;
        }
        let angle: f32 = gen_range(0.0, TAU);
        let dist: f32 = gen_range(640.0, 960.0);
        let mut pos = player_pos + vec2(angle.cos() * dist, angle.sin() * dist);
        pos.x = pos.x.clamp(70.0, WORLD_SIZE - 70.0);
        pos.y = pos.y.clamp(70.0, WORLD_SIZE - 70.0);
        if pos.distance(player_pos) < 430.0 {
            let to_center =
                (vec2(WORLD_SIZE * 0.5, WORLD_SIZE * 0.5) - player_pos).normalize_or_zero();
            pos = player_pos + to_center * 640.0;
            pos.x = pos.x.clamp(70.0, WORLD_SIZE - 70.0);
            pos.y = pos.y.clamp(70.0, WORLD_SIZE - 70.0);
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

        for node in self.nodes.iter_mut() {
            if node.shake > 0.0 {
                node.shake = (node.shake - dt).max(0.0);
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
            // At touching distance there is nothing left to hide behind.
            let pressed = dist_player < er + player.radius + 60.0;
            if sees_player || pressed {
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

            let attack_player = hunt && dist_player < er + player.radius + 12.0;
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
                // Unaware animals graze: a lazy drift, no interest in you.
                let t = self.elapsed * 0.6 + i as f32 * 2.1;
                desired = vec2(t.sin(), (t * 1.3).cos()) * 0.5;
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
                np.x = np.x.clamp(50.0, WORLD_SIZE - 50.0);
                np.y = np.y.clamp(50.0, WORLD_SIZE - 50.0);
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
                self.enemies[i].attack_cd = 0.95;
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
                if e.throw_cd <= 0.0 && dist_player < 430.0 && dist_player > 70.0 && !path_blocked {
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
        self.arrows.retain(|a| a.life > 0.0);
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
    /// elsewhere in the world, keeping the map stocked.
    fn spawn_replacement(&mut self, kind: NodeKind, player_pos: Vec2) {
        for _ in 0..160 {
            let pos = vec2(
                gen_range(110.0, WORLD_SIZE - 110.0),
                gen_range(110.0, WORLD_SIZE - 110.0),
            );
            if pos.distance(player_pos) < 380.0 {
                continue;
            }
            if self.nodes.iter().any(|n| n.pos.distance(pos) < 78.0) {
                continue;
            }
            self.push_node(pos, kind);
            return;
        }
        let mut pos = player_pos + vec2(gen_range(-320.0, 320.0), gen_range(-320.0, 320.0));
        pos.x = pos.x.clamp(110.0, WORLD_SIZE - 110.0);
        pos.y = pos.y.clamp(110.0, WORLD_SIZE - 110.0);
        self.push_node(pos, kind);
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
        draw_rectangle(
            -3000.0,
            -3000.0,
            WORLD_SIZE + 6000.0,
            WORLD_SIZE + 6000.0,
            Color::new(0.07, 0.13, 0.10, 1.0),
        );
        draw_rectangle(0.0, 0.0, WORLD_SIZE, WORLD_SIZE, Color::new(0.20, 0.33, 0.19, 1.0));

        let (min, max) = view_bounds(camera_target);

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

        // Boundary: thick dark edge with posts.
        draw_rectangle_lines(0.0, 0.0, WORLD_SIZE, WORLD_SIZE, 12.0, OUTLINE);
        draw_rectangle_lines(
            8.0,
            8.0,
            WORLD_SIZE - 16.0,
            WORLD_SIZE - 16.0,
            2.0,
            Color::new(0.92, 0.82, 0.45, 0.35),
        );
        let mut t = 60.0;
        while t < WORLD_SIZE {
            for (px, py) in [
                (t, 6.0),
                (t, WORLD_SIZE - 6.0),
                (6.0, t),
                (WORLD_SIZE - 6.0, t),
            ] {
                outlined_circle(vec2(px, py), 5.0, Color::new(0.55, 0.45, 0.28, 1.0));
            }
            t += 160.0;
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
