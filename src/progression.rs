//! Uncapped usage-based mastery, structure unlocks, and unlock banners.
//!
//! Each weapon track has ten named tiers. The tenth is Sapphire; levels past
//! ten keep the Sapphire name and keep growing without a cap.

use crate::world::Resource;

pub const MELEE_TIER_COUNT: usize = 10;
pub const RANGED_TIER_COUNT: usize = 10;
pub const SAPPHIRE_LEVEL: usize = 10;
pub const RANGED_UNLOCK_MELEE_LEVEL: usize = 3;

/// Cumulative melee mastery points required for the ten named tiers.
pub const MELEE_THRESHOLDS: [u32; MELEE_TIER_COUNT] =
    [0, 18, 45, 80, 125, 180, 245, 320, 410, 520];
/// Cumulative ranged mastery points required for the ten named tiers.
pub const RANGED_THRESHOLDS: [u32; RANGED_TIER_COUNT] =
    [0, 20, 50, 95, 150, 220, 310, 420, 560, 730];

/// Cumulative melee points needed to reach `level`. Levels beyond the named
/// tiers keep increasing, so there is no level cap.
pub fn melee_threshold(level: usize) -> u32 {
    if level <= 1 {
        return 0;
    }
    if level <= MELEE_TIER_COUNT {
        return MELEE_THRESHOLDS[level - 1];
    }
    let extra = (level - MELEE_TIER_COUNT) as u64;
    let base = MELEE_THRESHOLDS[MELEE_TIER_COUNT - 1] as u64;
    (base + 135 * extra + 18 * extra * extra).min(u32::MAX as u64) as u32
}

