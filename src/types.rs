use std::collections::HashMap;

use crate::game_data::{CardKey, CardQuality, Currency, Rarity};
use crate::state::SkillAllocation;

#[derive(Clone)]
pub(crate) struct UpgradeCostTable {
    pub(crate) currency: Currency,
    pub(crate) costs: Vec<f64>,
}

#[derive(Default)]
pub(crate) struct UpgradeCatalog {
    pub(crate) tables: HashMap<String, UpgradeCostTable>,
}

#[derive(Default, Clone)]
pub(crate) struct StatBreakdown {
    pub(crate) sections: Vec<StatBreakdownSection>,
}

#[derive(Default, Clone)]
pub(crate) struct StatBreakdownSection {
    pub(crate) title: String,
    pub(crate) lines: Vec<StatBreakdownLine>,
}

#[derive(Clone)]
pub(crate) struct StatBreakdownLine {
    pub(crate) label: String,
    pub(crate) value: String,
}

#[derive(Default, Clone)]
pub(crate) struct DerivedStats {
    pub(crate) flat_damage: f64,
    pub(crate) damage_pct: f64,
    pub(crate) crit_chance: f64,
    pub(crate) crit_damage_pct: f64,
    pub(crate) super_crit_chance: f64,
    pub(crate) super_crit_damage_pct: f64,
    pub(crate) ultra_crit_chance: f64,
    pub(crate) ultra_crit_damage_pct: f64,
    pub(crate) armor_pen_flat: f64,
    pub(crate) armor_pen_pct: f64,
    pub(crate) fragment_gain_pct: f64,
    pub(crate) fragment_gain_mult: f64,
    pub(crate) exp_gain_pct: f64,
    pub(crate) exp_gain_mult: f64,
    pub(crate) max_stamina: f64,
    pub(crate) speed_mod_chance: f64,
    pub(crate) stamina_mod_chance: f64,
    pub(crate) stamina_mod_gain: f64,
    #[allow(dead_code)]
    pub(crate) speed_mod_gain_bonus: f64,
    pub(crate) loot_mod_chance: f64,
    pub(crate) exp_mod_chance: f64,
    pub(crate) loot_mod_avg: f64,
    pub(crate) exp_mod_avg: f64,
    pub(crate) passive_attack_rate: f64,
    pub(crate) cards: HashMap<CardKey, CardQuality>,
    pub(crate) unlocked_abilities: u32,
    pub(crate) flurry_casts_per_second: f64,
    pub(crate) quake_effective_bonus: f64,
    pub(crate) enrage_attack_count: u32,
    pub(crate) flurry_attack_count: u32,
    pub(crate) quake_attack_count: u32,
    pub(crate) enrage_damage_bonus_pct: f64,
    pub(crate) enrage_crit_damage_bonus_pct: f64,
    pub(crate) enrage_cooldown: f64,
    pub(crate) flurry_cooldown: f64,
    pub(crate) quake_cooldown: f64,
    pub(crate) flurry_stamina_on_cast: f64,
    pub(crate) ability_instacharge_chance: f64,
    pub(crate) crosshair_spawn_chance: f64,
    pub(crate) gold_crosshair_chance: f64,
    pub(crate) crosshair_auto_tap_chance: f64,
    pub(crate) crosshair_bonus_hits_per_attack: f64,
    pub(crate) archaeology_poly_card_bonus: bool,
    pub(crate) stamina_additive_base: f64,
    pub(crate) stamina_agility_bonus: f64,
    pub(crate) stamina_upgrade_bonus: f64,
    pub(crate) stamina_skill_bonus: f64,
    pub(crate) stamina_block_bonker_pct: f64,
    pub(crate) stamina_exp_legendary_pct: f64,
    pub(crate) stamina_asc1_autotap_pct: f64,
    pub(crate) stamina_corruption_pct: f64,
    pub(crate) breakdown: StatBreakdown,
}

