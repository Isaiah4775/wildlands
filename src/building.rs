//! Grid-locked base building: three tiers of walls and gates, traps, turrets
//! and campfires, with ghost previews and procedural vector art.

use macroquad::prelude::*;
use macroquad::rand::gen_range;
use std::f32::consts::{PI, TAU};

use crate::player::Player;
use crate::progression::{Progression, UnlockId};
use crate::world::{
    draw_rot_rect, draw_rot_rect_outlined, draw_text_center, outlined_circle, outlined_poly,
    rot_vec, shadow, with_alpha, Floater, World, OUTLINE,
};

pub const TILE: f32 = 48.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum BuildKind {
    WoodWall,
    StoneWall,
    MetalWall,
    Door,
    StoneGate,
    MetalGate,
    SpikeTrap,
    MetalSpike,
    Turret,
    Campfire,
}

#[derive(Clone, Copy, Default)]
pub struct Cost {
    pub wood: f32,
    pub stone: f32,
    pub metal: f32,
}

impl BuildKind {
    pub fn name(self) -> &'static str {
        match self {
            BuildKind::WoodWall => "Wood Wall",
            BuildKind::StoneWall => "Stone Wall",
            BuildKind::MetalWall => "Metal Wall",
            BuildKind::Door => "Wood Gate",
            BuildKind::StoneGate => "Stone Gate",
            BuildKind::MetalGate => "Metal Gate",
            BuildKind::SpikeTrap => "Spike Trap",
            BuildKind::MetalSpike => "Reinforced Metal Spike",
            BuildKind::Turret => "Auto-Turret",
            BuildKind::Campfire => "Healing Campfire",
        }
    }

    pub fn cost(self) -> Cost {
        match self {
            BuildKind::WoodWall => Cost {
                wood: 5.0,
                stone: 0.0,
                metal: 0.0,
            },
            BuildKind::StoneWall => Cost {
                wood: 0.0,
                stone: 15.0,
                metal: 0.0,
            },
            BuildKind::MetalWall => Cost {
                wood: 0.0,
                stone: 25.0,
                metal: 15.0,
            },
            BuildKind::SpikeTrap => Cost {
                wood: 10.0,
                stone: 0.0,
                metal: 0.0,
            },
            BuildKind::MetalSpike => Cost {
                wood: 15.0,
                stone: 0.0,
                metal: 15.0,
            },
            BuildKind::Door => Cost {
                wood: 15.0,
                stone: 5.0,
                metal: 0.0,
            },
            BuildKind::StoneGate => Cost {
                wood: 20.0,
                stone: 20.0,
                metal: 0.0,
            },
            BuildKind::MetalGate => Cost {
                wood: 25.0,
                stone: 25.0,
                metal: 20.0,
            },
            BuildKind::Turret => Cost {
                wood: 35.0,
                stone: 35.0,
                metal: 15.0,
            },
            BuildKind::Campfire => Cost {
                wood: 25.0,
                stone: 25.0,
                metal: 25.0,
            },
        }
    }

    pub fn max_hp(self) -> f32 {
        match self {
            BuildKind::WoodWall => 140.0,
            BuildKind::StoneWall => 420.0,
            BuildKind::MetalWall => 900.0,
            BuildKind::SpikeTrap => 80.0,
            BuildKind::MetalSpike => 130.0,
            BuildKind::Door => 240.0,
            BuildKind::StoneGate => 520.0,
            BuildKind::MetalGate => 950.0,
            BuildKind::Turret => 220.0,
            BuildKind::Campfire => 180.0,
        }
    }

    pub fn unlock(self) -> Option<UnlockId> {
        match self {
            BuildKind::WoodWall | BuildKind::SpikeTrap => None,
            BuildKind::StoneWall => Some(UnlockId::StoneWall),
            BuildKind::MetalWall => Some(UnlockId::MetalWall),
            BuildKind::MetalSpike => Some(UnlockId::MetalSpike),
            BuildKind::Door => Some(UnlockId::WoodGate),
            BuildKind::StoneGate => Some(UnlockId::StoneGate),
            BuildKind::MetalGate => Some(UnlockId::MetalGate),
            BuildKind::Turret => Some(UnlockId::Turret),
            BuildKind::Campfire => Some(UnlockId::Campfire),
        }
    }

    pub fn unlocked(self, progress: &Progression) -> bool {
        self.unlock().map_or(true, |id| progress.is_unlocked(id))
    }

    pub fn cost_label(self) -> String {
        let c = self.cost();
        let mut parts: Vec<String> = Vec::new();
        if c.wood > 0.0 {
            parts.push(format!("{:.0} Wood", c.wood));
        }
        if c.stone > 0.0 {
            parts.push(format!("{:.0} Stone", c.stone));
        }
        if c.metal > 0.0 {
            parts.push(format!("{:.0} Metal", c.metal));
        }
        parts.join(" + ")
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Building {
    pub kind: BuildKind,
    #[serde(with = "crate::save::vec2_serde")]
    pub pos: Vec2,
    pub rot: u8,
    pub hp: f32,
    pub max_hp: f32,
    pub cooldown: f32,
    pub open: bool,
    pub flash: f32,
    pub aim: f32,
    pub seed: f32,
}

impl Building {
    pub fn angle(&self) -> f32 {
        self.rot as f32 * PI * 0.5
    }

    pub fn blocks(&self, is_player: bool) -> bool {
        // The survivor can walk across every structure they place. The horde
        // is stopped by walls, gates and turrets.
        if is_player {
            return false;
        }
        match self.kind {
            BuildKind::WoodWall
            | BuildKind::StoneWall
            | BuildKind::MetalWall
            | BuildKind::Door
            | BuildKind::StoneGate
            | BuildKind::MetalGate
            | BuildKind::Turret => true,
            _ => false,
        }
    }
}

#[derive(Clone)]
pub struct Shot {
    pub pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub dmg: f32,
}

pub struct Ghost {
    pub kind: BuildKind,
    pub pos: Vec2,
    pub rot: u8,
    pub valid: bool,
    pub reason: &'static str,
    pub show_grid: bool,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct BuildingManager {
    pub items: Vec<Building>,
    #[serde(skip)]
    pub shots: Vec<Shot>,
    pub grid_lock: bool,
    pub rotation: u8,
    /// Mastery level the structures' HP has been scaled to.
    pub scaled_level: usize,
}

fn defense_mult(level: usize) -> f32 {
    1.0 + level.max(1).saturating_sub(1) as f32 * 0.15
}

impl BuildingManager {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            shots: Vec::new(),
            grid_lock: true,
            rotation: 0,
            scaled_level: 1,
        }
    }

    fn target_near(&self, mouse_world: Vec2, player_pos: Vec2) -> Option<usize> {
        let mut best = None;
        let mut best_dist = 64.0;
        for (i, b) in self.items.iter().enumerate() {
            let d = b.pos.distance(mouse_world);
            if d < best_dist && b.pos.distance(player_pos) < 240.0 {
                best_dist = d;
                best = Some(i);
            }
        }
        best
    }

    pub fn demolish(
        &mut self,
        mouse_world: Vec2,
        player: &mut Player,
    ) -> Result<Cost, &'static str> {
        let Some(index) = self.target_near(mouse_world, player.pos) else {
            return Err("Aim at a nearby structure to demolish");
        };
        let building = self.items.remove(index);
        let full = building.kind.cost();
        let refund = Cost {
            wood: (full.wood * 0.5).floor(),
            stone: (full.stone * 0.5).floor(),
            metal: (full.metal * 0.5).floor(),
        };
        player.wood += refund.wood;
        player.stone += refund.stone;
        player.metal += refund.metal;
        Ok(refund)
    }

    pub fn repair(
        &mut self,
        mouse_world: Vec2,
        player: &mut Player,
    ) -> Result<Cost, &'static str> {
        let Some(index) = self.target_near(mouse_world, player.pos) else {
            return Err("Aim at a nearby structure to repair");
        };
        let b = &mut self.items[index];
        if b.hp >= b.max_hp - 0.5 {
            return Err("That structure is already at full health");
        }
        let full = b.kind.cost();
        let cost = Cost {
            wood: (full.wood * 0.2).ceil(),
            stone: (full.stone * 0.2).ceil(),
            metal: (full.metal * 0.2).ceil(),
        };
        if player.wood < cost.wood || player.stone < cost.stone || player.metal < cost.metal {
            return Err("Not enough resources to repair");
        }
        player.wood -= cost.wood;
        player.stone -= cost.stone;
        player.metal -= cost.metal;
        b.hp = (b.hp + b.max_hp * 0.4).min(b.max_hp);
        Ok(cost)
    }

    /// Grid-lock snapping to the centre of a 48x48 tile.
    pub fn snapped(&self, mouse_world: Vec2) -> Vec2 {
        if self.grid_lock {
            vec2(
                (mouse_world.x / TILE).floor() * TILE + TILE * 0.5,
                (mouse_world.y / TILE).floor() * TILE + TILE * 0.5,
            )
        } else {
            mouse_world
        }
    }

    pub fn ghost(
        &self,
        kind: BuildKind,
        mouse_world: Vec2,
        player: &Player,
        progress: &Progression,
        world: &World,
    ) -> Ghost {
        let pos = self.snapped(mouse_world);
        match self.check(kind, pos, player, progress, world) {
            Ok(()) => Ghost {
                kind,
                pos,
                rot: self.rotation,
                valid: true,
                reason: "",
                show_grid: self.grid_lock,
            },
            Err(reason) => Ghost {
                kind,
                pos,
                rot: self.rotation,
                valid: false,
                reason,
                show_grid: self.grid_lock,
            },
        }
    }

    pub fn check(
        &self,
        kind: BuildKind,
        pos: Vec2,
        player: &Player,
        progress: &Progression,
        world: &World,
    ) -> Result<(), &'static str> {
        if !kind.unlocked(progress) {
            return Err("Locked - open the tech tree (Tab)");
        }
        let cost = kind.cost();
        if player.wood < cost.wood || player.stone < cost.stone || player.metal < cost.metal {
            return Err("Not enough resources");
        }
        if self.items.iter().any(|b| b.pos.distance(pos) < TILE - 3.0) {
            return Err("Tile occupied");
        }
        if world
            .nodes
            .iter()
            .any(|n| n.pos.distance(pos) < n.radius + 26.0)
        {
            return Err("Resource in the way");
        }
        if world.is_water(pos) {
            return Err("Cannot build on water");
        }
        if player.pos.distance(pos) < 34.0 {
            return Err("Step back to build");
        }
        Ok(())
    }

    pub fn try_place(
        &mut self,
        kind: BuildKind,
        pos: Vec2,
        player: &mut Player,
        progress: &mut Progression,
        world: &World,
    ) -> Result<(), &'static str> {
        self.check(kind, pos, player, progress, world)?;
        let cost = kind.cost();
        player.wood -= cost.wood;
        player.stone -= cost.stone;
        player.metal -= cost.metal;
        let hp = kind.max_hp() * defense_mult(player.combat_level());
        self.items.push(Building {
            kind,
            pos,
            rot: self.rotation,
            hp,
            max_hp: hp,
            cooldown: 0.0,
            open: false,
            flash: 0.0,
            aim: 0.0,
            seed: gen_range(0.0, 10.0),
        });
        progress.register_placed(
            kind == BuildKind::WoodWall,
            kind == BuildKind::StoneWall,
        );
        Ok(())
    }

    pub fn update(
        &mut self,
        dt: f32,
        player: &mut Player,
        world: &mut World,
        progress: &mut Progression,
    ) {
        let level = player.combat_level();
        if level > self.scaled_level {
            let ratio = defense_mult(level) / defense_mult(self.scaled_level);
            for b in self.items.iter_mut() {
                b.max_hp *= ratio;
                b.hp = (b.hp * ratio).min(b.max_hp);
            }
            self.scaled_level = level;
        }
        let fortify = defense_mult(level);
        let mut heal_rate = 0.0;
        let mut new_shots: Vec<Shot> = Vec::new();

        for b in self.items.iter_mut() {
            if b.flash > 0.0 {
                b.flash = (b.flash - dt).max(0.0);
            }
            if b.cooldown > 0.0 {
                b.cooldown = (b.cooldown - dt).max(0.0);
            }

            match b.kind {
                BuildKind::Door | BuildKind::StoneGate | BuildKind::MetalGate => {
                    b.open = b.pos.distance(player.pos) < 78.0;
                }
                BuildKind::SpikeTrap | BuildKind::MetalSpike => {
                    if b.cooldown <= 0.0 {
                        let dmg = if b.kind == BuildKind::MetalSpike {
                            34.0
                        } else {
                            16.0
                        } * fortify;
                        let mut hits = 0u32;
                        for e in world.enemies.iter_mut() {
                            if e.pos.distance(b.pos) < 30.0 + e.radius() * 0.4 {
                                e.hp -= dmg;
                                e.hit_flash = 0.15;
                                if b.kind == BuildKind::MetalSpike {
                                    e.bleed = 4.0;
                                }
                                hits += 1;
                                world.floaters.push(Floater {
                                    pos: e.pos + vec2(0.0, -20.0),
                                    text: format!("{:.0}", dmg),
                                    color: Color::new(1.0, 0.66, 0.22, 1.0),
                                    life: 0.7,
                                    max_life: 0.7,
                                });
                            }
                        }
                        if hits > 0 {
                            b.cooldown = 0.8;
                            for _ in 0..hits {
                                progress.register_trap_damage();
                            }
                        }
                    }
                }
                BuildKind::Turret => {
                    let mut target: Option<Vec2> = None;
                    let mut best = 430.0;
                    for e in world.enemies.iter() {
                        let d = e.pos.distance(b.pos);
                        if d < best {
                            best = d;
                            target = Some(e.pos);
                        }
                    }
                    if let Some(tp) = target {
                        b.aim = (tp.y - b.pos.y).atan2(tp.x - b.pos.x);
                        if b.cooldown <= 0.0 {
                            b.cooldown = 0.55;
                            new_shots.push(Shot {
                                pos: b.pos + rot_vec(vec2(22.0, 0.0), b.aim),
                                vel: rot_vec(vec2(620.0, 0.0), b.aim),
                                life: 0.85,
                                dmg: 16.0 * fortify,
                            });
                        }
                    }
                }
                BuildKind::Campfire => {
                    if b.pos.distance(player.pos) < 125.0 {
                        heal_rate += 2.6 * fortify;
                    }
                }
                _ => {}
            }
        }

        self.shots.extend(new_shots);

        if heal_rate > 0.0 {
            player.heal(heal_rate * dt);
        }

        for si in 0..self.shots.len() {
            let spos = self.shots[si].pos;
            let mut hit_enemy: Option<usize> = None;
            for (ei, e) in world.enemies.iter().enumerate() {
                if e.hp > 0.0 && e.pos.distance(spos) < e.radius() + 6.0 {
                    hit_enemy = Some(ei);
                    break;
                }
            }
            if let Some(ei) = hit_enemy {
                let dmg = self.shots[si].dmg;
                world.enemies[ei].hp -= dmg;
                world.enemies[ei].hit_flash = 0.14;
                self.shots[si].life = 0.0;
                let epos = world.enemies[ei].pos;
                world.floaters.push(Floater {
                    pos: epos + vec2(0.0, -18.0),
                    text: format!("{:.0}", dmg),
                    color: Color::new(0.75, 0.95, 1.0, 1.0),
                    life: 0.7,
                    max_life: 0.7,
                });
            }
        }

        for s in self.shots.iter_mut() {
            s.pos += s.vel * dt;
            s.life -= dt;
        }
        self.shots.retain(|s| s.life > 0.0);
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    pub fn draw(&self) {
        let time = get_time() as f32;
        for b in self.items.iter() {
            let flash = b.flash > 0.0;
            if b.kind != BuildKind::Campfire {
                draw_rot_rect(
                    b.pos + vec2(4.0, 5.0),
                    46.0,
                    46.0,
                    b.angle(),
                    Color::new(0.01, 0.03, 0.02, 0.22),
                );
            }
            match b.kind {
                BuildKind::WoodWall => {
                    let angle = b.angle();
                    draw_rot_rect_outlined(b.pos, 42.0, 42.0, angle, Color::new(0.50, 0.34, 0.19, 1.0));
                    for i in -1..=1 {
                        let p = b.pos + rot_vec(vec2(0.0, i as f32 * 11.0), angle);
                        let a = p - rot_vec(vec2(16.0, 0.0), angle);
                        let c = p + rot_vec(vec2(16.0, 0.0), angle);
                        draw_line(
                            a.x,
                            a.y,
                            c.x,
                            c.y,
                            2.4,
                            Color::new(0.35, 0.23, 0.12, 1.0),
                        );
                    }
                    for s in [-1.0, 1.0] {
                        draw_circle(
                            b.pos.x + s * 13.0,
                            b.pos.y - 14.0,
                            2.2,
                            Color::new(0.80, 0.74, 0.55, 1.0),
                        );
                    }
                }
                BuildKind::StoneWall => {
                    let angle = b.angle();
                    draw_rot_rect_outlined(b.pos, 42.0, 42.0, angle, Color::new(0.56, 0.59, 0.62, 1.0));
                    for (ox, oy) in [(-9.0, -9.0), (9.0, -9.0), (-9.0, 9.0), (9.0, 9.0)] {
                        let p = b.pos + rot_vec(vec2(ox, oy), angle);
                        draw_rectangle(
                            p.x - 7.0,
                            p.y - 7.0,
                            14.0,
                            14.0,
                            Color::new(0.67, 0.70, 0.72, 1.0),
                        );
                        draw_rectangle_lines(
                            p.x - 7.0,
                            p.y - 7.0,
                            14.0,
                            14.0,
                            1.4,
                            Color::new(0.40, 0.43, 0.46, 1.0),
                        );
                    }
                    draw_rect_outlined(b.pos, 7.0, Color::new(0.48, 0.51, 0.54, 1.0));
                }
                BuildKind::MetalWall => {
                    let angle = b.angle();
                    draw_rot_rect_outlined(
                        b.pos,
                        42.0,
                        42.0,
                        angle,
                        Color::new(0.38, 0.45, 0.52, 1.0),
                    );
                    let a = b.pos + rot_vec(vec2(-14.0, -14.0), angle);
                    let c = b.pos + rot_vec(vec2(14.0, 14.0), angle);
                    draw_line(a.x, a.y, c.x, c.y, 4.0, Color::new(0.25, 0.31, 0.37, 1.0));
                    let d = b.pos + rot_vec(vec2(14.0, -14.0), angle);
                    let e = b.pos + rot_vec(vec2(-14.0, 14.0), angle);
                    draw_line(d.x, d.y, e.x, e.y, 4.0, Color::new(0.25, 0.31, 0.37, 1.0));
                    for (ox, oy) in [(-14.0, -14.0), (14.0, -14.0), (-14.0, 14.0), (14.0, 14.0)] {
                        let p = b.pos + rot_vec(vec2(ox, oy), angle);
                        draw_circle(p.x, p.y, 3.2, Color::new(0.18, 0.23, 0.28, 1.0));
                        draw_circle(p.x - 0.6, p.y - 0.6, 1.6, Color::new(0.70, 0.80, 0.86, 1.0));
                    }
                    draw_line(
                        b.pos.x - 10.0,
                        b.pos.y - 17.0,
                        b.pos.x + 7.0,
                        b.pos.y + 17.0,
                        2.0,
                        Color::new(0.55, 0.90, 1.0, 0.55),
                    );
                }
                BuildKind::SpikeTrap | BuildKind::MetalSpike => {
                    let metal = b.kind == BuildKind::MetalSpike;
                    let plate = if metal {
                        Color::new(0.28, 0.38, 0.44, 1.0)
                    } else {
                        Color::new(0.38, 0.39, 0.42, 1.0)
                    };
                    let spike = if metal {
                        Color::new(0.60, 0.92, 1.0, 1.0)
                    } else {
                        Color::new(0.78, 0.80, 0.82, 1.0)
                    };
                    draw_rot_rect_outlined(b.pos, 40.0, 40.0, b.angle(), plate);
                    for (ox, oy) in [(-10.0, -10.0), (10.0, -10.0), (-10.0, 10.0), (10.0, 10.0), (0.0, 0.0)] {
                        let p = b.pos + rot_vec(vec2(ox, oy), b.angle());
                        outlined_poly(p, 3, 6.5, -PI * 0.5, spike);
                    }
                }
                BuildKind::Door | BuildKind::StoneGate | BuildKind::MetalGate => {
                    let (frame, panel, trim) = match b.kind {
                        BuildKind::Door => (
                            Color::new(0.46, 0.32, 0.19, 1.0),
                            Color::new(0.64, 0.46, 0.26, 1.0),
                            Color::new(0.94, 0.80, 0.38, 1.0),
                        ),
                        BuildKind::StoneGate => (
                            Color::new(0.44, 0.47, 0.50, 1.0),
                            Color::new(0.63, 0.66, 0.69, 1.0),
                            Color::new(0.90, 0.92, 0.95, 1.0),
                        ),
                        _ => (
                            Color::new(0.27, 0.35, 0.42, 1.0),
                            Color::new(0.45, 0.64, 0.72, 1.0),
                            Color::new(0.62, 0.94, 1.0, 1.0),
                        ),
                    };
                    let angle = b.angle();
                    draw_rot_rect_outlined(b.pos, 44.0, 44.0, angle, frame);
                    let door_angle = if b.open { angle + PI * 0.42 } else { angle };
                    let offset = if b.open {
                        rot_vec(vec2(13.0, 0.0), angle)
                    } else {
                        Vec2::ZERO
                    };
                    draw_rot_rect_outlined(b.pos + offset, 30.0, 36.0, door_angle, panel);
                    let stripe_a = b.pos + offset + rot_vec(vec2(-13.0, 0.0), door_angle);
                    let stripe_b = b.pos + offset + rot_vec(vec2(13.0, 0.0), door_angle);
                    draw_line(
                        stripe_a.x,
                        stripe_a.y,
                        stripe_b.x,
                        stripe_b.y,
                        2.0,
                        with_alpha(OUTLINE, 0.6),
                    );
                    let knob = b.pos + offset + rot_vec(vec2(7.0, 11.0), door_angle);
                    outlined_circle(knob, 3.0, trim);
                    draw_circle(
                        b.pos.x + 13.0,
                        b.pos.y - 16.0,
                        3.5,
                        if b.open {
                            Color::new(0.35, 0.95, 0.45, 1.0)
                        } else {
                            Color::new(0.95, 0.36, 0.32, 1.0)
                        },
                    );
                }
                BuildKind::Turret => {
                    shadow(b.pos, 20.0);
                    draw_rot_rect_outlined(
                        b.pos + rot_vec(vec2(15.0, 0.0), b.aim),
                        30.0,
                        10.0,
                        b.aim,
                        Color::new(0.66, 0.69, 0.72, 1.0),
                    );
                    outlined_circle(b.pos + rot_vec(vec2(28.0, 0.0), b.aim), 5.0, Color::new(0.30, 0.32, 0.36, 1.0));
                    outlined_circle(b.pos, 19.0, Color::new(0.36, 0.39, 0.44, 1.0));
                    outlined_circle(b.pos, 12.0, Color::new(0.88, 0.76, 0.30, 1.0));
                    draw_circle(b.pos.x - 2.0, b.pos.y - 2.0, 4.0, Color::new(1.0, 0.92, 0.55, 1.0));
                }
                BuildKind::Campfire => {
                    let pulse = 0.5 + 0.5 * (time * 3.4 + b.seed).sin();
                    draw_circle(
                        b.pos.x,
                        b.pos.y,
                        125.0,
                        Color::new(1.0, 0.60, 0.22, 0.06 + 0.035 * pulse),
                    );
                    draw_circle(
                        b.pos.x,
                        b.pos.y,
                        70.0,
                        Color::new(1.0, 0.66, 0.28, 0.09 + 0.03 * pulse),
                    );
                    shadow(b.pos, 18.0);
                    for i in 0..8 {
                        let a = i as f32 / 8.0 * TAU + b.seed;
                        let p = b.pos + vec2(a.cos(), a.sin()) * 22.0;
                        outlined_circle(p, 5.0, Color::new(0.58, 0.56, 0.50, 1.0));
                    }
                    draw_rot_rect_outlined(b.pos, 32.0, 9.0, 0.55, Color::new(0.44, 0.28, 0.15, 1.0));
                    draw_rot_rect_outlined(b.pos, 32.0, 9.0, -0.55, Color::new(0.50, 0.32, 0.17, 1.0));
                    let h = 16.0 + pulse * 6.0;
                    let f = b.pos + vec2(0.0, -8.0);
                    draw_poly(f.x, f.y, 3, h + 3.0, -PI * 0.5, OUTLINE);
                    draw_poly(f.x, f.y, 3, h, -PI * 0.5, Color::new(1.0, 0.55, 0.16, 0.95));
                    draw_poly(f.x, f.y + 3.0, 3, h * 0.55, -PI * 0.5, Color::new(1.0, 0.88, 0.42, 1.0));
                    for i in 0..6 {
                        let t = (time * 0.9 + i as f32 * 0.19 + b.seed).fract();
                        let spark_x = b.pos.x + (i as f32 * 2.4 + time * 2.1).sin() * (4.0 + t * 8.0);
                        let spark_y = b.pos.y - 10.0 - t * 38.0;
                        draw_circle(
                            spark_x,
                            spark_y,
                            1.6 * (1.0 - t) + 0.4,
                            Color::new(1.0, 0.62 + t * 0.2, 0.18, (1.0 - t) * 0.8),
                        );
                    }
                }
            }

            if !matches!(b.kind, BuildKind::Turret | BuildKind::Campfire) {
                let damage = 1.0 - (b.hp / b.max_hp).clamp(0.0, 1.0);
                if damage > 0.2 {
                    let angle = b.angle();
                    for i in 0..3 {
                        let start = b.pos + rot_vec(vec2(-13.0 + i as f32 * 11.0, -10.0), angle);
                        let mid = start + rot_vec(vec2(4.0, 9.0), angle);
                        let end = mid + rot_vec(vec2(-3.0, 8.0), angle);
                        let alpha = 0.22 + damage * 0.38;
                        draw_line(start.x, start.y, mid.x, mid.y, 1.7, Color::new(0.04, 0.04, 0.04, alpha));
                        draw_line(mid.x, mid.y, end.x, end.y, 1.4, Color::new(0.04, 0.04, 0.04, alpha));
                    }
                }
            }

            if flash {
                let a = (b.flash / 0.22).clamp(0.0, 1.0) * 0.4;
                draw_circle(
                    b.pos.x,
                    b.pos.y,
                    26.0,
                    Color::new(1.0, 0.95, 0.9, a),
                );
            }
            if b.hp < b.max_hp {
                draw_health_bar(
                    b.pos + vec2(0.0, -32.0),
                    44.0,
                    6.0,
                    b.hp / b.max_hp,
                    Color::new(0.95, 0.72, 0.28, 1.0),
                );
            }
        }
    }

    pub fn draw_shots(&self) {
        for s in self.shots.iter() {
            let tail = s.pos - s.vel * 0.02;
            draw_line(
                tail.x,
                tail.y,
                s.pos.x,
                s.pos.y,
                3.0,
                Color::new(1.0, 0.9, 0.5, 0.55),
            );
            outlined_circle(s.pos, 4.0, Color::new(1.0, 0.86, 0.32, 1.0));
        }
    }
}

