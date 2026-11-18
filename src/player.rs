//! The player: movement, aiming, survival stats, mastery-driven weapons and the
//! animated vector-art survivor.

use macroquad::prelude::*;
use std::f32::consts::PI;

use crate::building::{BuildKind, BuildingManager};
use crate::progression::{
    Progression, UnlockId, MELEE_TIER_COUNT, RANGED_TIER_COUNT, SAPPHIRE_LEVEL,
};
use crate::world::{
    draw_rot_rect_outlined, outlined_circle, rot_vec, with_alpha, Resource, World, OUTLINE,
    WORLD_SIZE,
};

#[derive(Clone, Copy)]
pub struct MeleeTier {
    pub name: &'static str,
    pub damage: f32,
    pub reach: f32,
    pub arc: f32,
    pub cooldown: f32,
    pub gather: f32,
}

#[derive(Clone, Copy)]
pub struct RangedTier {
    pub name: &'static str,
    pub damage: f32,
    pub cooldown: f32,
    pub speed: f32,
    pub range: f32,
}

/// Ten melee mastery levels. Every level is earned by landing hits and kills
/// with the previous tier.
pub const MELEE_TIERS: [MeleeTier; MELEE_TIER_COUNT] = [
    MeleeTier {
        name: "Wood Club",
        damage: 14.0,
        reach: 58.0,
        arc: 1.05,
        cooldown: 0.45,
        gather: 1.0,
    },
    MeleeTier {
        name: "Wood Sword",
        damage: 19.0,
        reach: 62.0,
        arc: 1.0,
        cooldown: 0.40,
        gather: 1.10,
    },
    MeleeTier {
        name: "Stone Axe",
        damage: 25.0,
        reach: 66.0,
        arc: 1.15,
        cooldown: 0.36,
        gather: 1.6,
    },
    MeleeTier {
        name: "Stone Sword",
        damage: 31.0,
        reach: 70.0,
        arc: 1.0,
        cooldown: 0.33,
        gather: 1.75,
    },
    MeleeTier {
        name: "Metal Spear",
        damage: 39.0,
        reach: 94.0,
        arc: 0.8,
        cooldown: 0.40,
        gather: 2.0,
    },
    MeleeTier {
        name: "Metal Sword",
        damage: 47.0,
        reach: 78.0,
        arc: 1.0,
        cooldown: 0.30,
        gather: 2.15,
    },
    MeleeTier {
        name: "Steel Halberd",
        damage: 57.0,
        reach: 100.0,
        arc: 0.9,
        cooldown: 0.36,
        gather: 2.3,
    },
    MeleeTier {
        name: "Crystal Blade",
        damage: 68.0,
        reach: 84.0,
        arc: 1.0,
        cooldown: 0.28,
        gather: 2.5,
    },
    MeleeTier {
        name: "Titan War Hammer",
        damage: 82.0,
        reach: 92.0,
        arc: 1.2,
        cooldown: 0.48,
        gather: 2.8,
    },
    MeleeTier {
        name: "Sapphire Greatsword",
        damage: 115.0,
        reach: 120.0,
        arc: 1.3,
        cooldown: 0.38,
        gather: 3.5,
    },
];

