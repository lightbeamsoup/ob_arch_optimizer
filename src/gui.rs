use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use eframe::egui;

use crate::cost_data;
use crate::game_data::{
    ALL_CURRENCIES, CARD_QUALITIES, CARD_RARITIES, CardKey, CardQuality, Currency,
    UPGRADE_SPECS,
    base_stat_cap,
};
use crate::presentation::{
    explicit_upgrade_max_level, format_current_stats, format_recommendations,
    format_stat_breakdown, upgrade_gui_label, upgrade_total_effect,
};
use crate::sim::{
    derive_stats, finalize_skill_optimizer, simulate_run, simulate_run_internal,
    start_skill_optimizer, step_skill_optimizer,
};
use crate::state::{Config, DEFAULT_STATE_FILE, Objective, load_state_into, save_state};
use crate::types::{OptimizerHistoryPoint, SimulationResult, SkillOptimizerState, UpgradeCatalog};

const RUN_FRAGMENT_CURRENCIES: [crate::game_data::Currency; 6] = [
    crate::game_data::Currency::Common,
    crate::game_data::Currency::Rare,
    crate::game_data::Currency::Epic,
    crate::game_data::Currency::Legendary,
    crate::game_data::Currency::Mythic,
    crate::game_data::Currency::Divine,
];

fn ui_u32_stepper(ui: &mut egui::Ui, value: &mut u32, min: u32, max: Option<u32>) {
    if let Some(max) = max {
        *value = (*value).clamp(min, max);
    } else {
        *value = (*value).max(min);
    }

    ui.horizontal(|ui| {
        if ui.button("-").clicked() && *value > min {
            *value -= 1;
        }
        let mut drag = egui::DragValue::new(value).speed(1.0).range(min..=u32::MAX);
        if let Some(max) = max {
            drag = drag.range(min..=max);
        }
        ui.add(drag);
        if ui.button("+").clicked() {
            match max {
                Some(max) if *value < max => *value += 1,
                None => *value += 1,
                _ => {}
            }
        }
    });
}

struct GuiApp {
    cfg: Config,
    state_path: String,
    sim_count: u32,
    output: String,
    verbose_output_text: String,
    status: String,
    upgrade_catalog: UpgradeCatalog,
    batch_sim: Option<BatchSimState>,
    optimizer_state: Option<SkillOptimizerState>,
    optimizer_selected_point: Option<usize>,
    optimizer_last_summary: String,
}

struct BatchSimState {
    requested_runs: u32,
    completed_runs: u32,
    stop_requested: bool,
    cfg_snapshot: Config,
    state_path: PathBuf,
    xp_sum: f64,
    xp_min: Option<f64>,
    xp_max: Option<f64>,
    stage_min: Option<u32>,
    stage_max: Option<u32>,
    fragment_sum: HashMap<crate::game_data::Currency, f64>,
    fragment_min: HashMap<crate::game_data::Currency, f64>,
    fragment_max: HashMap<crate::game_data::Currency, f64>,
}

impl BatchSimState {
    fn new(cfg_snapshot: Config, state_path: PathBuf, requested_runs: u32) -> Self {
        Self {
            requested_runs,
            completed_runs: 0,
            stop_requested: false,
            cfg_snapshot,
            state_path,
            xp_sum: 0.0,
            xp_min: None,
            xp_max: None,
            stage_min: None,
            stage_max: None,
            fragment_sum: HashMap::new(),
            fragment_min: HashMap::new(),
            fragment_max: HashMap::new(),
        }
    }

    fn record(&mut self, result: &SimulationResult) {
        self.completed_runs += 1;
        self.xp_sum += result.xp_per_run;
        self.xp_min = Some(
            self.xp_min
                .map(|current| current.min(result.xp_per_run))
                .unwrap_or(result.xp_per_run),
        );
        self.xp_max = Some(
            self.xp_max
                .map(|current| current.max(result.xp_per_run))
                .unwrap_or(result.xp_per_run),
        );
        self.stage_min = Some(
            self.stage_min
                .map(|current| current.min(result.max_stage_reached))
                .unwrap_or(result.max_stage_reached),
        );
        self.stage_max = Some(
            self.stage_max
                .map(|current| current.max(result.max_stage_reached))
                .unwrap_or(result.max_stage_reached),
        );
        for currency in RUN_FRAGMENT_CURRENCIES {
            let value = result
                .fragment_per_run
                .get(&currency)
                .copied()
                .unwrap_or(0.0);
            *self.fragment_sum.entry(currency).or_insert(0.0) += value;
            self.fragment_min
                .entry(currency)
                .and_modify(|current| *current = current.min(value))
                .or_insert(value);
            self.fragment_max
                .entry(currency)
                .and_modify(|current| *current = current.max(value))
                .or_insert(value);
        }
    }

    fn average_xp(&self) -> f64 {
        if self.completed_runs == 0 {
            0.0
        } else {
            self.xp_sum / self.completed_runs as f64
        }
    }

    fn average_fragments(&self, currency: crate::game_data::Currency) -> f64 {
        if self.completed_runs == 0 {
            0.0
        } else {
            self.fragment_sum.get(&currency).copied().unwrap_or(0.0) / self.completed_runs as f64
        }
    }