fn draw_rect_outlined(pos: Vec2, size: f32, color: Color) {
    draw_rectangle(
        pos.x - size * 0.5 - 2.0,
        pos.y - size * 0.5 - 2.0,
        size + 4.0,
        size + 4.0,
        OUTLINE,
    );
    draw_rectangle(
        pos.x - size * 0.5,
        pos.y - size * 0.5,
        size,
        size,
        color,
    );
}

fn draw_health_bar(center_top: Vec2, width: f32, height: f32, frac: f32, fill: Color) {
    let x = center_top.x - width * 0.5;
    let y = center_top.y;
    draw_rectangle(x - 2.0, y - 2.0, width + 4.0, height + 4.0, OUTLINE);
    draw_rectangle(x, y, width, height, Color::new(0.13, 0.14, 0.18, 1.0));
    draw_rectangle(x, y, width * frac.clamp(0.0, 1.0), height, fill);
}

/// Draw the translucent placement preview and its local grid.
pub fn draw_ghost(g: &Ghost) {
    let valid_color = Color::new(0.35, 1.0, 0.45, 1.0);
    let invalid_color = Color::new(1.0, 0.30, 0.30, 1.0);
    let outline = if g.valid { valid_color } else { invalid_color };
    let fill = with_alpha(outline, 0.32);
    let angle = g.rot as f32 * PI * 0.5;

    if g.show_grid {
        let base = vec2(
            (g.pos.x / TILE).floor() * TILE,
            (g.pos.y / TILE).floor() * TILE,
        );
        for dx in -2..=2 {
            for dy in -2..=2 {
                let x = base.x + dx as f32 * TILE;
                let y = base.y + dy as f32 * TILE;
                draw_rectangle(x, y, TILE, TILE, with_alpha(WHITE, 0.02));
                draw_rectangle_lines(
                    x,
                    y,
                    TILE,
                    TILE,
                    1.0,
                    Color::new(1.0, 1.0, 1.0, 0.16),
                );
            }
        }
        draw_rectangle_lines(
            base.x,
            base.y,
            TILE,
            TILE,
            2.0,
            Color::new(1.0, 1.0, 1.0, 0.55),
        );
    }

    draw_rectangle(
        g.pos.x - TILE * 0.5,
        g.pos.y - TILE * 0.5,
        TILE,
        TILE,
        fill,
    );

    match g.kind {
        BuildKind::Turret => {
            draw_circle_lines(g.pos.x, g.pos.y, 430.0, 1.5, with_alpha(outline, 0.3));
            outlined_circle(g.pos, 18.0, fill);
            draw_rot_rect_outlined(g.pos + rot_vec(vec2(14.0, 0.0), angle), 26.0, 9.0, angle, fill);
        }
        BuildKind::Campfire => {
            draw_circle_lines(g.pos.x, g.pos.y, 125.0, 2.0, with_alpha(outline, 0.4));
            outlined_circle(g.pos, 18.0, fill);
        }
        BuildKind::SpikeTrap | BuildKind::MetalSpike => {
            draw_rot_rect_outlined(g.pos, 40.0, 40.0, angle, fill);
        }
        _ => {
            draw_rot_rect_outlined(g.pos, 42.0, 42.0, angle, fill);
        }
    }

    draw_rectangle_lines(
        g.pos.x - TILE * 0.5,
        g.pos.y - TILE * 0.5,
        TILE,
        TILE,
        2.5,
        outline,
    );

    if !g.valid {
        draw_text_center(
            g.reason,
            g.pos.x,
            g.pos.y - 44.0,
            16.0,
            Color::new(1.0, 0.62, 0.58, 1.0),
            Some(OUTLINE),
        );
    }
}