/// Five ranged levels, unlocked once melee mastery reaches level 3.
pub const RANGED_TIERS: [RangedTier; RANGED_TIER_COUNT] = [
    RangedTier {
        name: "Hunting Bow",
        damage: 22.0,
        cooldown: 0.60,
        speed: 660.0,
        range: 540.0,
    },
    RangedTier {
        name: "Recurve Bow",
        damage: 30.0,
        cooldown: 0.50,
        speed: 720.0,
        range: 620.0,
    },
    RangedTier {
        name: "Crossbow",
        damage: 42.0,
        cooldown: 0.68,
        speed: 820.0,
        range: 720.0,
    },
    RangedTier {
        name: "Steel Crossbow",
        damage: 57.0,
        cooldown: 0.58,
        speed: 920.0,
        range: 810.0,
    },
    RangedTier {
        name: "Crystal Longbow",
        damage: 74.0,
        cooldown: 0.50,
        speed: 1020.0,
        range: 940.0,
    },
    RangedTier {
        name: "Composite Bow",
        damage: 90.0,
        cooldown: 0.46,
        speed: 1080.0,
        range: 1000.0,
    },
    RangedTier {
        name: "Runic Bow",
        damage: 108.0,
        cooldown: 0.42,
        speed: 1140.0,
        range: 1060.0,
    },
    RangedTier {
        name: "Dragonbone Bow",
        damage: 128.0,
        cooldown: 0.39,
        speed: 1200.0,
        range: 1120.0,
    },
    RangedTier {
        name: "Void Bow",
        damage: 150.0,
        cooldown: 0.36,
        speed: 1260.0,
        range: 1180.0,
    },
    RangedTier {
        name: "Sapphire Longbow",
        damage: 175.0,
        cooldown: 0.33,
        speed: 1320.0,
        range: 1250.0,
    },
];

/// Damage of a single auto rifle round, before mastery scaling.
pub const RIFLE_BASE_DAMAGE: f32 = 34.0;
/// Damage of one birdshot pellet, before mastery scaling (nine fire per shot).
pub const SHOTGUN_PELLET_DAMAGE: f32 = 40.0;

/// Stats for any melee level. Past Sapphire the name stays the same and the
/// numbers keep climbing forever.
pub fn melee_tier(level: usize) -> MeleeTier {
    let index = level.clamp(1, MELEE_TIER_COUNT) - 1;
    let mut tier = MELEE_TIERS[index];
    let extra = level.saturating_sub(MELEE_TIER_COUNT) as f32;
    if extra > 0.0 {
        tier.damage *= 1.0 + extra * 0.10;
        tier.reach *= 1.0 + extra * 0.01;
        tier.cooldown /= 1.0 + extra * 0.035;
        tier.gather += extra * 0.15;
    }
    tier
}

pub fn melee_tier_name(level: usize) -> &'static str {
    let index = level.clamp(1, MELEE_TIER_COUNT) - 1;
    MELEE_TIERS[index].name
}

/// Stats for any ranged level. Past Sapphire the bow keeps its name and scales
/// damage, range and fire rate without a cap.
pub fn ranged_tier(level: usize) -> RangedTier {
    let index = level.clamp(1, RANGED_TIER_COUNT) - 1;
    let mut tier = RANGED_TIERS[index];
    let extra = level.saturating_sub(RANGED_TIER_COUNT) as f32;
    if extra > 0.0 {
        // Ranged damage outgrows melee (and the 18% enemy HP curve) once the
        // named tiers are exhausted.
        tier.damage *= 1.0 + extra * 0.20;
        tier.cooldown /= 1.0 + extra * 0.035;
        tier.speed *= 1.0 + extra * 0.01;
        tier.range *= 1.0 + extra * 0.01;
    }
    tier
}

pub fn ranged_tier_name(level: usize) -> &'static str {
    let index = level.clamp(1, RANGED_TIER_COUNT) - 1;
    RANGED_TIERS[index].name
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SlotItem {
    Melee,
    Ranged,
    AutoRifle,
    Shotgun,
    Food,
    Build(BuildKind),
}

impl SlotItem {
    pub fn unlocked(self, progress: &Progression) -> bool {
        match self {
            SlotItem::Melee | SlotItem::Food => true,
            SlotItem::AutoRifle | SlotItem::Shotgun => true,
            SlotItem::Ranged => progress.ranged_unlocked(),
            SlotItem::Build(kind) => kind.unlocked(progress),
        }
    }

    /// The trap slot upgrades itself to the reinforced version once mastered.
    pub fn build_kind(self, progress: &Progression) -> Option<BuildKind> {
        match self {
            SlotItem::Build(BuildKind::SpikeTrap)
                if progress.is_unlocked(UnlockId::MetalSpike) =>
            {
                Some(BuildKind::MetalSpike)
            }
            SlotItem::Build(kind) => Some(kind),
            _ => None,
        }
    }
}