    fn summary(&self) -> String {
        let mut out = String::new();
        let state = if self.stop_requested && self.completed_runs < self.requested_runs {
            "stop requested"
        } else if self.completed_runs >= self.requested_runs {
            "complete"
        } else {
            "running"
        };
        out.push_str("Batch Simulation\n");
        out.push_str(&format!(
            "Progress: {}/{} ({state})\n",
            self.completed_runs, self.requested_runs
        ));
        out.push_str(&format!("Average XP/run: {:.3}\n", self.average_xp()));
        out.push_str(&format!(
            "Min XP/run: {:.3}  Max XP/run: {:.3}\n",
            self.xp_min.unwrap_or(0.0),
            self.xp_max.unwrap_or(0.0)
        ));
        out.push_str(&format!(
            "Min stage: {}  Max stage: {}\n",
            self.stage_min.unwrap_or(0),
            self.stage_max.unwrap_or(0)
        ));
        out.push('\n');
        out.push_str("Fragments/run\n");
        for currency in RUN_FRAGMENT_CURRENCIES {
            out.push_str(&format!(
                "{currency}: avg {:.4}  min {:.4}  max {:.4}\n",
                self.average_fragments(currency),
                self.fragment_min.get(&currency).copied().unwrap_or(0.0),
                self.fragment_max.get(&currency).copied().unwrap_or(0.0)
            ));
        }
        out
    }
}

impl GuiApp {
    fn optimizer_objective_label(&self) -> String {
        match self.cfg.objective {
            Objective::Fragments => match self.cfg.fragment_objective_currency {
                Some(currency) => format!("{currency} fragments/sec"),
                None => "total fragments/sec".to_string(),
            },
            Objective::Experience => "xp/sec".to_string(),
            Objective::MaxLevel => "max level score".to_string(),
        }
    }

    fn apply_best_optimizer_skills(&mut self) {
        let Some(state) = self.optimizer_state.as_ref() else {
            return;
        };
        if !state.finished {
            return;
        }
        let summary = finalize_skill_optimizer(state);
        self.cfg.skills = summary.best_skills;
        self.optimizer_selected_point = state.history.len().checked_sub(1);
        self.status = "Applied optimized skills to the current build".to_string();
    }

    fn new(mut cfg: Config) -> Self {
        let default_path = env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(DEFAULT_STATE_FILE);
        let should_auto_load =
            !cfg.no_auto_load && cfg.load_state.is_none() && default_path.exists();
        let mut status = String::new();
        if should_auto_load {
            match load_state_into(&default_path, &mut cfg) {
                Ok(()) => {
                    cfg.load_state = Some(default_path.clone());
                    cfg.save_state = Some(default_path.clone());
                    status = format!("Auto-loaded {}", default_path.display());
                }
                Err(err) => {
                    status = err;
                }
            }
        }
        let state_path = cfg
            .save_state
            .as_ref()
            .or(cfg.load_state.as_ref())
            .unwrap_or(&default_path)
            .display()
            .to_string();
        cfg.save_state = Some(PathBuf::from(&state_path));
        let upgrade_catalog = cost_data::built_in_upgrade_catalog();
        let mut app = Self {
            cfg,
            state_path,
            sim_count: 1,
            output: String::new(),
            verbose_output_text: String::new(),
            status,
            upgrade_catalog,
            batch_sim: None,
            optimizer_state: None,
            optimizer_selected_point: None,
            optimizer_last_summary: String::new(),
        };
        app.refresh_verbose_output();
        app
    }

    fn refresh_catalog(&mut self) {
        self.upgrade_catalog = cost_data::built_in_upgrade_catalog();
    }

    fn effective_state_path(&self) -> PathBuf {
        let trimmed = self.state_path.trim();
        if trimmed.is_empty() {
            PathBuf::from(DEFAULT_STATE_FILE)
        } else {
            PathBuf::from(trimmed)
        }
    }

    fn load_state(&mut self) {
        let path = self.effective_state_path();
        match load_state_into(&path, &mut self.cfg) {
            Ok(()) => {
                self.cfg.load_state = Some(path.clone());
                self.cfg.save_state = Some(path.clone());
                self.refresh_catalog();
                self.refresh_verbose_output();
                self.status = format!("Loaded {}", path.display());
            }
            Err(err) => self.status = err,
        }
    }

    fn save_state(&mut self) {
        let path = self.effective_state_path();
        self.cfg.save_state = Some(path.clone());
        match save_state(&path, &self.cfg) {
            Ok(()) => self.status = format!("Saved {}", path.display()),
            Err(err) => self.status = err,
        }
    }

    fn start_optimizer(&mut self) {
        self.refresh_catalog();
        self.optimizer_selected_point = None;
        self.optimizer_last_summary.clear();
        self.optimizer_state = start_skill_optimizer(&self.cfg);
        self.status = if self.optimizer_state.is_some() {
            "Running optimizer".to_string()
        } else {
            "Optimizer disabled".to_string()
        };
    }

    fn stop_optimizer(&mut self) {
        if let Some(state) = self.optimizer_state.as_mut() {
            state.stop_requested = true;
            self.status = format!(
                "Optimizer stop requested. Finishing loop {} after {} evaluations",
                state.loops + 1,
                state.pending_neighbors.len()
            );
        }
    }