/// Cumulative ranged points needed to reach `level`, without a cap.
pub fn ranged_threshold(level: usize) -> u32 {
    if level <= 1 {
        return 0;
    }
    if level <= RANGED_TIER_COUNT {
        return RANGED_THRESHOLDS[level - 1];
    }
    let extra = (level - RANGED_TIER_COUNT) as u64;
    let base = RANGED_THRESHOLDS[RANGED_TIER_COUNT - 1] as u64;
    (base + 110 * extra + 14 * extra * extra).min(u32::MAX as u64) as u32
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnlockId {
    StoneWall,
    MetalWall,
    WoodGate,
    StoneGate,
    MetalGate,
    MetalSpike,
    Turret,
    Campfire,
}

impl UnlockId {
    pub fn idx(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            UnlockId::StoneWall => "Stone Wall",
            UnlockId::MetalWall => "Metal Wall",
            UnlockId::WoodGate => "Wood Gate",
            UnlockId::StoneGate => "Stone Gate",
            UnlockId::MetalGate => "Metal Gate",
            UnlockId::MetalSpike => "Reinforced Metal Spike",
            UnlockId::Turret => "Auto-Turret",
            UnlockId::Campfire => "Healing Campfire",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            UnlockId::StoneWall => "Place 10 Wood Walls",
            UnlockId::MetalWall => "Place 10 Stone Walls",
            UnlockId::WoodGate => "Gather 25 Wood and 10 Stone",
            UnlockId::StoneGate => "Gather 50 Wood and 40 Stone",
            UnlockId::MetalGate => "Gather 60 Wood, 60 Stone and 25 Metal",
            UnlockId::MetalSpike => "Deal damage 15 times with Spike Traps",
            UnlockId::Turret => "Gather 45 Wood, 45 Stone and 20 Metal",
            UnlockId::Campfire => "Gather 35 Wood, 35 Stone and 35 Metal",
        }
    }

    pub fn cost_label(self) -> &'static str {
        match self {
            UnlockId::StoneWall => "15 Stone",
            UnlockId::MetalWall => "25 Stone + 15 Metal",
            UnlockId::WoodGate => "15 Wood + 5 Stone",
            UnlockId::StoneGate => "20 Wood + 20 Stone",
            UnlockId::MetalGate => "25 Wood + 25 Stone + 20 Metal",
            UnlockId::MetalSpike => "15 Wood + 15 Metal",
            UnlockId::Turret => "35 Wood + 35 Stone + 15 Metal",
            UnlockId::Campfire => "25 Wood + 25 Stone + 25 Metal",
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Banner {
    pub headline: String,
    pub text: String,
    pub life: f32,
    pub max_life: f32,
}

impl Banner {
    pub fn alpha(&self) -> f32 {
        let fade_in = ((self.max_life - self.life) / 0.3).clamp(0.0, 1.0);
        let fade_out = (self.life / 0.6).clamp(0.0, 1.0);
        fade_in.min(fade_out)
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Progression {
    unlocked: [bool; 8],
    pub melee_hits: u32,
    pub melee_kills: u32,
    pub ranged_hits: u32,
    pub ranged_kills: u32,
    last_melee_level: usize,
    last_ranged_level: usize,
    pub wood_walls_placed: u32,
    pub stone_walls_placed: u32,
    pub trap_ticks: u32,
    pub total_placed: u32,
    pub total_wood: u32,
    pub total_stone: u32,
    pub total_metal: u32,
    pub total_food: u32,
    pub kills: u32,
    pub banners: Vec<Banner>,
    pub tech_open: bool,
    pub collapsed: [bool; 4],
    pub time_alive: f32,
}

impl Default for Progression {
    fn default() -> Self {
        Self {
            unlocked: [false; 8],
            melee_hits: 0,
            melee_kills: 0,
            ranged_hits: 0,
            ranged_kills: 0,
            last_melee_level: 1,
            last_ranged_level: 0,
            wood_walls_placed: 0,
            stone_walls_placed: 0,
            trap_ticks: 0,
            total_placed: 0,
            total_wood: 0,
            total_stone: 0,
            total_metal: 0,
            total_food: 0,
            kills: 0,
            banners: Vec::new(),
            tech_open: false,
            collapsed: [false; 4],
            time_alive: 0.0,
        }
    }
}

impl Progression {
    pub fn new() -> Self {
        Self::default()
    }

    // -----------------------------------------------------------------------
    // Melee & ranged mastery
    // -----------------------------------------------------------------------

    pub fn melee_points(&self) -> u32 {
        self.melee_hits.saturating_add(self.melee_kills.saturating_mul(6))
    }

    pub fn ranged_points(&self) -> u32 {
        self.ranged_hits
            .saturating_add(self.ranged_kills.saturating_mul(4))
    }

    pub fn melee_level(&self) -> usize {
        let points = self.melee_points();
        let mut level = 1;
        while level < 1_000_000 && melee_threshold(level + 1) <= points {
            level += 1;
        }
        level
    }

    pub fn ranged_unlocked(&self) -> bool {
        self.melee_level() >= RANGED_UNLOCK_MELEE_LEVEL
    }

    pub fn ranged_level(&self) -> usize {
        if !self.ranged_unlocked() {
            return 0;
        }
        let points = self.ranged_points();
        let mut level = 1;
        while level < 1_000_000 && ranged_threshold(level + 1) <= points {
            level += 1;
        }
        level
    }

    /// 0..1 progress towards the next melee level, with a short label.
    pub fn melee_next(&self) -> (f32, String) {
        let level = self.melee_level();
        let previous = melee_threshold(level);
        let next = melee_threshold(level + 1);
        let points = self.melee_points();
        let span = next.saturating_sub(previous).max(1);
        let frac = points.saturating_sub(previous) as f32 / span as f32;
        (
            frac.clamp(0.0, 1.0),
            format!("{} / {} to Lv {}", points, next, level + 1),
        )
    }

    /// 0..1 progress towards the next ranged level, or the melee gate.
    pub fn ranged_next(&self) -> (f32, String) {
        if !self.ranged_unlocked() {
            let required = melee_threshold(RANGED_UNLOCK_MELEE_LEVEL);
            let frac = (self.melee_points() as f32 / required.max(1) as f32).clamp(0.0, 1.0);
            return (
                frac,
                format!("Melee Lv {} to unlock", RANGED_UNLOCK_MELEE_LEVEL),
            );
        }
        let level = self.ranged_level();
        let previous = ranged_threshold(level);
        let next = ranged_threshold(level + 1);
        let points = self.ranged_points();
        let span = next.saturating_sub(previous).max(1);
        let frac = points.saturating_sub(previous) as f32 / span as f32;
        (
            frac.clamp(0.0, 1.0),
            format!("{} / {} to Lv {}", points, next, level + 1),
        )
    }

    // -----------------------------------------------------------------------
    // Structure unlocks
    // -----------------------------------------------------------------------

    pub fn is_unlocked(&self, id: UnlockId) -> bool {
        self.unlocked[id.idx()]
    }

    pub fn unlocked_count(&self) -> usize {
        self.unlocked.iter().filter(|v| **v).count()
    }

    pub fn unlock(&mut self, id: UnlockId) {
        if self.unlocked[id.idx()] {
            return;
        }
        self.unlocked[id.idx()] = true;
        self.show_banner("NEW ITEM UNLOCKED!", id.name());
    }

    pub fn show_banner(&mut self, headline: &str, text: &str) {
        if self.banners.len() >= 3 {
            self.banners.remove(0);
        }
        self.banners.push(Banner {
            headline: headline.to_string(),
            text: text.to_string(),
            life: 4.0,
            max_life: 4.0,
        });
    }

    pub fn hint(&mut self, text: &str) {
        if self.banners.len() >= 3 {
            self.banners.remove(0);
        }
        self.banners.push(Banner {
            headline: "LOCKED".to_string(),
            text: text.to_string(),
            life: 2.2,
            max_life: 2.2,
        });
    }

    pub fn update(&mut self, dt: f32) {
        self.time_alive += dt;

        let melee_level = self.melee_level();
        if melee_level > self.last_melee_level {
            self.last_melee_level = melee_level;
            let name = crate::player::melee_tier_name(melee_level);
            self.show_banner(
                "NEW ITEM UNLOCKED!",
                &format!("Lv {} - {}", melee_level, name),
            );
        }

        self.check_unlocks();

        let ranged_level = self.ranged_level();
        if ranged_level > self.last_ranged_level {
            self.last_ranged_level = ranged_level;
            let name = crate::player::ranged_tier_name(ranged_level);
            if ranged_level == 1 {
                self.show_banner("RANGED WEAPONS UNLOCKED!", name);
            } else {
                self.show_banner(
                    "NEW ITEM UNLOCKED!",
                    &format!("Lv {} - {}", ranged_level, name),
                );
            }
        }

        for b in self.banners.iter_mut() {
            b.life -= dt;
        }
        self.banners.retain(|b| b.life > 0.0);
    }

    pub fn check_unlocks(&mut self) {
        if self.wood_walls_placed >= 10 {
            self.unlock(UnlockId::StoneWall);
        }
        if self.stone_walls_placed >= 10 {
            self.unlock(UnlockId::MetalWall);
        }
        if self.total_wood >= 25 && self.total_stone >= 10 {
            self.unlock(UnlockId::WoodGate);
        }
        if self.total_wood >= 50 && self.total_stone >= 40 {
            self.unlock(UnlockId::StoneGate);
        }
        if self.total_wood >= 60 && self.total_stone >= 60 && self.total_metal >= 25 {
            self.unlock(UnlockId::MetalGate);
        }
        if self.trap_ticks >= 15 {
            self.unlock(UnlockId::MetalSpike);
        }
        if self.total_wood >= 45 && self.total_stone >= 45 && self.total_metal >= 20 {
            self.unlock(UnlockId::Turret);
        }
        if self.total_wood >= 35 && self.total_stone >= 35 && self.total_metal >= 35 {
            self.unlock(UnlockId::Campfire);
        }
    }

    pub fn register_melee_hit(&mut self) {
        self.melee_hits += 1;
    }

    pub fn register_melee_kill(&mut self) {
        self.melee_kills += 1;
        self.kills += 1;
    }

    pub fn register_ranged_hit(&mut self) {
        self.ranged_hits += 1;
    }

    pub fn register_ranged_kill(&mut self) {
        self.ranged_kills += 1;
        self.kills += 1;
    }

    /// Kills credited to traps, turrets or bleeding.
    pub fn register_other_kill(&mut self) {
        self.kills += 1;
    }

    pub fn register_gather(&mut self, resource: Resource, amount: f32) {
        let n = amount.max(0.0).round() as u32;
        match resource {
            Resource::Wood => self.total_wood += n,
            Resource::Stone => self.total_stone += n,
            Resource::Metal => self.total_metal += n,
            Resource::Food => self.total_food += n,
        }
    }

    pub fn register_placed(&mut self, wood_wall: bool, stone_wall: bool) {
        self.total_placed += 1;
        if wood_wall {
            self.wood_walls_placed += 1;
        }
        if stone_wall {
            self.stone_walls_placed += 1;
        }
    }

    pub fn register_trap_damage(&mut self) {
        self.trap_ticks += 1;
    }

    /// Progress fraction plus a short human readable label for a locked item.
    pub fn progress(&self, id: UnlockId) -> (f32, String) {
        let ratio = |a: f32, b: f32| (a / b).clamp(0.0, 1.0);
        match id {
            UnlockId::StoneWall => (
                ratio(self.wood_walls_placed as f32, 10.0),
                format!("{} / 10 wood walls placed", self.wood_walls_placed.min(10)),
            ),
            UnlockId::MetalWall => (
                ratio(self.stone_walls_placed as f32, 10.0),
                format!("{} / 10 stone walls placed", self.stone_walls_placed.min(10)),
            ),
            UnlockId::WoodGate => {
                let frac = ratio(self.total_wood as f32, 25.0)
                    .min(ratio(self.total_stone as f32, 10.0));
                (
                    frac,
                    format!(
                        "Wood {}/25  Stone {}/10",
                        self.total_wood.min(25),
                        self.total_stone.min(10)
                    ),
                )
            }
            UnlockId::StoneGate => {
                let frac = ratio(self.total_wood as f32, 50.0)
                    .min(ratio(self.total_stone as f32, 40.0));
                (
                    frac,
                    format!(
                        "Wood {}/50  Stone {}/40",
                        self.total_wood.min(50),
                        self.total_stone.min(40)
                    ),
                )
            }
            UnlockId::MetalGate => {
                let frac = ratio(self.total_wood as f32, 60.0)
                    .min(ratio(self.total_stone as f32, 60.0))
                    .min(ratio(self.total_metal as f32, 25.0));
                (
                    frac,
                    format!(
                        "Wood {}/60  Stone {}/60  Metal {}/25",
                        self.total_wood.min(60),
                        self.total_stone.min(60),
                        self.total_metal.min(25)
                    ),
                )
            }
            UnlockId::MetalSpike => (
                ratio(self.trap_ticks as f32, 15.0),
                format!("{} / 15 trap damage dealt", self.trap_ticks.min(15)),
            ),
            UnlockId::Turret => {
                let frac = ratio(self.total_wood as f32, 45.0)
                    .min(ratio(self.total_stone as f32, 45.0))
                    .min(ratio(self.total_metal as f32, 20.0));
                (
                    frac,
                    format!(
                        "Wood {}/45  Stone {}/45  Metal {}/20",
                        self.total_wood.min(45),
                        self.total_stone.min(45),
                        self.total_metal.min(20)
                    ),
                )
            }
            UnlockId::Campfire => {
                let frac = ratio(self.total_wood as f32, 35.0)
                    .min(ratio(self.total_stone as f32, 35.0))
                    .min(ratio(self.total_metal as f32, 35.0));
                (
                    frac,
                    format!(
                        "Wood {}/35  Stone {}/35  Metal {}/35",
                        self.total_wood.min(35),
                        self.total_stone.min(35),
                        self.total_metal.min(35)
                    ),
                )
            }
        }
    }
}
