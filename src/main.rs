//! WILDLANDS - a 2D top-down survival and base-building game.
//!
//! Everything on screen is drawn with macroquad primitives: no image assets
//! are loaded.

mod building;
mod player;
mod progression;
mod save;
mod ui;
mod world;

use macroquad::prelude::*;
use macroquad::rand::{gen_range, srand};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use building::{BuildingManager, Ghost};
use player::{Player, SlotItem};
use progression::{Progression, RANGED_UNLOCK_MELEE_LEVEL};
use world::{Enemy, ResourceNode, World, WORLD_SIZE};

#[derive(Clone, Copy, PartialEq, Eq)]
enum GameState {
    StartMenu,
    Playing,
    Paused,
    GameOver,
}

struct Game {
    state: GameState,
    player: Player,
    world: World,
    buildings: BuildingManager,
    progress: Progression,
    camera_target: Vec2,
    ghost: Option<Ghost>,
    elapsed: f32,
    /// Throttles hold-to-build so dragging lays one structure at a time.
    build_cd: f32,
    damage_numbers: bool,
    screen_shake: bool,
    pause_notice: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SaveData {
    player: Player,
    progress: Progression,
    buildings: BuildingManager,
    nodes: Vec<ResourceNode>,
    enemies: Vec<Enemy>,
    elapsed: f32,
    world_elapsed: f32,
    hp_level: usize,
}

fn window_conf() -> Conf {
    Conf {
        window_title: "WILDLANDS".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: false,
        sample_count: 4,
        window_resizable: false,
        ..Default::default()
    }
}

fn fresh_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x5EED)
}

#[macroquad::main(window_conf)]
async fn main() {
    srand(fresh_seed());
    let mut game = Game::new();

    loop {
        let dt = get_frame_time().min(0.05);
        game.update(dt);
        game.draw();
        next_frame().await;
    }
}

impl Game {
    fn new() -> Self {
        let start = vec2(WORLD_SIZE * 0.5, WORLD_SIZE * 0.5);
        Self {
            state: GameState::StartMenu,
            player: Player::new(start),
            world: World::new(),
            buildings: BuildingManager::new(),
            progress: Progression::new(),
            camera_target: start,
            ghost: None,
            elapsed: 0.0,
            build_cd: 0.0,
            damage_numbers: true,
            screen_shake: true,
            pause_notice: String::new(),
        }
    }