/// Fixed 14-slot hotbar. Weapon slots upgrade themselves as mastery grows.
pub fn hotbar() -> [SlotItem; 14] {
    [
        SlotItem::Melee,
        SlotItem::Ranged,
        SlotItem::AutoRifle,
        SlotItem::Shotgun,
        SlotItem::Food,
        SlotItem::Build(BuildKind::WoodWall),
        SlotItem::Build(BuildKind::StoneWall),
        SlotItem::Build(BuildKind::MetalWall),
        SlotItem::Build(BuildKind::Door),
        SlotItem::Build(BuildKind::StoneGate),
        SlotItem::Build(BuildKind::MetalGate),
        SlotItem::Build(BuildKind::SpikeTrap),
        SlotItem::Build(BuildKind::Turret),
        SlotItem::Build(BuildKind::Campfire),
    ]
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Player {
    #[serde(with = "crate::save::vec2_serde")]
    pub pos: Vec2,
    pub radius: f32,
    pub aim: f32,
    pub health: f32,
    pub max_health: f32,
    pub hunger: f32,
    pub max_hunger: f32,
    pub wood: f32,
    pub stone: f32,
    pub metal: f32,
    pub food: f32,
    pub selected: usize,
    pub swing_cd: f32,
    pub swing_timer: f32,
    pub swing_angle: f32,
    pub hurt_flash: f32,
    pub eat_flash: f32,
    pub walk_phase: f32,
    pub moving: bool,
    pub melee_level: usize,
    pub ranged_level: usize,
    pub ranged_cd: f32,
    pub shot_flash: f32,
    /// Brief damage immunity after a hit so packs cannot delete you in one frame.
    pub invuln: f32,
}

impl Player {
    pub fn new(pos: Vec2) -> Self {
        Self {
            pos,
            radius: 16.0,
            aim: 0.0,
            health: 100.0,
            max_health: 100.0,
            hunger: 100.0,
            max_hunger: 100.0,
            wood: 0.0,
            stone: 0.0,
            metal: 0.0,
            food: 0.0,
            selected: 0,
            swing_cd: 0.0,
            swing_timer: 0.0,
            swing_angle: 0.0,
            hurt_flash: 0.0,
            eat_flash: 0.0,
            walk_phase: 0.0,
            moving: false,
            melee_level: 1,
            ranged_level: 0,
            ranged_cd: 0.0,
            shot_flash: 0.0,
            invuln: 0.0,
        }
    }

    pub fn melee(&self) -> MeleeTier {
        melee_tier(self.melee_level.max(1))
    }

    pub fn ranged(&self) -> RangedTier {
        ranged_tier(self.ranged_level.max(1))
    }

    /// The level firearms scale from. Guns improve with either mastery track,
    /// so every level you gain makes them hit harder and shoot faster.
    pub fn combat_level(&self) -> usize {
        self.melee_level.max(self.ranged_level).max(1)
    }

    pub fn gun_damage(&self, base: f32) -> f32 {
        base * (1.0 + (self.combat_level() - 1) as f32 * 0.20)
    }

    /// Lower is faster. Fire rate improves every level and never stops growing.
    pub fn gun_cooldown(&self, base: f32) -> f32 {
        // Asymptotic, never clamped: every level keeps making guns faster.
        base / (1.0 + (self.combat_level() - 1) as f32 * 0.045)
    }

    pub fn gun_name(&self, shotgun: bool) -> &'static str {
        if self.combat_level() >= SAPPHIRE_LEVEL {
            if shotgun {
                "Sapphire Shotgun"
            } else {
                "Sapphire Auto Rifle"
            }
        } else if shotgun {
            "Birdshot Shotgun"
        } else {
            "Auto Rifle"
        }
    }

    pub fn update_weapon(&mut self, progress: &Progression) {
        self.melee_level = progress.melee_level();
        self.ranged_level = progress.ranged_level();
        let target = 100.0 + (self.combat_level() - 1) as f32 * 20.0;
        if target > self.max_health {
            let gain = target - self.max_health;
            self.max_health = target;
            self.health = (self.health + gain).min(self.max_health);
        }
    }

    /// Level-based damage reduction. It approaches 100% but never fully caps.
    pub fn armor_fraction(&self) -> f32 {
        let extra = (self.combat_level() - 1) as f32;
        let raw = extra * 0.035;
        raw / (1.0 + raw)
    }

    pub fn enemy_damage_multiplier(&self) -> f32 {
        1.0 + (self.combat_level() - 1) as f32 * 0.06
    }

    pub fn selected_item(&self) -> SlotItem {
        hotbar()[self.selected]
    }

    pub fn add_resource(&mut self, resource: Resource, amount: f32) {
        match resource {
            Resource::Wood => self.wood += amount,
            Resource::Stone => self.stone += amount,
            Resource::Metal => self.metal += amount,
            Resource::Food => self.food += amount,
        }
    }

    /// Returns the health actually lost, so callers do not show numbers for
    /// hits absorbed by post-hit invulnerability.
    pub fn take_damage(&mut self, amount: f32) -> f32 {
        if self.invuln > 0.0 {
            return 0.0;
        }
        let reduced = amount * (1.0 - self.armor_fraction());
        let before = self.health;
        self.health = (self.health - reduced).max(0.0);
        self.hurt_flash = 0.35;
        self.invuln = 0.35;
        (before - self.health).max(0.0)
    }

    pub fn heal(&mut self, amount: f32) {
        self.health = (self.health + amount).min(self.max_health);
    }

    /// Eat 10 Food for 25 Health and a chunk of hunger.
    pub fn eat(&mut self) -> bool {
        if self.food < 10.0 {
            return false;
        }
        self.food -= 10.0;
        self.heal(25.0);
        self.hunger = (self.hunger + 30.0).min(self.max_hunger);
        self.eat_flash = 0.45;
        true
    }

    pub fn update_survival(&mut self, dt: f32) {
        self.hunger = (self.hunger - 0.55 * dt).max(0.0);
        if self.hunger <= 0.0 {
            self.health = (self.health - 1.4 * dt).max(0.0);
        } else if self.hunger > 45.0 {
            self.health = (self.health + 0.35 * dt).min(self.max_health);
        }
        self.swing_cd = (self.swing_cd - dt).max(0.0);
        self.swing_timer = (self.swing_timer - dt).max(0.0);
        self.hurt_flash = (self.hurt_flash - dt).max(0.0);
        self.eat_flash = (self.eat_flash - dt).max(0.0);
        self.ranged_cd = (self.ranged_cd - dt).max(0.0);
        self.shot_flash = (self.shot_flash - dt).max(0.0);
        self.invuln = (self.invuln - dt).max(0.0);
    }

    pub fn move_and_collide(&mut self, dt: f32, buildings: &BuildingManager, world: &World) {
        let mut dir = Vec2::ZERO;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
            dir.y -= 1.0;
        }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
            dir.y += 1.0;
        }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
            dir.x -= 1.0;
        }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
            dir.x += 1.0;
        }

        if dir.length_squared() > 0.0 {
            dir = dir.normalize();
            self.pos += dir * 238.0 * dt;
            self.moving = true;
            self.walk_phase += dt * 12.0;
        } else {
            self.moving = false;
            self.walk_phase += dt * 2.0;
        }

        self.pos.x = self.pos.x.clamp(40.0, WORLD_SIZE - 40.0);
        self.pos.y = self.pos.y.clamp(40.0, WORLD_SIZE - 40.0);

        // The survivor can step over their own walls and gates; the horde
        // cannot. Turrets and other stations are also walk-through for you.
        for b in buildings.items.iter() {
            if !b.blocks(true) {
                continue;
            }
            let off = self.pos - b.pos;
            let min_dist = self.radius + 22.0;
            if off.length_squared() < min_dist * min_dist {
                let push = if off.length_squared() > 0.01 {
                    off.normalize()
                } else {
                    vec2(1.0, 0.0)
                };
                self.pos = b.pos + push * min_dist;
            }
        }

        for n in world.nodes.iter() {
            if !n.kind.solid() {
                continue;
            }
            let off = self.pos - n.pos;
            let min_dist = self.radius + n.radius * 0.7;
            if off.length_squared() < min_dist * min_dist {
                let push = if off.length_squared() > 0.01 {
                    off.normalize()
                } else {
                    vec2(1.0, 0.0)
                };
                self.pos = n.pos + push * min_dist;
            }
        }
    }

    pub fn draw(&self) {
        let dir = vec2(self.aim.cos(), self.aim.sin());
        let side = vec2(-dir.y, dir.x);

        draw_circle(
            self.pos.x + 5.0,
            self.pos.y + 13.0,
            18.0,
            Color::new(0.02, 0.05, 0.03, 0.18),
        );
        draw_circle(
            self.pos.x + 1.0,
            self.pos.y + 11.0,
            13.0,
            Color::new(0.02, 0.05, 0.03, 0.14),
        );

        let step = if self.moving {
            self.walk_phase.sin() * 7.0
        } else {
            0.0
        };
        for s in [-1.0, 1.0] {
            let leg = self.pos + side * (7.5 * s) + dir * (step * s) + vec2(0.0, 6.0);
            outlined_circle(leg, 5.4, Color::new(0.20, 0.24, 0.32, 1.0));
        }

        // Wind-caught cloak, quiver and layered shoulders give the survivor a
        // painted silhouette instead of a plain disc.
        let cloak_sway = if self.moving {
            self.walk_phase.sin() * 3.5
        } else {
            (get_time() as f32 * 1.2).sin() * 1.2
        };
        tri_outlined(
            self.pos + side * 11.0 - dir * 4.0,
            self.pos - side * 11.0 - dir * 4.0,
            self.pos - dir * 23.0 + side * cloak_sway,
            Color::new(0.08, 0.30, 0.34, 1.0),
        );
        draw_line(
            self.pos.x + side.x * (step * 0.8),
            self.pos.y + side.y * (step * 0.8),
            self.pos.x - dir.x * 18.0,
            self.pos.y - dir.y * 18.0,
            1.2,
            Color::new(0.28, 0.57, 0.56, 0.45),
        );
        let quiver = self.pos - dir * 5.0 + side * 11.0;
        draw_rot_rect_outlined(
            quiver,
            8.0,
            21.0,
            self.aim + 0.48,
            Color::new(0.34, 0.22, 0.13, 1.0),
        );
        for i in [-1.0, 1.0] {
            let fletch = quiver - dir * 10.0 + side * (2.5 * i);
            draw_line(
                fletch.x,
                fletch.y,
                fletch.x - dir.x * 7.0,
                fletch.y - dir.y * 7.0,
                1.8,
                Color::new(0.86, 0.85, 0.78, 1.0),
            );
        }

        draw_rot_rect_outlined(
            self.pos - dir * 9.0,
            16.0,
            20.0,
            self.aim,
            Color::new(0.40, 0.29, 0.18, 1.0),
        );

        let body_color = if self.hurt_flash > 0.2 {
            Color::new(0.95, 0.45, 0.42, 1.0)
        } else {
            Color::new(0.20, 0.60, 0.70, 1.0)
        };
        outlined_circle(self.pos, 16.0, body_color);
        draw_circle(
            self.pos.x - dir.x * 3.0,
            self.pos.y - dir.y * 3.0,
            10.0,
            Color::new(0.31, 0.73, 0.80, 1.0),
        );
        for s in [-1.0, 1.0] {
            outlined_circle(
                self.pos + side * (12.0 * s) - dir * 1.0,
                5.6,
                Color::new(0.16, 0.43, 0.49, 1.0),
            );
        }
        let belt_a = self.pos + side * 12.0 + dir * 3.0;
        let belt_b = self.pos - side * 12.0 + dir * 3.0;
        draw_line(
            belt_a.x,
            belt_a.y,
            belt_b.x,
            belt_b.y,
            3.0,
            Color::new(0.13, 0.21, 0.24, 0.9),
        );
        outlined_circle(self.pos + dir * 3.0, 3.0, Color::new(0.86, 0.66, 0.28, 1.0));

        let head = self.pos + dir * 3.0;
        outlined_circle(head - dir * 2.0, 11.5, Color::new(0.16, 0.14, 0.12, 1.0));
        outlined_circle(head, 10.0, Color::new(0.95, 0.78, 0.62, 1.0));
        outlined_circle(
            head - dir * 4.0 - vec2(0.0, 3.0),
            8.5,
            Color::new(0.95, 0.78, 0.62, 1.0),
        );
        for s in [-1.0, 1.0] {
            let eye = head + side * (4.2 * s) + dir * 4.0;
            draw_circle(eye.x, eye.y, 1.9, OUTLINE);
            draw_circle(eye.x - 0.5, eye.y - 0.6, 0.7, Color::new(0.86, 0.96, 1.0, 0.9));
        }
        draw_rot_rect_outlined(
            head - dir * 8.0,
            14.0,
            5.0,
            self.aim,
            Color::new(0.14, 0.34, 0.38, 1.0),
        );
        draw_circle(
            head.x - dir.x * 3.0 - side.x * 3.0,
            head.y - dir.y * 3.0 - side.y * 3.0,
            3.2,
            Color::new(1.0, 0.94, 0.82, 0.16),
        );

        let weapon = self.selected_item();
        let ranged_weapon = matches!(
            weapon,
            SlotItem::Ranged | SlotItem::AutoRifle | SlotItem::Shotgun
        );
        let swing_offset = if !ranged_weapon && self.swing_timer > 0.0 {
            (1.0 - self.swing_timer / 0.22) * 1.5 - 0.75
        } else {
            0.0
        };
        let w_angle = self.aim + swing_offset;
        let hand = self.pos + rot_vec(vec2(13.0, 9.0), w_angle);
        let gun_hand = self.pos + rot_vec(vec2(18.0, 7.0), self.aim);

        match weapon {
            SlotItem::Ranged => {
                let bow_hand = self.pos + rot_vec(vec2(16.0, 10.0), self.aim);
                draw_bow(bow_hand, self.aim, self.shot_flash > 0.0);
            }
            SlotItem::AutoRifle => {
                draw_gun(gun_hand, self.aim, false, self.shot_flash > 0.0)
            }
            SlotItem::Shotgun => {
                draw_gun(gun_hand, self.aim, true, self.shot_flash > 0.0)
            }
            _ => draw_melee_weapon(self.melee_level, hand, w_angle),
        }

        let stats = self.melee();
        if self.swing_timer > 0.0 && !ranged_weapon {
            let t = 1.0 - self.swing_timer / 0.22;
            let half = stats.arc;
            let start = self.aim - half;
            let end = start + 2.0 * half * t;
            draw_arc(
                self.pos.x,
                self.pos.y,
                20,
                stats.reach * 0.95,
                start.to_degrees(),
                6.0,
                (end - start).to_degrees(),
                Color::new(1.0, 1.0, 0.92, 0.30 + 0.4 * (1.0 - t)),
            );
        }

        if self.eat_flash > 0.0 {
            let a = (self.eat_flash / 0.45).clamp(0.0, 1.0);
            draw_circle(
                self.pos.x,
                self.pos.y - 28.0,
                10.0 * a,
                with_alpha(Color::new(0.45, 1.0, 0.5, 1.0), a * 0.8),
            );
        }
    }
}