    fn step_optimizer(&mut self) {
        let objective_label = self.optimizer_objective_label();
        let Some(state) = self.optimizer_state.as_mut() else {
            return;
        };
        let was_finished = state.finished;
        if !state.finished {
            step_skill_optimizer(state);
            self.status = if state.stop_requested {
                format!(
                    "Optimizer stopping after loop {}: eval {}",
                    state.loops + 1,
                    state.evaluations
                )
            } else {
                format!(
                    "Optimizer running: loop {} eval {}",
                    state.loops, state.evaluations
                )
            };
            if let Some(last) = state.history.last() {
                self.optimizer_last_summary = format_optimizer_point(last, &self.cfg);
            }
        }
        if !was_finished && state.finished {
            let summary = finalize_skill_optimizer(state);
            self.optimizer_last_summary = format!(
                "Objective: {}\nBest value: {:.6}\nLoops: {}\nEvaluations: {}\nSimulations: {}\nBest skills: str {} agi {} per {} int {} luck {} div {} corr {}",
                objective_label,
                summary.best_value,
                summary.loops,
                summary.evaluations,
                summary.simulations,
                summary.best_skills.strength,
                summary.best_skills.agility,
                summary.best_skills.perception,
                summary.best_skills.intellect,
                summary.best_skills.luck,
                summary.best_skills.divinity,
                summary.best_skills.corruption
            );
            let mut recommendation_cfg = self.cfg.clone();
            recommendation_cfg.optimize_skills = false;
            self.output = format_recommendations(
                &recommendation_cfg,
                &self.upgrade_catalog,
                &simulate_run_internal(&recommendation_cfg, false),
            );
            self.status = if state.stop_requested {
                "Optimizer stopped after current loop".to_string()
            } else {
                "Optimizer complete".to_string()
            };
        }
    }

    fn start_batch_simulation(&mut self) {
        let path = self.effective_state_path();
        self.cfg.save_state = Some(path.clone());
        self.batch_sim = Some(BatchSimState::new(
            self.cfg.clone(),
            path,
            self.sim_count.clamp(1, 1000),
        ));
        self.refresh_verbose_output();
        self.output = self
            .batch_sim
            .as_ref()
            .map(BatchSimState::summary)
            .unwrap_or_default();
        self.status = format!("Running {} simulations", self.sim_count.clamp(1, 1000));
    }

    fn stop_batch_simulation(&mut self) {
        if let Some(batch) = self.batch_sim.as_mut() {
            batch.stop_requested = true;
            self.status = format!(
                "Stop requested. Finishing after simulation {}",
                batch.completed_runs
            );
        }
    }

    fn step_batch_simulation(&mut self) {
        let Some(batch) = self.batch_sim.as_mut() else {
            return;
        };
        if !batch.stop_requested && batch.completed_runs < batch.requested_runs {
            let result = simulate_run(&batch.cfg_snapshot);
            batch.record(&result);
            self.output = batch.summary();
            self.status = format!(
                "Completed {}/{} simulations",
                batch.completed_runs, batch.requested_runs
            );
            self.cfg.highest_stage_reached = self
                .cfg
                .highest_stage_reached
                .max(result.updated_highest_stage);
        }
        if batch.completed_runs >= batch.requested_runs || batch.stop_requested {
            let path = batch.state_path.clone();
            let finished = batch.completed_runs;
            let requested = batch.requested_runs;
            self.output = batch.summary();
            self.batch_sim = None;
            match save_state(&path, &self.cfg) {
                Ok(()) => {
                    self.status = format!(
                        "Finished {finished}/{requested} simulations and saved {}",
                        path.display()
                    )
                }
                Err(err) => self.status = err,
            }
        }
    }

    fn refresh_verbose_output(&mut self) {
        if self.cfg.verbose_output {
            self.verbose_output_text = format_stat_breakdown(&self.cfg);
        } else {
            self.verbose_output_text.clear();
        }
    }