    fn save_game(&self) -> Result<(), String> {
        let data = SaveData {
            player: self.player.clone(),
            progress: self.progress.clone(),
            buildings: self.buildings.clone(),
            nodes: self.world.nodes.clone(),
            enemies: self.world.enemies.clone(),
            elapsed: self.elapsed,
            world_elapsed: self.world.elapsed,
            hp_level: self.world.saved_hp_level(),
        };
        let json = serde_json::to_string(&data).map_err(|e| e.to_string())?;
        let path = save::save_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, json).map_err(|e| e.to_string())
    }

    fn load_game(&mut self) -> bool {
        let Ok(text) = fs::read_to_string(save::save_path()) else {
            return false;
        };
        let Ok(data) = serde_json::from_str::<SaveData>(&text) else {
            return false;
        };
        self.player = data.player;
        self.progress = data.progress;
        self.progress.tech_open = false;
        self.progress.banners.clear();
        self.buildings = data.buildings;
        self.buildings.shots.clear();
        self.world = World::from_saved(
            data.nodes,
            data.enemies,
            data.world_elapsed,
            data.hp_level,
        );
        self.elapsed = data.elapsed;
        self.camera_target = self.player.pos;
        self.ghost = None;
        self.state = GameState::Playing;
        true
    }

    fn restart(&mut self) {
        srand(fresh_seed());
        let start = vec2(WORLD_SIZE * 0.5, WORLD_SIZE * 0.5);
        self.player = Player::new(start);
        self.world = World::new();
        self.buildings = BuildingManager::new();
        self.progress = Progression::new();
        self.camera_target = start;
        self.ghost = None;
        self.elapsed = 0.0;
        self.build_cd = 0.0;
        self.state = GameState::Playing;
    }

    fn camera(&self) -> Camera2D {
        let shake = if self.screen_shake && self.state == GameState::Playing {
            self.player.hurt_flash * 16.0
        } else {
            0.0
        };
        let offset = if shake > 0.2 {
            vec2(gen_range(-shake, shake), gen_range(-shake, shake))
        } else {
            Vec2::ZERO
        };
        Camera2D {
            target: self.camera_target + offset,
            zoom: vec2(2.0 / screen_width(), 2.0 / screen_height()),
            ..Default::default()
        }
    }

    fn mouse_world(&self) -> Vec2 {
        let (mx, my) = mouse_position();
        self.camera_target
            + vec2(
                mx - screen_width() * 0.5,
                my - screen_height() * 0.5,
            )
    }

    fn update(&mut self, dt: f32) {
        let (mx, my) = mouse_position();
        let mouse = vec2(mx, my);

        match self.state {
            GameState::StartMenu => {
                let has_save = save::save_exists();
                if is_key_pressed(KeyCode::N)
                    || (is_mouse_button_pressed(MouseButton::Left)
                        && ui::in_rect(mouse, ui::start_button_rect(has_save)))
                {
                    self.restart();
                } else if has_save
                    && (is_key_pressed(KeyCode::Enter)
                        || (is_mouse_button_pressed(MouseButton::Left)
                            && ui::in_rect(mouse, ui::continue_button_rect())))
                {
                    self.load_game();
                } else if !has_save && is_key_pressed(KeyCode::Enter) {
                    self.restart();
                }
            }
            GameState::Playing => self.update_playing(dt, mouse),
            GameState::Paused => {
                if is_key_pressed(KeyCode::P)
                    || is_key_pressed(KeyCode::Escape)
                    || is_key_pressed(KeyCode::Enter)
                {
                    self.state = GameState::Playing;
                    return;
                }
                if is_key_pressed(KeyCode::N) {
                    self.damage_numbers = !self.damage_numbers;
                }
                if is_key_pressed(KeyCode::B) {
                    self.screen_shake = !self.screen_shake;
                }
                if !is_mouse_button_pressed(MouseButton::Left) {
                    return;
                }
                if ui::in_rect(mouse, ui::resume_button_rect()) {
                    self.state = GameState::Playing;
                } else if ui::in_rect(mouse, ui::save_button_rect()) {
                    self.pause_notice = match self.save_game() {
                        Ok(()) => "Game saved".to_string(),
                        Err(e) => format!("Save failed: {}", e),
                    };
                } else if ui::in_rect(mouse, ui::pause_quit_rect()) {
                    let _ = self.save_game();
                    self.state = GameState::StartMenu;
                } else if ui::in_rect(mouse, ui::damage_toggle_rect()) {
                    self.damage_numbers = !self.damage_numbers;
                } else if ui::in_rect(mouse, ui::shake_toggle_rect()) {
                    self.screen_shake = !self.screen_shake;
                }
            }
            GameState::GameOver => {
                if is_key_pressed(KeyCode::Enter)
                    || is_key_pressed(KeyCode::R)
                    || (is_mouse_button_pressed(MouseButton::Left)
                        && ui::in_rect(mouse, ui::restart_button_rect()))
                {
                    self.restart();
                }
            }
        }
    }

    fn update_playing(&mut self, dt: f32, mouse: Vec2) {
        if is_key_pressed(KeyCode::Tab) {
            self.progress.tech_open = !self.progress.tech_open;
        }
        if self.progress.tech_open {
            if is_key_pressed(KeyCode::Escape) {
                self.progress.tech_open = false;
            }
            if is_mouse_button_pressed(MouseButton::Left) {
                ui::tech_tree_click(&mut self.progress, mouse);
            }
            self.ghost = None;
            return;
        }

        if is_key_pressed(KeyCode::Escape)
            || is_key_pressed(KeyCode::P)
            || (is_mouse_button_pressed(MouseButton::Left)
                && ui::in_rect(mouse, ui::pause_button_rect()))
        {
            self.state = GameState::Paused;
            self.ghost = None;
            self.pause_notice = match self.save_game() {
                Ok(()) => "Game autosaved".to_string(),
                Err(e) => format!("Autosave failed: {}", e),
            };
            return;
        }

        self.elapsed += dt;
        self.progress.update(dt);
        self.build_cd = (self.build_cd - dt).max(0.0);

        const KEYS: [KeyCode; 14] = [
            KeyCode::Key1,
            KeyCode::Key2,
            KeyCode::Key3,
            KeyCode::Key4,
            KeyCode::Key5,
            KeyCode::Key6,
            KeyCode::Key7,
            KeyCode::Key8,
            KeyCode::Key9,
            KeyCode::Key0,
            KeyCode::Minus,
            KeyCode::Equal,
            KeyCode::LeftBracket,
            KeyCode::RightBracket,
        ];
        for (i, key) in KEYS.iter().enumerate() {
            if is_key_pressed(*key) {
                self.select_slot(i);
            }
        }
        let mut clicked_hotbar = false;
        if is_mouse_button_pressed(MouseButton::Left) {
            if let Some(i) = ui::hotbar_slot_at(mouse) {
                self.select_slot(i);
                clicked_hotbar = true;
            }
        }

        self.player.update_weapon(&self.progress);

        let aim_vec = self.mouse_world() - self.player.pos;
        if aim_vec.length_squared() > 1.0 {
            self.player.aim = aim_vec.y.atan2(aim_vec.x);
        }

        if is_key_pressed(KeyCode::G) {
            self.buildings.grid_lock = !self.buildings.grid_lock;
            self.progress.show_banner(
                "GRID-LOCK",
                if self.buildings.grid_lock {
                    "Snap to 48 x 48 tiles"
                } else {
                    "Free placement"
                },
            );
        }
        if is_key_pressed(KeyCode::R) {
            self.buildings.rotation = (self.buildings.rotation + 1) % 4;
        }
        if is_key_pressed(KeyCode::N) {
            self.damage_numbers = !self.damage_numbers;
            self.progress.show_banner(
                "DAMAGE NUMBERS",
                if self.damage_numbers { "On" } else { "Off" },
            );
        }
        if is_key_pressed(KeyCode::B) {
            self.screen_shake = !self.screen_shake;
            self.progress.show_banner(
                "SCREEN SHAKE",
                if self.screen_shake { "On" } else { "Off" },
            );
        }

        let mouse_world = self.mouse_world();
        if is_mouse_button_pressed(MouseButton::Right) && ui::hotbar_slot_at(mouse).is_none() {
            match self.buildings.demolish(mouse_world, &mut self.player) {
                Ok(refund) => {
                    self.world.float_text(
                        self.player.pos + vec2(0.0, -46.0),
                        &format!(
                            "DEMOLISHED  +{:.0}W +{:.0}S +{:.0}M",
                            refund.wood, refund.stone, refund.metal
                        ),
                        Color::new(0.78, 0.86, 0.90, 1.0),
                    );
                }
                Err(reason) => self.progress.hint(reason),
            }
        }
        if is_key_pressed(KeyCode::H) {
            match self.buildings.repair(mouse_world, &mut self.player) {
                Ok(cost) => {
                    self.world.float_text(
                        self.player.pos + vec2(0.0, -46.0),
                        &format!("REPAIRED  -{:.0}W -{:.0}S -{:.0}M", cost.wood, cost.stone, cost.metal),
                        Color::new(0.54, 0.94, 0.62, 1.0),
                    );
                }
                Err(reason) => self.progress.hint(reason),
            }
        }

        self.ghost = None;
        let selected = self.player.selected_item();
        let use_action = (is_mouse_button_pressed(MouseButton::Left) && !clicked_hotbar)
            || is_key_pressed(KeyCode::Space);
        let use_action_held = (is_mouse_button_down(MouseButton::Left)
            && !clicked_hotbar
            && ui::hotbar_slot_at(mouse).is_none())
            || is_key_down(KeyCode::Space);

        match selected {
            SlotItem::Melee => {
                if use_action_held {
                    self.world.attack(&mut self.player, &mut self.progress);
                }
            }
            SlotItem::Ranged => {
                if use_action_held {
                    self.world.fire_arrow(&mut self.player);
                }
            }
            SlotItem::AutoRifle => {
                if use_action_held {
                    self.world.fire_auto(&mut self.player);
                }
            }
            SlotItem::Shotgun => {
                if use_action_held {
                    self.world.fire_shotgun(&mut self.player);
                }
            }
            SlotItem::Food => {
                if use_action || (use_action_held && self.player.eat_flash <= 0.0) {
                    self.try_eat();
                }
            }
            SlotItem::Build(_) => {
                let pos = self.buildings.snapped(self.mouse_world());
                if let Some(kind) = selected.build_kind(&self.progress) {
                    let repeat_build = use_action_held && self.build_cd <= 0.0;
                    if use_action || repeat_build {
                        let result = self.buildings.try_place(
                            kind,
                            pos,
                            &mut self.player,
                            &mut self.progress,
                            &self.world,
                        );
                        match result {
                            Ok(()) => {
                                self.build_cd = 0.14;
                                self.world.float_text(
                                    pos + vec2(0.0, -22.0),
                                    kind.name(),
                                    Color::new(0.72, 1.0, 0.78, 1.0),
                                );
                            }
                            Err(reason) => {
                                if use_action {
                                    self.progress.hint(reason);
                                }
                            }
                        }
                    }
                    let ghost = self.buildings.ghost(
                        kind,
                        self.mouse_world(),
                        &self.player,
                        &self.progress,
                        &self.world,
                    );
                    self.ghost = Some(ghost);
                }
            }
        }

        if is_key_pressed(KeyCode::Q) {
            self.try_eat();
        }

        self.player
            .move_and_collide(dt, &self.buildings, &self.world);
        self.player.update_survival(dt);
        self.world.update(
            dt,
            &mut self.player,
            &mut self.buildings,
            &mut self.progress,
        );
        self.buildings.update(
            dt,
            &mut self.player,
            &mut self.world,
            &mut self.progress,
        );

        let half = vec2(screen_width() * 0.5, screen_height() * 0.5);
        let min_x = half.x.min(WORLD_SIZE * 0.5);
        let min_y = half.y.min(WORLD_SIZE * 0.5);
        let follow = 1.0 - (-9.0 * dt).exp();
        self.camera_target += (self.player.pos - self.camera_target) * follow;
        self.camera_target.x = self.camera_target.x.clamp(min_x, WORLD_SIZE - min_x);
        self.camera_target.y = self.camera_target.y.clamp(min_y, WORLD_SIZE - min_y);

        if self.player.health <= 0.0 {
            self.state = GameState::GameOver;
            self.ghost = None;
            self.progress.tech_open = false;
        }
    }

    fn select_slot(&mut self, i: usize) {
        let item = player::hotbar()[i];
        if item.unlocked(&self.progress) {
            self.player.selected = i;
        } else {
            let hint = match item {
                SlotItem::Ranged => format!(
                    "Ranged weapons unlock at Melee Lv {}",
                    RANGED_UNLOCK_MELEE_LEVEL
                ),
                SlotItem::Build(kind) => match kind.unlock() {
                    Some(id) => format!("{}  -  {}", id.name(), id.hint()),
                    None => "Still locked".to_string(),
                },
                _ => "Still locked".to_string(),
            };
            self.progress.hint(&hint);
        }
    }

    fn try_eat(&mut self) {
        if self.player.eat() {
            let pos = self.player.pos;
            self.world.float_text(
                pos + vec2(0.0, -38.0),
                "+25 HP",
                Color::new(0.50, 1.0, 0.56, 1.0),
            );
        } else {
            self.progress.hint("Need 10 Food to eat");
        }
    }

    fn draw(&self) {
        match self.state {
            GameState::StartMenu => {
                set_default_camera();
                ui::draw_start_menu(get_time() as f32, save::save_exists());
            }
            _ => {
                let camera = self.camera();
                set_camera(&camera);
                clear_background(Color::new(0.06, 0.11, 0.08, 1.0));

                let target = camera.target;
                self.world.draw_ground(target);
                self.world.draw_nodes(target);
                self.world.draw_targets(target);
                self.buildings.draw();
                self.player.draw();
                self.world.draw_enemies(target);
                self.world.draw_particles();
                self.world.draw_arrows();
                self.world.draw_bullets();
                self.world.draw_enemy_shots();
                self.buildings.draw_shots();
                if let Some(g) = self.ghost.as_ref() {
                    building::draw_ghost(g);
                }
                if self.damage_numbers {
                    self.world.draw_floaters();
                }

                set_default_camera();
                let (mx, my) = mouse_position();
                let mouse = vec2(mx, my);
                match self.state {
                    GameState::Playing => {
                        ui::draw_enemy_indicators(&self.world, self.camera_target);
                        ui::draw_hud(
                            &self.player,
                            &self.world,
                            &self.buildings,
                            &self.progress,
                            mouse,
                            self.elapsed,
                            self.ghost.as_ref(),
                        );
                    }
                    GameState::Paused => {
                        ui::draw_hud(
                            &self.player,
                            &self.world,
                            &self.buildings,
                            &self.progress,
                            mouse,
                            self.elapsed,
                            self.ghost.as_ref(),
                        );
                        ui::draw_pause_overlay(
                            mouse,
                            self.damage_numbers,
                            self.screen_shake,
                            &self.pause_notice,
                        );
                    }
                    GameState::GameOver => {
                        ui::draw_game_over(
                            &self.progress,
                            self.elapsed,
                            mouse,
                            get_time() as f32,
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_data_round_trips() {
        let game = Game::new();
        let nodes = game.world.nodes.len();
        let enemies = game.world.enemies.len();
        let data = SaveData {
            player: game.player.clone(),
            progress: game.progress.clone(),
            buildings: game.buildings.clone(),
            nodes: game.world.nodes.clone(),
            enemies: game.world.enemies.clone(),
            elapsed: game.elapsed,
            world_elapsed: game.world.elapsed,
            hp_level: game.world.saved_hp_level(),
        };
        let json = serde_json::to_string(&data).expect("serialize save");
        let restored: SaveData = serde_json::from_str(&json).expect("deserialize save");
        assert_eq!(restored.nodes.len(), nodes);
        assert_eq!(restored.enemies.len(), enemies);
        assert_eq!(restored.player.pos, game.player.pos);
        assert_eq!(restored.buildings.scaled_level, 1);
    }

    #[test]
    fn save_file_round_trips_through_disk() {
        let old_home = std::env::var("HOME").ok();
        let dir = std::env::temp_dir().join(format!(
            "wildlands-save-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("temp home");
        std::env::set_var("HOME", &dir);

        let game = Game::new();
        game.save_game().expect("write save");
        assert!(save::save_path().is_file());

        let mut loaded = Game::new();
        assert!(loaded.load_game());
        assert_eq!(loaded.player.pos, game.player.pos);
        assert_eq!(loaded.player.health, game.player.health);
        assert_eq!(loaded.world.nodes.len(), game.world.nodes.len());
        assert!(loaded.state == GameState::Playing);

        if let Some(home) = old_home {
            std::env::set_var("HOME", home);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