fn tri_outlined(a: Vec2, b: Vec2, c: Vec2, fill: Color) {
    let center = (a + b + c) / 3.0;
    let expand = |p: Vec2| p + (p - center).normalize_or_zero() * 2.6;
    draw_triangle(expand(a), expand(b), expand(c), OUTLINE);
    draw_triangle(a, b, c, fill);
}

fn draw_sword(hand: Vec2, angle: f32, length: f32, width: f32, blade: Color) {
    let handle = hand + rot_vec(vec2(-4.0, 0.0), angle);
    draw_rot_rect_outlined(handle, 16.0, 6.0, angle, Color::new(0.47, 0.31, 0.17, 1.0));
    let guard = hand + rot_vec(vec2(6.0, 0.0), angle);
    draw_rot_rect_outlined(
        guard,
        6.0,
        width + 12.0,
        angle,
        Color::new(0.86, 0.74, 0.34, 1.0),
    );
    let blade_center = guard + rot_vec(vec2(length * 0.5, 0.0), angle);
    draw_rot_rect_outlined(blade_center, length, width, angle, blade);
}

fn draw_melee_weapon(level: usize, hand: Vec2, angle: f32) {
    let brown = Color::new(0.50, 0.34, 0.18, 1.0);
    match level {
        1 => {
            draw_rot_rect_outlined(hand, 32.0, 9.0, angle, brown);
            outlined_circle(
                hand + rot_vec(vec2(18.0, 0.0), angle),
                6.5,
                Color::new(0.60, 0.41, 0.22, 1.0),
            );
        }
        2 => draw_sword(hand, angle, 24.0, 8.0, Color::new(0.62, 0.46, 0.26, 1.0)),
        3 => {
            draw_rot_rect_outlined(hand, 30.0, 6.0, angle, brown);
            let head = hand + rot_vec(vec2(18.0, 0.0), angle);
            tri_outlined(
                head + rot_vec(vec2(-2.0, -11.0), angle),
                head + rot_vec(vec2(-2.0, 11.0), angle),
                head + rot_vec(vec2(10.0, 0.0), angle),
                Color::new(0.64, 0.67, 0.70, 1.0),
            );
        }
        4 => draw_sword(hand, angle, 27.0, 9.0, Color::new(0.68, 0.71, 0.73, 1.0)),
        5 => {
            draw_rot_rect_outlined(hand, 48.0, 5.0, angle, brown);
            let tip = hand + rot_vec(vec2(30.0, 0.0), angle);
            tri_outlined(
                tip + rot_vec(vec2(-10.0, -6.0), angle),
                tip + rot_vec(vec2(-10.0, 6.0), angle),
                tip,
                Color::new(0.72, 0.94, 1.0, 1.0),
            );
        }
        6 => draw_sword(hand, angle, 32.0, 10.0, Color::new(0.76, 0.80, 0.84, 1.0)),
        7 => {
            draw_rot_rect_outlined(hand, 58.0, 5.0, angle, brown);
            let head = hand + rot_vec(vec2(26.0, 0.0), angle);
            tri_outlined(
                head + rot_vec(vec2(-4.0, -13.0), angle),
                head + rot_vec(vec2(-4.0, 5.0), angle),
                head + rot_vec(vec2(10.0, -2.0), angle),
                Color::new(0.72, 0.76, 0.80, 1.0),
            );
            let tip = hand + rot_vec(vec2(31.0, 0.0), angle);
            tri_outlined(
                tip + rot_vec(vec2(-8.0, -4.0), angle),
                tip + rot_vec(vec2(-8.0, 4.0), angle),
                tip,
                Color::new(0.90, 0.80, 0.40, 1.0),
            );
        }
        8 => {
            draw_sword(hand, angle, 34.0, 11.0, Color::new(0.52, 0.94, 1.0, 1.0));
            draw_circle(
                (hand + rot_vec(vec2(28.0, 0.0), angle)).x,
                (hand + rot_vec(vec2(28.0, 0.0), angle)).y,
                5.0,
                Color::new(0.80, 1.0, 1.0, 0.5),
            );
        }
        9 => {
            draw_rot_rect_outlined(hand, 30.0, 7.0, angle, brown);
            let head = hand + rot_vec(vec2(21.0, 0.0), angle);
            draw_rot_rect_outlined(head, 13.0, 28.0, angle, Color::new(0.58, 0.61, 0.65, 1.0));
            outlined_circle(head, 6.0, Color::new(0.80, 0.72, 0.34, 1.0));
        }
        _ => {
            draw_sword(hand, angle, 46.0, 16.0, Color::new(0.96, 0.82, 0.36, 1.0));
            outlined_circle(
                hand + rot_vec(vec2(4.0, 0.0), angle),
                8.0,
                Color::new(0.52, 0.94, 1.0, 1.0),
            );
        }
    }
}