    fn draw_optimizer_graph(&mut self, ui: &mut egui::Ui) {
        let Some(state) = self.optimizer_state.as_ref() else {
            if self.optimizer_last_summary.is_empty() {
                return;
            }
            ui.label("Last optimizer run");
            ui.add(
                egui::TextEdit::multiline(&mut self.optimizer_last_summary)
                    .desired_rows(10)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace)
                    .interactive(false),
            );
            return;
        };
        if state.history.is_empty() {
            return;
        }
        let history = state.history.clone();
        let evaluations = state.evaluations;
        let loops = state.loops;
        let current_value = state.current_eval.objective_value;
        let objective_label = self.optimizer_objective_label();
        let optimizer_finished = state.finished;
        let mut apply_best_clicked = false;
        ui.heading("Optimizer Trace");
        ui.columns(2, |columns| {
            let desired = egui::vec2(columns[0].available_width(), 220.0);
            let (response, painter) = columns[0].allocate_painter(desired, egui::Sense::click());
            let rect = response.rect;
            painter.rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.0, columns[0].style().visuals.window_stroke.color),
                egui::StrokeKind::Outside,
            );
            let min_value = history
                .iter()
                .map(|p| p.objective_value)
                .fold(f64::INFINITY, f64::min);
            let max_value = history
                .iter()
                .map(|p| p.objective_value)
                .fold(f64::NEG_INFINITY, f64::max);
            let value_span = (max_value - min_value).max(1e-9);
            let max_index = history.len().saturating_sub(1).max(1) as f32;
            let points: Vec<egui::Pos2> = history
                .iter()
                .enumerate()
                .map(|(i, point)| {
                    let x = rect.left() + rect.width() * (i as f32 / max_index);
                    let y_norm = ((point.objective_value - min_value) / value_span) as f32;
                    let y = rect.bottom() - rect.height() * y_norm;
                    egui::pos2(x, y)
                })
                .collect();
            painter.add(egui::Shape::line(
                points.clone(),
                egui::Stroke::new(2.0, egui::Color32::from_rgb(40, 120, 200)),
            ));
            for (i, point) in points.iter().enumerate() {
                let selected = self.optimizer_selected_point == Some(i);
                painter.circle_filled(
                    *point,
                    if selected { 4.0 } else { 2.5 },
                    if selected {
                        egui::Color32::from_rgb(220, 90, 60)
                    } else {
                        egui::Color32::from_rgb(40, 120, 200)
                    },
                );
            }
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let mut best: Option<(usize, f32)> = None;
                    for (i, point) in points.iter().enumerate() {
                        let d = point.distance(pos);
                        let current = best.map(|(_, dist)| dist).unwrap_or(f32::INFINITY);
                        if d < current {
                            best = Some((i, d));
                        }
                    }
                    if let Some((index, _)) = best {
                        self.optimizer_selected_point = Some(index);
                    }
                }
            }
            columns[0].label(format!(
                "{}  |  evals {}  loops {}  current {:.6}",
                objective_label, evaluations, loops, current_value
            ));
            columns[0].label(format!(
                "{} sims/eval. One loop = one full neighbor sweep before the optimizer moves to a new skill baseline.",
                self.cfg.optimizer_runs_per_eval.max(1)
            ));
            if optimizer_finished && columns[0].button("Apply Best Skills").clicked() {
                apply_best_clicked = true;
            }

            let selected_index = self
                .optimizer_selected_point
                .unwrap_or_else(|| history.len().saturating_sub(1));
            let selected = &history[selected_index];
            let mut details = format_optimizer_point(selected, &self.cfg);
            columns[1].label("Selected point");
            columns[1].add(
                egui::TextEdit::multiline(&mut details)
                    .desired_rows(12)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace)
                    .interactive(false),
            );
        });
        if apply_best_clicked {
            self.apply_best_optimizer_skills();
        }
    }

    fn stat_cap(&self, skill: &str) -> u32 {
        if skill == "corruption" && self.cfg.ascension < 2 {
            return 0;
        }
        base_stat_cap(skill)
            + 5 * u32::from(
                self.cfg
                    .upgrades
                    .get("exp_gain_double")
                    .copied()
                    .unwrap_or(0)
                    > 0,
            )
    }

    fn total_skill_points(&self) -> u32 {
        self.cfg.archaeology_level
            + self
                .cfg
                .upgrades
                .get("asc1_stat_points")
                .copied()
                .unwrap_or(0)
    }

    fn spent_skill_points(&self) -> u32 {
        self.cfg.skills.strength
            + self.cfg.skills.agility
            + self.cfg.skills.perception
            + self.cfg.skills.intellect
            + self.cfg.skills.luck
            + self.cfg.skills.divinity
            + self.cfg.skills.corruption
    }

    fn available_for(&self, current: u32) -> u32 {
        let spent_elsewhere = self.spent_skill_points().saturating_sub(current);
        self.total_skill_points().saturating_sub(spent_elsewhere)
    }

    fn skill_effect_text(&self, skill: &str, level: u32) -> String {
        let upgrade = |id: &str| self.cfg.upgrades.get(id).copied().unwrap_or(0) as f64;
        let lvl = level as f64;
        match skill {
            "strength" => format!(
                "+{:.0} flat dmg, +{:.1}% dmg, +{:.1}% crit dmg",
                lvl + 0.2 * upgrade("strength_skill_buff") * lvl,
                lvl + 0.1 * upgrade("strength_skill_buff") * lvl
                    + upgrade("asc1_strength_skill_buff") * lvl,
                3.0 * lvl + upgrade("asc1_strength_skill_buff") * lvl
            ),
            "agility" => format!(
                "+{:.0} stamina, +{:.2}% speed mod ch, +{:.2}% crit",
                5.0 * lvl + upgrade("agility_skill_buff") * lvl,
                0.2 * lvl + 0.02 * upgrade("agility_skill_buff") * lvl,
                lvl
            ),
            "perception" => format!(
                "+{:.0} armor pen, +{:.0}% frags, +{:.2}% loot mod ch",
                2.0 * lvl + upgrade("perception_skill_buff") * lvl,
                4.0 * lvl,
                0.3 * lvl + 0.01 * upgrade("perception_skill_buff") * lvl
            ),
            "intellect" => format!(
                "+{:.0}% xp, +{:.0}% armor pen, +{:.2}% exp mod ch",
                5.0 * lvl + upgrade("intellect_skill_buff") * lvl,
                3.0 * lvl,
                0.3 * lvl
            ),
            "luck" => format!(
                "+{:.0}% crit, +{:.2}% all mod, +{:.1}% gold xhair",
                2.0 * lvl,
                0.2 * lvl,
                0.5 * lvl
            ),
            "divinity" => format!(
                "+{:.0} flat dmg, +{:.0}% super crit, +{:.0}% auto-tap",
                2.0 * lvl,
                2.0 * lvl,
                2.0 * lvl
            ),
            "corruption" => format!(
                "+{:.0}% dmg, +{:.0}% speed-mod atk rate, -{:.0}% max stamina",
                6.0 * lvl,
                3.0 * lvl,
                3.0 * lvl
            ),
            _ => String::new(),
        }
    }

    fn bonus_effect_text(&self, bonus: &str) -> String {
        match bonus {
            "arch_shop_bundle" => {
                if self.cfg.arch_shop_bundle {
                    "1.25x fragment multiplier".to_string()
                } else {
                    "inactive".to_string()
                }
            }
            "arch_asc_shop_bundle" => {
                if self.cfg.arch_asc_shop_bundle {
                    "+1.15x xp, +5.00% auto-tap, +2.00% loot mod, +2.00% gold xhair".to_string()
                } else {
                    "inactive".to_string()
                }
            }
            "block_bonker" => {
                if self.cfg.block_bonker {
                    let stage = self.cfg.highest_stage_reached.min(100);
                    format!(
                        "stage {} => +{}% dmg, +{}% max stamina, +15 speed mod gain",
                        stage, stage, stage
                    )
                } else {
                    "inactive".to_string()
                }
            }
            "avada_keda" => {
                if self.cfg.avada_keda {
                    "+5 attacks ability duration, -10s cooldowns, +3.00% instacharge".to_string()
                } else {
                    "inactive".to_string()
                }
            }
            "axolotl_pet_quest_unlocked" => {
                if self.cfg.axolotl_pet_quest_unlocked {
                    let pct = 3.0 + 3.0 * self.cfg.axolotl_pet_level.min(11) as f64;
                    format!("{:.2}% fragment bonus in multiplier bin", pct)
                } else {
                    "inactive".to_string()
                }
            }
            "glimmering_geoduck" => {
                if self.cfg.glimmering_geoduck {
                    let uncapped = 0.25 * self.cfg.mythic_chests_owned as f64;
                    let pct = match self.cfg.ascension {
                        0 => uncapped,
                        1 => uncapped.min(50.0),
                        _ => uncapped.min(75.0),
                    };
                    format!(
                        "{:.2}% fragment bonus from {} chests",
                        pct, self.cfg.mythic_chests_owned
                    )
                } else {
                    "inactive".to_string()
                }
            }
            "mythic_chests_owned" => {
                let uncapped = 0.25 * self.cfg.mythic_chests_owned as f64;
                let pct = match self.cfg.ascension {
                    0 => uncapped,
                    1 => uncapped.min(50.0),
                    _ => uncapped.min(75.0),
                };
                format!("Geoduck contribution: +{:.2}%", pct)
            }
            "axolotl_pet_level" => {
                let pct = if self.cfg.axolotl_pet_quest_unlocked {
                    3.0 + 3.0 * self.cfg.axolotl_pet_level.min(11) as f64
                } else {
                    0.0
                };
                format!("Axolotl multiplier bonus: +{:.2}%", pct)
            }
            "hestia_idol_level" => {
                let pct = 0.01 * self.cfg.hestia_idol_level.min(3000) as f64;
                format!("Hestia additive fragment bonus: +{:.2}%", pct)
            }
            _ => String::new(),
        }
    }

    fn upgrade_max_level(&self, id: &str) -> Option<u32> {
        explicit_upgrade_max_level(id).or_else(|| {
            self.upgrade_catalog
                .tables
                .get(id)
                .map(|table| table.costs.len() as u32)
        })
    }
}