#[derive(Default, Clone)]
pub(crate) struct SimulationResult {
    pub(crate) run_time: f64,
    pub(crate) attacks_per_second: f64,
    pub(crate) avg_damage_per_hit: f64,
    pub(crate) max_stamina: f64,
    pub(crate) starting_stamina: f64,
    pub(crate) ending_stamina: f64,
    pub(crate) enrage_uptime: f64,
    pub(crate) flurry_uptime: f64,
    pub(crate) quake_uptime: f64,
    pub(crate) total_attacks: u64,
    pub(crate) enrage_casts: u32,
    pub(crate) flurry_casts: u32,
    pub(crate) quake_casts: u32,
    pub(crate) full_stages_cleared: u32,
    pub(crate) max_stage_reached: u32,
    pub(crate) previous_highest_stage: u32,
    pub(crate) updated_highest_stage: u32,
    pub(crate) ended_at_cap: bool,
    pub(crate) xp_per_run: f64,
    pub(crate) xp_per_second: f64,
    pub(crate) fragment_per_run: HashMap<Currency, f64>,
    pub(crate) fragment_per_second: HashMap<Currency, f64>,
    pub(crate) rarity_totals: Vec<RarityTotal>,
    pub(crate) stage_summaries: Vec<StageSummary>,
}

#[derive(Default, Clone)]
pub(crate) struct RarityTotal {
    pub(crate) rarity: Option<Rarity>,
    pub(crate) blocks_destroyed: f64,
    pub(crate) xp: f64,
    pub(crate) fragments: f64,
}

#[derive(Clone)]
pub(crate) struct StageSummary {
    pub(crate) stage: u32,
    pub(crate) fraction_cleared: f64,
}

#[derive(Clone)]
pub(crate) struct Recommendation {
    pub(crate) display_name: &'static str,
    pub(crate) currency: Currency,
    pub(crate) next_level: u32,
    pub(crate) next_cost: f64,
    pub(crate) delta_value: f64,
    pub(crate) roi: f64,
    pub(crate) time_to_afford: Option<f64>,
}

#[derive(Default, Clone)]
pub(crate) struct OptimizerEvaluation {
    pub(crate) objective_value: f64,
    pub(crate) fragment_per_second: HashMap<Currency, f64>,
    pub(crate) fragment_total_per_second: f64,
    pub(crate) xp_per_second: f64,
    pub(crate) max_level_score: f64,
}

#[derive(Clone)]
pub(crate) struct SkillOptimizationResult {
    pub(crate) initial_skills: SkillAllocation,
    pub(crate) best_skills: SkillAllocation,
    pub(crate) initial_value: f64,
    pub(crate) best_value: f64,
    pub(crate) loops: u32,
    pub(crate) evaluations: u32,
    pub(crate) simulations: u32,
}

#[derive(Clone)]
pub(crate) struct OptimizerHistoryPoint {
    pub(crate) index: u32,
    pub(crate) loop_index: u32,
    pub(crate) objective_value: f64,
    pub(crate) skills: SkillAllocation,
    pub(crate) evaluation: OptimizerEvaluation,
}

#[derive(Clone)]
pub(crate) struct SkillOptimizerState {
    pub(crate) cfg: crate::state::Config,
    pub(crate) current_cfg: crate::state::Config,
    pub(crate) current_eval: OptimizerEvaluation,
    pub(crate) initial_skills: SkillAllocation,
    pub(crate) initial_value: f64,
    pub(crate) loops: u32,
    pub(crate) evaluations: u32,
    pub(crate) pending_neighbors: Vec<crate::state::Config>,
    pub(crate) next_neighbor_index: usize,
    pub(crate) best_neighbor: Option<(crate::state::Config, OptimizerEvaluation)>,
    pub(crate) history: Vec<OptimizerHistoryPoint>,
    pub(crate) stop_requested: bool,
    pub(crate) finished: bool,
}