fn draw_gun(hand: Vec2, angle: f32, shotgun: bool, firing: bool) {
    let dark = Color::new(0.24, 0.27, 0.32, 1.0);
    let metal = Color::new(0.50, 0.55, 0.62, 1.0);
    let wood = Color::new(0.52, 0.35, 0.19, 1.0);
    let barrel_len = if shotgun { 44.0 } else { 54.0 };

    draw_rot_rect_outlined(
        hand - rot_vec(vec2(11.0, 2.0), angle),
        20.0,
        10.0,
        angle,
        wood,
    );
    draw_rot_rect_outlined(
        hand + rot_vec(vec2(8.0, 0.0), angle),
        26.0,
        9.0,
        angle,
        dark,
    );
    draw_rot_rect_outlined(
        hand + rot_vec(vec2(8.0 + barrel_len * 0.5, 0.0), angle),
        barrel_len,
        if shotgun { 9.0 } else { 6.0 },
        angle,
        metal,
    );
    if shotgun {
        draw_rot_rect_outlined(
            hand + rot_vec(vec2(25.0, 1.0), angle),
            18.0,
            12.0,
            angle,
            wood,
        );
    } else {
        draw_rot_rect_outlined(
            hand + rot_vec(vec2(22.0, 6.0), angle),
            20.0,
            6.0,
            angle,
            dark,
        );
        draw_rot_rect_outlined(
            hand + rot_vec(vec2(6.0, -7.0), angle),
            12.0,
            4.0,
            angle,
            Color::new(0.34, 0.38, 0.44, 1.0),
        );
    }

    if firing {
        let muzzle = hand + rot_vec(vec2(8.0 + barrel_len, 0.0), angle);
        draw_circle(
            muzzle.x,
            muzzle.y,
            if shotgun { 12.0 } else { 7.0 },
            Color::new(1.0, 0.80, 0.30, 0.75),
        );
        draw_circle(
            muzzle.x,
            muzzle.y,
            if shotgun { 5.5 } else { 3.2 },
            Color::new(1.0, 0.98, 0.82, 1.0),
        );
    }
}