fn format_optimizer_point(point: &OptimizerHistoryPoint, cfg: &Config) -> String {
    let objective_label = match cfg.objective {
        Objective::Fragments => match cfg.fragment_objective_currency {
            Some(currency) => format!("{currency} fragments/sec"),
            None => "total fragments/sec".to_string(),
        },
        Objective::Experience => "xp/sec".to_string(),
        Objective::MaxLevel => "max level score".to_string(),
    };
    format!(
        "Eval {}\nLoop {}\nTarget: {}\nObjective: {:.6}\nXP/s: {:.6}\nFragments/s: {:.6}\nMax level score: {:.6}\nSkills: str {} agi {} per {} int {} luck {} div {} corr {}",
        point.index,
        point.loop_index,
        objective_label,
        point.objective_value,
        point.evaluation.xp_per_second,
        point.evaluation.fragment_total_per_second,
        point.evaluation.max_level_score,
        point.skills.strength,
        point.skills.agility,
        point.skills.perception,
        point.skills.intellect,
        point.skills.luck,
        point.skills.divinity,
        point.skills.corruption
    )
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.batch_sim.is_some() {
            self.step_batch_simulation();
            if self.batch_sim.is_some() {
                ctx.request_repaint();
            }
        }
        if self.optimizer_state.is_some() {
            self.step_optimizer();
            if self.optimizer_state.as_ref().is_some_and(|s| !s.finished) {
                ctx.request_repaint();
            }
        }
        self.cfg.axolotl_pet_level = self.cfg.axolotl_pet_level.min(11);
        self.cfg.hestia_idol_level = self.cfg.hestia_idol_level.min(3000);
        if self.cfg.ascension < 2 {
            self.cfg.skills.corruption = 0;
        }
        self.refresh_verbose_output();
        let live_stats = derive_stats(&self.cfg);
        let mut live_stats_text = format_current_stats(&self.cfg);

        egui::SidePanel::right("live_stats_panel")
            .default_width(640.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Live Stats");
                ui.label("Matches the full stat sheet and updates as you edit the build.");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut live_stats_text)
                            .desired_rows(24)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace)
                            .interactive(false),
                    );
                    if self.cfg.verbose_output {
                        ui.separator();
                        ui.heading("Verbose Stats");
                        ui.label("Derived directly from the sim formulas.");
                        ui.add(
                            egui::TextEdit::multiline(&mut self.verbose_output_text)
                                .desired_rows(28)
                                .desired_width(f32::INFINITY)
                                .font(egui::TextStyle::Monospace)
                                .interactive(false),
                        );
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Idle Obelisk Archaeology");
            ui.horizontal(|ui| {
                ui.label("State file");
                ui.text_edit_singleline(&mut self.state_path);
                if ui.button("Load").clicked() {
                    self.load_state();
                }
                if ui.button("Save").clicked() {
                    self.save_state();
                }
                ui.label("Sims");
                ui.add(
                    egui::DragValue::new(&mut self.sim_count)
                        .speed(1.0)
                        .range(1..=1000),
                );
                if self.batch_sim.is_some() {
                    if ui.button("Stop Sim").clicked() {
                        self.stop_batch_simulation();
                    }
                } else if ui.button("Run Sim").clicked() {
                    self.start_batch_simulation();
                }
                if self.optimizer_state.as_ref().is_some_and(|s| !s.finished) {
                    if ui.button("Stop Optimize").clicked() {
                        self.stop_optimizer();
                    }
                } else if ui.button("Run Optimize").clicked() {
                    self.start_optimizer();
                }
            });
            if !self.status.is_empty() {
                ui.label(&self.status);
            }
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.collapsing("Run Settings", |ui| {
                    egui::Grid::new("run_settings_grid")
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label("Objective");
                            egui::ComboBox::from_id_salt("objective_combo")
                                .selected_text(match self.cfg.objective {
                                    Objective::Fragments => match self.cfg.fragment_objective_currency
                                    {
                                        Some(currency) => {
                                            format!("Fragments ({currency})")
                                        }
                                        None => "Fragments (all)".to_string(),
                                    },
                                    _ => self.cfg.objective.label().to_string(),
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.cfg.objective,
                                        Objective::Fragments,
                                        Objective::Fragments.label(),
                                    );
                                    ui.selectable_value(
                                        &mut self.cfg.objective,
                                        Objective::Experience,
                                        Objective::Experience.label(),
                                    );
                                    ui.selectable_value(
                                        &mut self.cfg.objective,
                                        Objective::MaxLevel,
                                        Objective::MaxLevel.label(),
                                    );
                                });
                            ui.end_row();

                            if self.cfg.objective == Objective::Fragments {
                                ui.label("Fragment target");
                                egui::ComboBox::from_id_salt("fragment_objective_combo")
                                    .selected_text(
                                        self.cfg
                                            .fragment_objective_currency
                                            .map(|currency| currency.to_string())
                                            .unwrap_or_else(|| "all".to_string()),
                                    )
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut self.cfg.fragment_objective_currency,
                                            None,
                                            "all",
                                        );
                                        for currency in [
                                            Currency::Common,
                                            Currency::Rare,
                                            Currency::Epic,
                                            Currency::Legendary,
                                            Currency::Mythic,
                                            Currency::Divine,
                                        ] {
                                            ui.selectable_value(
                                                &mut self.cfg.fragment_objective_currency,
                                                Some(currency),
                                                currency.to_string(),
                                            );
                                        }
                                    });
                                ui.end_row();
                            }

                            ui.label("Stage cap");
                            ui.add(egui::DragValue::new(&mut self.cfg.stage_cap).speed(1.0));
                            ui.end_row();

                            ui.label("Archaeology level");
                            ui.add(
                                egui::DragValue::new(&mut self.cfg.archaeology_level).speed(1.0),
                            );
                            ui.end_row();

                            ui.label("Ascension");
                            ui.add(egui::DragValue::new(&mut self.cfg.ascension).speed(1.0));
                            ui.end_row();

                            ui.label("Highest stage reached");
                            ui.add(
                                egui::DragValue::new(&mut self.cfg.highest_stage_reached)
                                    .speed(1.0),
                            );
                            ui.end_row();

                            ui.label("Base crosshair chance %");
                            ui.add(
                                egui::DragValue::new(&mut self.cfg.base_crosshair_chance)
                                    .speed(0.1)
                                    .range(0.0..=100.0),
                            );
                            ui.end_row();

                            ui.label("Print 2/sec attack speed");
                            ui.checkbox(
                                &mut self.cfg.sheet_speed_mod_always_active,
                                "Speed mod effectively permanent",
                            );
                            ui.end_row();

                            ui.label("Verbose sim output");
                            if ui
                                .checkbox(&mut self.cfg.verbose_output, "Show diagnostics")
                                .changed()
                            {
                                self.refresh_verbose_output();
                            }
                            ui.end_row();

                            ui.label("Crosshair summary");
                            ui.vertical(|ui| {
                                ui.label(format!(
                                    "gold {:.2}%  auto-tap {:.2}%  bonus hits/atk {:.4}",
                                    live_stats.gold_crosshair_chance,
                                    live_stats.crosshair_auto_tap_chance,
                                    live_stats.crosshair_bonus_hits_per_attack
                                ));
                            });
                            ui.end_row();

                            ui.label("Top recommendations");
                            ui.add(egui::DragValue::new(&mut self.cfg.top).speed(1.0));
                            ui.end_row();

                            ui.label("Optimize skills");
                            ui.checkbox(
                                &mut self.cfg.optimize_skills,
                                "Search skill allocations too",
                            );
                            ui.end_row();

                            ui.label("Optimizer runs/eval");
                            ui.add(
                                egui::DragValue::new(&mut self.cfg.optimizer_runs_per_eval)
                                    .speed(1.0)
                                    .range(1..=200),
                            );
                            ui.end_row();
                        });
                });

                ui.collapsing("Optimizer Debug", |ui| {
                    egui::Grid::new("optimizer_debug_grid")
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label("Convergence %");
                            ui.add(
                                egui::DragValue::new(&mut self.cfg.optimizer_convergence_pct)
                                    .speed(0.01)
                                    .range(0.0..=100.0),
                            );
                            ui.end_row();

                            ui.label("Max optimizer loops");
                            ui.add(
                                egui::DragValue::new(&mut self.cfg.optimizer_max_loops)
                                    .speed(1.0)
                                    .range(1..=200),
                            );
                            ui.end_row();
                        });
                });

                ui.collapsing("Skills", |ui| {
                    let total_points = self.total_skill_points();
                    let spent_points = self.spent_skill_points();
                    ui.label(format!("Skill points: {spent_points}/{total_points} spent"));
                    egui::Grid::new("skills_grid")
                        .num_columns(3)
                        .show(ui, |ui| {
                            let strength_cap = self.stat_cap("strength");
                            let strength_current = self.cfg.skills.strength;
                            let strength_max =
                                strength_cap.min(self.available_for(strength_current));
                            ui.label(format!("Strength / {strength_cap}"));
                            ui_u32_stepper(
                                ui,
                                &mut self.cfg.skills.strength,
                                0,
                                Some(strength_max),
                            );
                            ui.label(self.skill_effect_text("strength", self.cfg.skills.strength));
                            ui.end_row();
                            let agility_cap = self.stat_cap("agility");
                            let agility_current = self.cfg.skills.agility;
                            let agility_max = agility_cap.min(self.available_for(agility_current));
                            ui.label(format!("Agility / {agility_cap}"));
                            ui_u32_stepper(ui, &mut self.cfg.skills.agility, 0, Some(agility_max));
                            ui.label(self.skill_effect_text("agility", self.cfg.skills.agility));
                            ui.end_row();
                            let perception_cap = self.stat_cap("perception");
                            let perception_current = self.cfg.skills.perception;
                            let perception_max =
                                perception_cap.min(self.available_for(perception_current));
                            ui.label(format!("Perception / {perception_cap}"));
                            ui_u32_stepper(
                                ui,
                                &mut self.cfg.skills.perception,
                                0,
                                Some(perception_max),
                            );
                            ui.label(
                                self.skill_effect_text("perception", self.cfg.skills.perception),
                            );
                            ui.end_row();
                            let intellect_cap = self.stat_cap("intellect");
                            let intellect_current = self.cfg.skills.intellect;
                            let intellect_max =
                                intellect_cap.min(self.available_for(intellect_current));
                            ui.label(format!("Intellect / {intellect_cap}"));
                            ui_u32_stepper(
                                ui,
                                &mut self.cfg.skills.intellect,
                                0,
                                Some(intellect_max),
                            );
                            ui.label(
                                self.skill_effect_text("intellect", self.cfg.skills.intellect),
                            );
                            ui.end_row();
                            let luck_cap = self.stat_cap("luck");
                            let luck_current = self.cfg.skills.luck;
                            let luck_max = luck_cap.min(self.available_for(luck_current));
                            ui.label(format!("Luck / {luck_cap}"));
                            ui_u32_stepper(ui, &mut self.cfg.skills.luck, 0, Some(luck_max));
                            ui.label(self.skill_effect_text("luck", self.cfg.skills.luck));
                            ui.end_row();
                            let divinity_cap = self.stat_cap("divinity");
                            let divinity_current = self.cfg.skills.divinity;
                            let divinity_max =
                                divinity_cap.min(self.available_for(divinity_current));
                            ui.label(format!("Divinity / {divinity_cap}"));
                            ui_u32_stepper(
                                ui,
                                &mut self.cfg.skills.divinity,
                                0,
                                Some(divinity_max),
                            );
                            ui.label(self.skill_effect_text("divinity", self.cfg.skills.divinity));
                            ui.end_row();
                            let corruption_cap = self.stat_cap("corruption");
                            let corruption_current = self.cfg.skills.corruption;
                            let corruption_max =
                                corruption_cap.min(self.available_for(corruption_current));
                            ui.label(format!("Corruption / {corruption_cap}"));
                            ui.add_enabled_ui(self.cfg.ascension >= 2, |ui| {
                                ui_u32_stepper(
                                    ui,
                                    &mut self.cfg.skills.corruption,
                                    0,
                                    Some(corruption_max),
                                );
                            });
                            if self.cfg.ascension < 2 {
                                ui.label("Locked until Asc 2");
                            } else {
                                ui.label(
                                    self.skill_effect_text(
                                        "corruption",
                                        self.cfg.skills.corruption,
                                    ),
                                );
                            }
                            ui.end_row();
                        });
                });

                ui.collapsing("Bonuses", |ui| {
                    egui::Grid::new("bonuses_grid")
                        .num_columns(3)
                        .show(ui, |ui| {
                            ui.label("Arch Shop Bundle");
                            ui.checkbox(&mut self.cfg.arch_shop_bundle, "");
                            ui.label(self.bonus_effect_text("arch_shop_bundle"));
                            ui.end_row();

                            ui.label("Arch Asc Shop Bundle");
                            ui.checkbox(&mut self.cfg.arch_asc_shop_bundle, "");
                            ui.label(self.bonus_effect_text("arch_asc_shop_bundle"));
                            ui.end_row();

                            ui.label("Block Bonker");
                            ui.checkbox(&mut self.cfg.block_bonker, "");
                            ui.label(self.bonus_effect_text("block_bonker"));
                            ui.end_row();

                            ui.label("Avada Keda");
                            ui.checkbox(&mut self.cfg.avada_keda, "");
                            ui.label(self.bonus_effect_text("avada_keda"));
                            ui.end_row();

                            ui.label("Axolotl pet quest");
                            ui.checkbox(&mut self.cfg.axolotl_pet_quest_unlocked, "");
                            ui.label(self.bonus_effect_text("axolotl_pet_quest_unlocked"));
                            ui.end_row();

                            ui.label("Glimmering Geoduck");
                            ui.checkbox(&mut self.cfg.glimmering_geoduck, "");
                            ui.label(self.bonus_effect_text("glimmering_geoduck"));
                            ui.end_row();

                            ui.label("Mythic chests owned");
                            ui.add(
                                egui::DragValue::new(&mut self.cfg.mythic_chests_owned).speed(1.0),
                            );
                            ui.label(self.bonus_effect_text("mythic_chests_owned"));
                            ui.end_row();

                            ui.label("Axolotl pet level");
                            ui.add(
                                egui::DragValue::new(&mut self.cfg.axolotl_pet_level).speed(1.0),
                            );
                            ui.label(self.bonus_effect_text("axolotl_pet_level"));
                            ui.end_row();

                            ui.label("Hestia idol level");
                            ui.add(
                                egui::DragValue::new(&mut self.cfg.hestia_idol_level).speed(1.0),
                            );
                            ui.label(self.bonus_effect_text("hestia_idol_level"));
                            ui.end_row();
                        });
                });

                ui.collapsing("Balances", |ui| {
                    egui::Grid::new("balances_grid")
                        .num_columns(2)
                        .show(ui, |ui| {
                            for currency in ALL_CURRENCIES {
                                let value = self.cfg.balances.entry(currency).or_insert(0.0);
                                ui.label(currency.to_string());
                                ui.add(egui::DragValue::new(value).speed(1.0));
                                ui.end_row();
                            }
                        });
                });

                ui.collapsing("Upgrades", |ui| {
                    egui::Grid::new("upgrades_grid")
                        .num_columns(3)
                        .striped(true)
                        .show(ui, |ui| {
                            for spec in UPGRADE_SPECS {
                                let max_level = self.upgrade_max_level(spec.id);
                                let level =
                                    self.cfg.upgrades.entry(spec.id.to_string()).or_insert(0);
                                ui.label(upgrade_gui_label(*spec, *level));
                                ui.horizontal(|ui| {
                                    ui_u32_stepper(ui, level, 0, max_level);
                                    if ui.button("0").clicked() {
                                        *level = 0;
                                    }
                                    let max_label = match max_level {
                                        Some(max) => format!("Max {max}"),
                                        None => "Max".to_string(),
                                    };
                                    if ui
                                        .add_enabled(
                                            max_level.is_some(),
                                            egui::Button::new(max_label),
                                        )
                                        .clicked()
                                    {
                                        if let Some(max) = max_level {
                                            *level = max;
                                        }
                                    }
                                });
                                ui.label(upgrade_total_effect(*spec, *level));
                                ui.end_row();
                            }
                        });
                });

                ui.collapsing("Cards", |ui| {
                    let active_cards = self.cfg.cards.len();
                    let poly_cards = self
                        .cfg
                        .cards
                        .values()
                        .filter(|quality| {
                            matches!(quality, CardQuality::Polychrome | CardQuality::Infernal)
                        })
                        .count();
                    ui.label(format!(
                        "Cards: {active_cards} active, {poly_cards} polychrome/infernal"
                    ));
                    egui::Grid::new("cards_grid").num_columns(2).show(ui, |ui| {
                        for rarity in CARD_RARITIES {
                            for tier in 1..=4u8 {
                                let key = CardKey { rarity, tier };
                                let mut quality = self
                                    .cfg
                                    .cards
                                    .get(&key)
                                    .copied()
                                    .unwrap_or(CardQuality::None);
                                ui.label(format!("{}.{}", rarity.name(), tier));
                                egui::ComboBox::from_id_salt(format!(
                                    "card_{}_{}",
                                    rarity.name(),
                                    tier
                                ))
                                .selected_text(quality.as_str())
                                .show_ui(ui, |ui| {
                                    for option in CARD_QUALITIES {
                                        ui.selectable_value(&mut quality, option, option.as_str());
                                    }
                                });
                                if quality == CardQuality::None {
                                    self.cfg.cards.remove(&key);
                                } else {
                                    self.cfg.cards.insert(key, quality);
                                }
                                ui.end_row();
                            }
                        }
                    });
                });

                ui.separator();
                self.draw_optimizer_graph(ui);
                if self.optimizer_state.is_some() || !self.optimizer_last_summary.is_empty() {
                    ui.separator();
                }
                ui.heading("Output");
                ui.add(
                    egui::TextEdit::multiline(&mut self.output)
                        .desired_rows(24)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace)
                        .interactive(false),
                );
            });
        });
    }
}

pub(crate) fn launch_gui(cfg: Config) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1800.0, 900.0]),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "Idle Obelisk Archaeology",
        options,
        Box::new(move |_cc| Ok(Box::new(GuiApp::new(cfg)))),
    )
    .map_err(|e| format!("failed to launch GUI: {e}"))
}