fn draw_bow(hand: Vec2, angle: f32, firing: bool) {
    let brown = Color::new(0.55, 0.36, 0.20, 1.0);
    let start = angle - 1.35;
    draw_arc(
        hand.x,
        hand.y,
        12,
        21.0,
        start.to_degrees(),
        4.0,
        2.7f32.to_degrees(),
        OUTLINE,
    );
    draw_arc(
        hand.x,
        hand.y,
        12,
        19.0,
        start.to_degrees(),
        2.5,
        2.7f32.to_degrees(),
        brown,
    );
    let p1 = hand + vec2((angle - 1.35).cos(), (angle - 1.35).sin()) * 21.0;
    let p2 = hand + vec2((angle + 1.35).cos(), (angle + 1.35).sin()) * 21.0;
    draw_line(
        p1.x,
        p1.y,
        p2.x,
        p2.y,
        1.5,
        Color::new(0.90, 0.90, 0.82, 0.9),
    );
    if firing {
        let tip = hand + vec2(angle.cos(), angle.sin()) * 30.0;
        draw_line(
            hand.x,
            hand.y,
            tip.x,
            tip.y,
            2.5,
            Color::new(1.0, 0.88, 0.40, 1.0),
        );
        draw_circle(tip.x, tip.y, 3.0, Color::new(1.0, 0.92, 0.50, 1.0));
    }
    let _ = PI;
}
