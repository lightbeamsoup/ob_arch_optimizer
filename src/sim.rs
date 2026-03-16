use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::game_data::{
    BASE_DAMAGE, BASE_SPEED_MOD_GAIN, BASE_STAMINA, BASE_STAMINA_MOD_GAIN, BLOCK_TIERS,
    BLOCKS_PER_STAGE, BOSS_FLOORS, BlockTier, CardKey, CardQuality, Currency, Rarity, SPAWN_TABLE,
    base_stat_cap,
};
use crate::state::{Config, Objective, SkillAllocation};
use crate::types::{
    DerivedStats, OptimizerEvaluation, OptimizerHistoryPoint, RarityTotal, SimulationResult,
    SkillOptimizationResult, SkillOptimizerState, StageSummary, StatBreakdown, StatBreakdownLine,
    StatBreakdownSection,
};

const OPTIMIZER_BASE_SEED: u64 = 0xC0DE_5EED_1234_5678;

pub(crate) fn block_bonker_stage_bonus_pct(cfg: &Config) -> f64 {
    if cfg.block_bonker {
        cfg.highest_stage_reached.min(100) as f64
    } else {
        0.0
    }
}

pub(crate) fn flat_damage_from_config(cfg: &Config) -> f64 {
    let s = &cfg.skills;
    let level = |id: &str| -> f64 { cfg.upgrades.get(id).copied().unwrap_or(0) as f64 };
    let strength_skill_buff = level("strength_skill_buff");

    BASE_DAMAGE
        + s.strength as f64
        + 2.0 * s.divinity as f64
        + level("flat_damage_common")
        + 2.0 * level("flat_damage_rare")
        + 2.0 * level("flat_damage_super_crit")
        + 3.0 * level("asc1_flat_damage_enrage")
        + 3.0 * level("asc1_flat_damage_ultra")
        + strength_skill_buff * 0.2 * s.strength as f64
}

pub(crate) fn additive_damage_pct_from_config(cfg: &Config) -> f64 {
    let s = &cfg.skills;
    let level = |id: &str| -> f64 { cfg.upgrades.get(id).copied().unwrap_or(0) as f64 };
    let strength_skill_buff = level("strength_skill_buff");
    let asc1_strength_skill_buff = level("asc1_strength_skill_buff");

    s.strength as f64
        + 6.0 * s.corruption as f64
        + strength_skill_buff * 0.1 * s.strength as f64
        + asc1_strength_skill_buff * s.strength as f64
        + 2.0 * level("damage_armor_pen_mythic")
        + 10.0 * level("asc1_damage_exp")
}

pub(crate) fn crit_damage_pct_from_config(cfg: &Config) -> f64 {
    let s = &cfg.skills;
    let level = |id: &str| -> f64 { cfg.upgrades.get(id).copied().unwrap_or(0) as f64 };
    let asc1_strength_skill_buff = level("asc1_strength_skill_buff");

    3.0 * s.strength as f64
        + level("crit_upgrade")
        + 2.0 * level("crit_super_damage")
        + asc1_strength_skill_buff * s.strength as f64
}

pub(crate) fn geoduck_fragment_gain_pct_from_config(cfg: &Config) -> f64 {
    if cfg.glimmering_geoduck {
        let uncapped = 0.25 * cfg.mythic_chests_owned as f64;
        match cfg.ascension {
            0 => uncapped,
            1 => uncapped.min(50.0),
            _ => uncapped.min(75.0),
        }
    } else {
        0.0
    }
}

pub(crate) fn fragment_gain_pct_from_config(cfg: &Config) -> (f64, f64) {
    let s = &cfg.skills;
    let level = |id: &str| -> f64 { cfg.upgrades.get(id).copied().unwrap_or(0) as f64 };
    let hestia_fragment_gain_pct = 0.01 * cfg.hestia_idol_level.min(3000) as f64;
    let fragment_gain_pct = 4.0 * s.perception as f64
        + 2.0 * level("fragment_gain_gems")
        + 2.0 * level("exp_frag_epic")
        + hestia_fragment_gain_pct;
    (fragment_gain_pct, hestia_fragment_gain_pct)
}

pub(crate) fn fragment_gain_mult_from_config(cfg: &Config) -> (f64, f64, f64) {
    let level = |id: &str| -> f64 { cfg.upgrades.get(id).copied().unwrap_or(0) as f64 };
    let geoduck_fragment_gain_pct = geoduck_fragment_gain_pct_from_config(cfg);
    let axolotl_fragment_gain_pct = if cfg.axolotl_pet_quest_unlocked {
        3.0 + 3.0 * cfg.axolotl_pet_level.min(11) as f64
    } else {
        0.0
    };
    let mut fragment_gain_mult = if level("fragment_gain_multiplier") > 0.0 {
        1.25
    } else {
        1.0
    };
    if cfg.arch_shop_bundle {
        fragment_gain_mult *= 1.25;
    }
    fragment_gain_mult *= 1.0 + axolotl_fragment_gain_pct / 100.0;
    fragment_gain_mult *= 1.0 + geoduck_fragment_gain_pct / 100.0;
    (
        fragment_gain_mult,
        axolotl_fragment_gain_pct,
        geoduck_fragment_gain_pct,
    )
}

pub(crate) fn exp_gain_pct_from_config(cfg: &Config) -> f64 {
    let s = &cfg.skills;
    let level = |id: &str| -> f64 { cfg.upgrades.get(id).copied().unwrap_or(0) as f64 };
    let intellect_skill_buff = level("intellect_skill_buff");
    5.0 * s.intellect as f64
        + 5.0 * level("arch_exp_gems")
        + 2.0 * level("arch_exp_common")
        + 3.0 * level("exp_frag_epic")
        + 5.0 * level("exp_stamina_legendary")
        + intellect_skill_buff * s.intellect as f64
        + 10.0 * level("asc1_damage_exp")
}

pub(crate) fn exp_gain_mult_from_config(cfg: &Config) -> f64 {
    let mut mult = if cfg.upgrades.get("exp_gain_double").copied().unwrap_or(0) > 0 {
        2.0
    } else {
        1.0
    };
    if cfg.arch_asc_shop_bundle {
        mult *= 1.15;
    }
    mult
}

pub(crate) fn all_mod_bonus_pct_from_config(cfg: &Config) -> f64 {
    let s = &cfg.skills;
    let level = |id: &str| -> f64 { cfg.upgrades.get(id).copied().unwrap_or(0) as f64 };
    0.2 * s.luck as f64 + 1.5 * level("all_mod_chances") + 0.02 * level("asc1_all_mod")
}

pub(crate) fn loot_mod_chance_from_config(cfg: &Config) -> f64 {
    let s = &cfg.skills;
    let level = |id: &str| -> f64 { cfg.upgrades.get(id).copied().unwrap_or(0) as f64 };
    let perception_skill_buff = level("perception_skill_buff");
    0.3 * s.perception as f64
        + 0.05 * level("fragment_gain_gems")
        + 0.01 * perception_skill_buff * s.perception as f64
        + if cfg.arch_asc_shop_bundle { 2.0 } else { 0.0 }
        + all_mod_bonus_pct_from_config(cfg)
}

pub(crate) fn exp_mod_chance_from_config(cfg: &Config) -> f64 {
    let s = &cfg.skills;
    let level = |id: &str| -> f64 { cfg.upgrades.get(id).copied().unwrap_or(0) as f64 };
    0.3 * s.intellect as f64
        + 0.05 * level("arch_exp_gems")
        + 0.10 * level("exp_mod_gain")
        + all_mod_bonus_pct_from_config(cfg)
}

pub(crate) fn loot_mod_avg_from_config(cfg: &Config) -> f64 {
    let level = |id: &str| -> f64 { cfg.upgrades.get(id).copied().unwrap_or(0) as f64 };
    2.0 + 0.30 * level("loot_mod_gain")
}

pub(crate) fn exp_mod_avg_from_config(cfg: &Config) -> f64 {
    let level = |id: &str| -> f64 { cfg.upgrades.get(id).copied().unwrap_or(0) as f64 };
    3.0 + 0.10 * level("exp_mod_gain") + 0.02 * level("asc1_supercrit_expmod")
}

pub(crate) fn simulate_run(cfg: &Config) -> SimulationResult {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15);
    let mut result = simulate_run_seeded(cfg, seed);
    if cfg.sheet_speed_mod_always_active {
        result.run_time *= 0.5;
        result.attacks_per_second *= 2.0;
        result.xp_per_second *= 2.0;
        for rate in result.fragment_per_second.values_mut() {
            *rate *= 2.0;
        }
    }
    result
}

fn breakdown_section(title: &str, lines: Vec<(String, String)>) -> StatBreakdownSection {
    StatBreakdownSection {
        title: title.to_string(),
        lines: lines
            .into_iter()
            .map(|(label, value)| StatBreakdownLine { label, value })
            .collect(),
    }
}

pub(crate) fn simulate_run_internal(
    cfg: &Config,
    _use_feedback_speed_mod: bool,
) -> SimulationResult {
    simulate_run_seeded(cfg, 0x9E37_79B9_7F4A_7C15)
}

pub(crate) fn evaluate_optimizer(cfg: &Config) -> OptimizerEvaluation {
    let runs = cfg.optimizer_runs_per_eval.max(1);
    let mut eval = OptimizerEvaluation::default();
    for i in 0..runs {
        let result = simulate_run_seeded(
            cfg,
            OPTIMIZER_BASE_SEED.wrapping_add((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)),
        );
        eval.objective_value += optimizer_objective_value(cfg, &result);
        eval.xp_per_second += result.xp_per_second;
        eval.max_level_score += optimizer_objective_value(
            &Config {
                objective: Objective::MaxLevel,
                ..cfg.clone()
            },
            &result,
        );
        for (currency, rate) in result.fragment_per_second {
            *eval.fragment_per_second.entry(currency).or_insert(0.0) += rate;
            eval.fragment_total_per_second += rate;
        }
    }
    eval.objective_value /= runs as f64;
    eval.xp_per_second /= runs as f64;
    eval.max_level_score /= runs as f64;
    eval.fragment_total_per_second /= runs as f64;
    for rate in eval.fragment_per_second.values_mut() {
        *rate /= runs as f64;
    }
    eval
}

pub(crate) fn start_skill_optimizer(cfg: &Config) -> Option<SkillOptimizerState> {
    if !cfg.optimize_skills {
        return None;
    }
    let current_eval = evaluate_optimizer(cfg);
    let mut state = SkillOptimizerState {
        cfg: cfg.clone(),
        current_cfg: cfg.clone(),
        current_eval: current_eval.clone(),
        initial_skills: cfg.skills.clone(),
        initial_value: current_eval.objective_value,
        loops: 0,
        evaluations: 1,
        pending_neighbors: skill_neighbors(cfg),
        next_neighbor_index: 0,
        best_neighbor: None,
        history: Vec::new(),
        stop_requested: false,
        finished: false,
    };
    state.history.push(OptimizerHistoryPoint {
        index: 0,
        loop_index: 0,
        objective_value: current_eval.objective_value,
        skills: cfg.skills.clone(),
        evaluation: current_eval,
    });
    Some(state)
}

pub(crate) fn step_skill_optimizer(state: &mut SkillOptimizerState) {
    if state.finished {
        return;
    }
    if state.loops >= state.cfg.optimizer_max_loops.max(1) {
        state.finished = true;
        return;
    }
    if state.pending_neighbors.is_empty() {
        state.finished = true;
        return;
    }

    if state.next_neighbor_index < state.pending_neighbors.len() {
        let neighbor = state.pending_neighbors[state.next_neighbor_index].clone();
        state.next_neighbor_index += 1;
        let eval = evaluate_optimizer(&neighbor);
        let current_best = state
            .best_neighbor
            .as_ref()
            .map(|(_, best_eval)| best_eval.objective_value)
            .unwrap_or(f64::NEG_INFINITY);
        if eval.objective_value > current_best {
            state.best_neighbor = Some((neighbor.clone(), eval.clone()));
        }
        let point_index = state.history.len() as u32;
        state.history.push(OptimizerHistoryPoint {
            index: point_index,
            loop_index: state.loops + 1,
            objective_value: eval.objective_value,
            skills: neighbor.skills.clone(),
            evaluation: eval,
        });
        state.evaluations += 1;
        return;
    }

    state.loops += 1;
    let threshold = state.cfg.optimizer_convergence_pct.max(0.0) / 100.0;
    let Some((neighbor_cfg, neighbor_eval)) = state.best_neighbor.take() else {
        state.finished = true;
        return;
    };
    let improvement_needed = state.current_eval.objective_value.abs() * threshold;
    if neighbor_eval.objective_value <= state.current_eval.objective_value + improvement_needed {
        state.finished = true;
        return;
    }
    if state.stop_requested {
        state.current_cfg = neighbor_cfg;
        state.current_eval = neighbor_eval;
        state.finished = true;
        return;
    }
    state.current_cfg = neighbor_cfg;
    state.current_eval = neighbor_eval;
    state.pending_neighbors = skill_neighbors(&state.current_cfg);
    state.next_neighbor_index = 0;
}

pub(crate) fn finalize_skill_optimizer(state: &SkillOptimizerState) -> SkillOptimizationResult {
    SkillOptimizationResult {
        initial_skills: state.initial_skills.clone(),
        best_skills: state.current_cfg.skills.clone(),
        initial_value: state.initial_value,
        best_value: state.current_eval.objective_value,
        loops: state.loops,
        evaluations: state.evaluations,
        simulations: state.evaluations * state.cfg.optimizer_runs_per_eval.max(1),
    }
}

pub(crate) fn optimize_skills(cfg: &Config) -> Option<SkillOptimizationResult> {
    let mut state = start_skill_optimizer(cfg)?;
    while !state.finished {
        step_skill_optimizer(&mut state);
    }
    Some(finalize_skill_optimizer(&state))
}

fn optimizer_objective_value(cfg: &Config, result: &SimulationResult) -> f64 {
    match cfg.objective {
        Objective::Fragments => match cfg.fragment_objective_currency {
            Some(currency) => result.fragment_per_second.get(&currency).copied().unwrap_or(0.0),
            None => result.fragment_per_second.values().sum(),
        },
        Objective::Experience => result.xp_per_second,
        Objective::MaxLevel => match result.stage_summaries.last() {
            Some(summary) if summary.fraction_cleared < 0.999_999 => {
                (summary.stage.saturating_sub(1)) as f64 + summary.fraction_cleared
            }
            Some(summary) => summary.stage as f64,
            None => 0.0,
        },
    }
}

fn skill_neighbors(cfg: &Config) -> Vec<Config> {
    let mut out = Vec::new();
    let total_points =
        cfg.archaeology_level + cfg.upgrades.get("asc1_stat_points").copied().unwrap_or(0);
    let spent = skill_spent(&cfg.skills);
    let caps = skill_caps(cfg);

    let skills = [
        ("strength", cfg.skills.strength),
        ("agility", cfg.skills.agility),
        ("perception", cfg.skills.perception),
        ("intellect", cfg.skills.intellect),
        ("luck", cfg.skills.luck),
        ("divinity", cfg.skills.divinity),
        ("corruption", cfg.skills.corruption),
    ];

    if spent < total_points {
        for (name, value) in skills {
            let cap = *caps.get(name).unwrap_or(&0);
            if value < cap {
                let mut next = cfg.clone();
                adjust_skill(&mut next.skills, name, 1);
                out.push(next);
            }
        }
    }

    for (from_name, from_value) in skills {
        if from_value == 0 {
            continue;
        }

        let mut next = cfg.clone();
        adjust_skill(&mut next.skills, from_name, -1);
        out.push(next);

        for (to_name, to_value) in skills {
            if from_name == to_name {
                continue;
            }
            let to_cap = *caps.get(to_name).unwrap_or(&0);
            if to_value >= to_cap {
                continue;
            }
            let mut moved = cfg.clone();
            adjust_skill(&mut moved.skills, from_name, -1);
            adjust_skill(&mut moved.skills, to_name, 1);
            out.push(moved);
        }
    }

    out
}

fn skill_caps(cfg: &Config) -> std::collections::HashMap<&'static str, u32> {
    let mut out = std::collections::HashMap::new();
    let cap_bonus = 5 * u32::from(cfg.upgrades.get("exp_gain_double").copied().unwrap_or(0) > 0);
    for name in [
        "strength",
        "agility",
        "perception",
        "intellect",
        "luck",
        "divinity",
        "corruption",
    ] {
        let cap = if name == "corruption" && cfg.ascension < 2 {
            0
        } else {
            base_stat_cap(name) + cap_bonus
        };
        out.insert(name, cap);
    }
    out
}

fn skill_spent(skills: &SkillAllocation) -> u32 {
    skills.strength
        + skills.agility
        + skills.perception
        + skills.intellect
        + skills.luck
        + skills.divinity
        + skills.corruption
}

fn adjust_skill(skills: &mut SkillAllocation, name: &str, delta: i32) {
    let slot = match name {
        "strength" => &mut skills.strength,
        "agility" => &mut skills.agility,
        "perception" => &mut skills.perception,
        "intellect" => &mut skills.intellect,
        "luck" => &mut skills.luck,
        "divinity" => &mut skills.divinity,
        "corruption" => &mut skills.corruption,
        _ => return,
    };
    if delta >= 0 {
        *slot += delta as u32;
    } else {
        *slot = slot.saturating_sub((-delta) as u32);
    }
}

fn simulate_run_seeded(cfg: &Config, seed: u64) -> SimulationResult {
    let stats = derive_stats(cfg);
    let mut result = SimulationResult {
        max_stamina: stats.max_stamina,
        starting_stamina: stats.max_stamina,
        ending_stamina: stats.max_stamina,
        previous_highest_stage: cfg.highest_stage_reached,
        updated_highest_stage: cfg.highest_stage_reached,
        rarity_totals: vec![
            RarityTotal {
                rarity: Some(Rarity::Dirt),
                ..RarityTotal::default()
            },
            RarityTotal {
                rarity: Some(Rarity::Common),
                ..RarityTotal::default()
            },
            RarityTotal {
                rarity: Some(Rarity::Rare),
                ..RarityTotal::default()
            },
            RarityTotal {
                rarity: Some(Rarity::Epic),
                ..RarityTotal::default()
            },
            RarityTotal {
                rarity: Some(Rarity::Legendary),
                ..RarityTotal::default()
            },
            RarityTotal {
                rarity: Some(Rarity::Mythic),
                ..RarityTotal::default()
            },
            RarityTotal {
                rarity: Some(Rarity::Divine),
                ..RarityTotal::default()
            },
        ],
        ..SimulationResult::default()
    };

    let passive_attack_rate = stats.passive_attack_rate.max(0.10);
    let mut stamina = stats.max_stamina;
    let mut now = 0.0;
    let mut total_damage_done = 0.0;
    let mut rng = SimRng::new(seed);
    let mut enrage = AbilityTracker::default();
    let mut flurry = AbilityTracker::default();
    let mut quake = AbilityTracker::default();

    for stage in 1..=cfg.stage_cap {
        let mut blocks = concrete_stage_blocks(stage, &stats, &mut rng);
        let stage_block_count = blocks.len() as f64;
        let mut destroyed_blocks = 0.0;
        let mut stage_xp = 0.0;
        let mut stage_fragments: HashMap<Currency, f64> = HashMap::new();

        while !blocks.is_empty() && stamina >= 1.0 {
            autocast_abilities(
                &stats,
                &mut stamina,
                &mut now,
                &mut enrage,
                &mut flurry,
                &mut quake,
            );

            let flurry_active = flurry.attacks_remaining > 0;
            let enrage_active = enrage.attacks_remaining > 0;
            let quake_active = quake.attacks_remaining > 0;
            let attack_rate = passive_attack_rate * if flurry_active { 2.0 } else { 1.0 };
            let attack_interval = 1.0 / attack_rate.max(0.10);
            stamina -= 1.0;
            now += attack_interval;
            result.total_attacks += 1;
            if enrage_active {
                enrage.attacks_while_active += 1;
            }
            if flurry_active {
                flurry.attacks_while_active += 1;
            }
            if quake_active {
                quake.attacks_while_active += 1;
            }

            let main_hit_damage = sample_hit_damage(&stats, enrage_active, &mut rng);
            let direct_damage =
                direct_attack_damage_from_roll(&stats, main_hit_damage, enrage_active, &mut rng);
            let quake_damage = if quake_active {
                0.20 * main_hit_damage
            } else {
                0.0
            };

            apply_attack_to_stage(
                &mut blocks,
                direct_damage,
                quake_damage,
                &stats,
                &mut stamina,
                &mut stage_xp,
                &mut stage_fragments,
                &mut result.rarity_totals,
                &mut destroyed_blocks,
                &mut total_damage_done,
                &mut rng,
            );

            for tracker in [&mut enrage, &mut flurry, &mut quake] {
                if tracker.attacks_remaining > 0 {
                    tracker.attacks_remaining -= 1;
                }
            }
        }

        let fraction_cleared = if stage_block_count > 0.0 {
            (destroyed_blocks / stage_block_count).clamp(0.0, 1.0)
        } else {
            1.0
        };

        result.run_time = now;
        result.xp_per_run += stage_xp;
        for (currency, amount) in stage_fragments {
            *result.fragment_per_run.entry(currency).or_insert(0.0) += amount;
        }
        result.stage_summaries.push(StageSummary {
            stage,
            fraction_cleared,
        });

        if fraction_cleared >= 0.999_999 {
            result.full_stages_cleared += 1;
            result.max_stage_reached = stage;
            result.updated_highest_stage = result.updated_highest_stage.max(stage);
        } else {
            result.max_stage_reached = stage.saturating_sub(1) + u32::from(fraction_cleared > 0.0);
            if fraction_cleared > 0.0 {
                result.updated_highest_stage = result.updated_highest_stage.max(stage);
            }
            stamina = stamina.max(0.0);
            break;
        }
    }

    result.ending_stamina = stamina.max(0.0).min(stats.max_stamina);
    if result.max_stage_reached == cfg.stage_cap && result.full_stages_cleared == cfg.stage_cap {
        result.ended_at_cap = true;
    }
    if result.run_time > 0.0 {
        result.attacks_per_second = result.total_attacks as f64 / result.run_time;
        result.avg_damage_per_hit = total_damage_done / result.total_attacks.max(1) as f64;
        result.xp_per_second = result.xp_per_run / result.run_time;
        for (currency, amount) in result.fragment_per_run.clone() {
            result
                .fragment_per_second
                .insert(currency, amount / result.run_time);
        }
        result.enrage_uptime =
            enrage.attacks_while_active as f64 / result.total_attacks.max(1) as f64;
        result.flurry_uptime =
            flurry.attacks_while_active as f64 / result.total_attacks.max(1) as f64;
        result.quake_uptime =
            quake.attacks_while_active as f64 / result.total_attacks.max(1) as f64;
    }
    result.enrage_casts = enrage.casts;
    result.flurry_casts = flurry.casts;
    result.quake_casts = quake.casts;
    result
}

pub(crate) fn derive_stats(cfg: &Config) -> DerivedStats {
    let s = &cfg.skills;
    let level = |id: &str| -> f64 { cfg.upgrades.get(id).copied().unwrap_or(0) as f64 };
    let block_bonker_stage = block_bonker_stage_bonus_pct(cfg);

    let agility_skill_buff = level("agility_skill_buff");
    let perception_skill_buff = level("perception_skill_buff");
    let unlocked_abilities = level("unlock_ability") as u32;

    let flat_damage = flat_damage_from_config(cfg);

    let damage_pct = additive_damage_pct_from_config(cfg) + block_bonker_stage;

    let crit_chance = s.agility as f64 + 2.0 * s.luck as f64 + 0.25 * level("crit_upgrade");
    let crit_damage_pct = crit_damage_pct_from_config(cfg);

    let super_crit_chance = 0.35 * level("flat_damage_super_crit")
        + 0.35 * level("super_ultra_crit")
        + 2.0 * s.divinity as f64;
    let super_crit_damage_pct =
        2.0 * level("crit_super_damage") + 0.50 * level("asc1_supercrit_expmod");
    let ultra_crit_chance = level("super_ultra_crit") + 0.50 * level("asc1_flat_damage_ultra");
    let ultra_crit_damage_pct = 1.0 * level("asc1_ultra_damage");

    let armor_pen_flat = 2.0 * s.perception as f64
        + level("armor_pen_common")
        + 3.0 * level("damage_armor_pen_mythic")
        + 3.0 * level("asc1_crosshair_armor_pen")
        + perception_skill_buff * s.perception as f64;
    let armor_pen_pct = 3.0 * s.intellect as f64 + 2.0 * level("armor_pen_pct_cdr");

    let (fragment_gain_pct, _hestia_fragment_gain_pct) = fragment_gain_pct_from_config(cfg);
    let (fragment_gain_mult, _axolotl_fragment_gain_pct, _geoduck_fragment_gain_pct) =
        fragment_gain_mult_from_config(cfg);

    let exp_gain_pct = exp_gain_pct_from_config(cfg);
    let exp_gain_mult = exp_gain_mult_from_config(cfg);

    let all_mod = all_mod_bonus_pct_from_config(cfg);

    let stamina_additive_base = BASE_STAMINA;
    let stamina_agility_bonus = 5.0 * s.agility as f64;
    let stamina_upgrade_bonus = 2.0 * level("max_stamina_gems")
        + 2.0 * level("max_stamina_rare")
        + 4.0 * level("max_stamina_epic")
        + 4.0 * level("instacharge_stamina");
    let stamina_skill_bonus = agility_skill_buff * s.agility as f64;
    let additive_stamina =
        stamina_additive_base + stamina_agility_bonus + stamina_upgrade_bonus + stamina_skill_bonus;

    let stamina_block_bonker_pct = block_bonker_stage;
    let stamina_exp_legendary_pct = level("exp_stamina_legendary");
    let stamina_asc1_autotap_pct = 0.5 * level("asc1_max_stam_autotap");
    let stamina_corruption_pct = 3.0 * s.corruption as f64;
    let max_stamina_mult = ((1.0 + stamina_block_bonker_pct / 100.0)
        * (1.0 + stamina_exp_legendary_pct / 100.0 + stamina_asc1_autotap_pct / 100.0
            - stamina_corruption_pct / 100.0))
        .max(0.10);
    let max_stamina = additive_stamina * max_stamina_mult;

    let speed_mod_chance =
        0.2 * s.agility as f64 + all_mod + 0.02 * agility_skill_buff * s.agility as f64;
    let stamina_mod_chance = 0.05 * level("max_stamina_gems")
        + 0.05 * level("max_stamina_rare")
        + 0.03 * level("asc1_ultra_damage")
        + 0.10 * level("asc1_instacharge_stamina")
        + all_mod;
    let stamina_mod_gain =
        BASE_STAMINA_MOD_GAIN + level("max_stamina_epic") + 2.0 * level("stamina_mod_gain_once");
    let speed_mod_gain_bonus = BASE_SPEED_MOD_GAIN + if cfg.block_bonker { 15.0 } else { 0.0 };
    let loot_mod_chance = loot_mod_chance_from_config(cfg);
    let exp_mod_chance = exp_mod_chance_from_config(cfg);

    let loot_mod_avg = loot_mod_avg_from_config(cfg);
    let exp_mod_avg = exp_mod_avg_from_config(cfg);

    let ability_instacharge_chance = 0.30 * level("instacharge_stamina")
        + 0.10 * level("asc1_instacharge_stamina")
        + if cfg.avada_keda { 3.0 } else { 0.0 };
    let crosshair_spawn_chance = cfg.base_crosshair_chance.clamp(0.0, 100.0);
    let gold_crosshair_chance = (0.5 * s.luck as f64
        + level("asc1_gold_crosshair")
        + if cfg.arch_asc_shop_bundle { 2.0 } else { 0.0 })
    .clamp(0.0, 100.0);
    let crosshair_auto_tap_chance = (2.0 * s.divinity as f64
        + level("asc1_gold_crosshair")
        + if cfg.arch_asc_shop_bundle { 5.0 } else { 0.0 }
        + 0.20 * level("asc1_max_stam_autotap"))
    .clamp(0.0, 100.0);
    let crosshair_bonus_hits_per_attack =
        (crosshair_spawn_chance / 100.0) * (crosshair_auto_tap_chance / 100.0);
    let cooldown_scale = 1.0 / (1.0 - (ability_instacharge_chance / 100.0).min(0.75));
    let avada_keda_cdr = if cfg.avada_keda { 10.0 } else { 0.0 };
    let avada_keda_duration_bonus = if cfg.avada_keda { 5.0 } else { 0.0 };
    let enrage_attack_count = if unlocked_abilities >= 1 {
        (5.0 + avada_keda_duration_bonus) as u32
    } else {
        0
    };
    let flurry_attack_count = if unlocked_abilities >= 2 {
        (5.0 + avada_keda_duration_bonus) as u32
    } else {
        0
    };
    let quake_attack_count = if unlocked_abilities >= 3 {
        (5.0 + level("quake_upgrade") + avada_keda_duration_bonus) as u32
    } else {
        0
    };
    let enrage_cooldown = ((60.0
        - level("enrage_upgrade")
        - level("armor_pen_pct_cdr")
        - level("asc1_flat_damage_enrage")
        - avada_keda_cdr)
        .max(1.0))
        / cooldown_scale;
    let flurry_cooldown =
        ((120.0 - level("flurry_upgrade") - level("armor_pen_pct_cdr") - avada_keda_cdr).max(1.0))
            / cooldown_scale;
    let quake_cooldown =
        ((180.0 - 2.0 * level("quake_upgrade") - level("armor_pen_pct_cdr") - avada_keda_cdr)
            .max(1.0))
            / cooldown_scale;
    let enrage_damage_bonus_pct = if unlocked_abilities >= 1 {
        20.0 + 2.0 * level("enrage_upgrade")
    } else {
        0.0
    };
    let enrage_crit_damage_bonus_pct = if unlocked_abilities >= 1 {
        100.0 + 2.0 * level("enrage_upgrade")
    } else {
        0.0
    };
    let flurry_stamina_on_cast = if unlocked_abilities >= 2 {
        5.0 + level("flurry_upgrade")
    } else {
        0.0
    };
    let flurry_casts_per_second = if unlocked_abilities >= 2 {
        cooldown_scale / flurry_cooldown.max(1.0)
    } else {
        0.0
    };
    let quake_effective_bonus = if unlocked_abilities >= 3 {
        0.20 * (quake_attack_count as f64) * cooldown_scale / quake_cooldown.max(1.0)
    } else {
        0.0
    };

    let speed_proc_bonus = (speed_mod_chance / 100.0)
        * (0.45 + speed_mod_gain_bonus / 100.0)
        * (1.0 + 0.03 * s.corruption as f64);

    let strength_skill_flat = 0.2 * level("strength_skill_buff") * s.strength as f64;
    let strength_skill_dmg = 0.1 * level("strength_skill_buff") * s.strength as f64;
    let asc1_strength_dmg = level("asc1_strength_skill_buff") * s.strength as f64;
    let crit_mult = 1.5 * (1.0 + crit_damage_pct / 100.0);
    let (fragment_gain_pct_breakdown, hestia_pct) = fragment_gain_pct_from_config(cfg);
    let (fragment_gain_mult_breakdown, axolotl_pct, geoduck_pct) =
        fragment_gain_mult_from_config(cfg);
    let exp_gain_pct_breakdown = exp_gain_pct_from_config(cfg);
    let exp_gain_mult_breakdown = exp_gain_mult_from_config(cfg);
    let loot_mod_chance_breakdown = loot_mod_chance_from_config(cfg);
    let exp_mod_chance_breakdown = exp_mod_chance_from_config(cfg);
    let loot_mod_avg_breakdown = loot_mod_avg_from_config(cfg);
    let exp_mod_avg_breakdown = exp_mod_avg_from_config(cfg);
    let additive_stamina_total =
        stamina_additive_base + stamina_agility_bonus + stamina_upgrade_bonus + stamina_skill_bonus;
    let fragment_base_total = 1.0 + fragment_gain_pct_breakdown / 100.0;
    let exp_base_total = 1.0 + exp_gain_pct_breakdown / 100.0;
    let loot_mod_gain_extra = loot_mod_avg_breakdown - 1.0;
    let exp_mod_gain_extra = exp_mod_avg_breakdown - 1.0;
    let breakdown = StatBreakdown {
        sections: vec![
            breakdown_section(
                "Damage",
                vec![
                    (
                        "Flat components".to_string(),
                        format!(
                            "10.00 + {:.2} + {:.2} + {:.2} + {:.2} + {:.2} + {:.2} + {:.2} + {:.2}",
                            s.strength as f64,
                            2.0 * s.divinity as f64,
                            level("flat_damage_common"),
                            2.0 * level("flat_damage_rare"),
                            2.0 * level("flat_damage_super_crit"),
                            3.0 * level("asc1_flat_damage_enrage"),
                            3.0 * level("asc1_flat_damage_ultra"),
                            strength_skill_flat
                        ),
                    ),
                    ("  Base damage".to_string(), "10.00".to_string()),
                    (
                        "  Strength flat".to_string(),
                        format!("+{:.2}", s.strength as f64),
                    ),
                    (
                        "  Divinity flat".to_string(),
                        format!("+{:.2}", 2.0 * s.divinity as f64),
                    ),
                    (
                        "  Common flat upgrade".to_string(),
                        format!("+{:.2}", level("flat_damage_common")),
                    ),
                    (
                        "  Rare flat upgrade".to_string(),
                        format!("+{:.2}", 2.0 * level("flat_damage_rare")),
                    ),
                    (
                        "  Super-crit flat upgrade".to_string(),
                        format!("+{:.2}", 2.0 * level("flat_damage_super_crit")),
                    ),
                    (
                        "  Asc1 Enrage flat upgrade".to_string(),
                        format!("+{:.2}", 3.0 * level("asc1_flat_damage_enrage")),
                    ),
                    (
                        "  Asc1 Ultra flat upgrade".to_string(),
                        format!("+{:.2}", 3.0 * level("asc1_flat_damage_ultra")),
                    ),
                    (
                        "  Strength skill flat bonus".to_string(),
                        format!("+{:.2}", strength_skill_flat),
                    ),
                    (
                        "Flat damage total".to_string(),
                        format!("{:.2}", flat_damage),
                    ),
                    (
                        "Additive % components".to_string(),
                        format!(
                            "{:.2}% + {:.2}% + {:.2}% + {:.2}% + {:.2}% + {:.2}%",
                            s.strength as f64,
                            6.0 * s.corruption as f64,
                            block_bonker_stage,
                            strength_skill_dmg,
                            asc1_strength_dmg,
                            2.0 * level("damage_armor_pen_mythic")
                                + 10.0 * level("asc1_damage_exp")
                        ),
                    ),
                    (
                        "  Strength %".to_string(),
                        format!("+{:.2}%", s.strength as f64),
                    ),
                    (
                        "  Corruption %".to_string(),
                        format!("+{:.2}%", 6.0 * s.corruption as f64),
                    ),
                    (
                        "  Block Bonker %".to_string(),
                        format!("+{:.2}%", block_bonker_stage),
                    ),
                    (
                        "  Strength skill % bonus".to_string(),
                        format!("+{:.2}%", strength_skill_dmg),
                    ),
                    (
                        "  Asc1 Strength skill % bonus".to_string(),
                        format!("+{:.2}%", asc1_strength_dmg),
                    ),
                    (
                        "  Mythic damage/pen %".to_string(),
                        format!("+{:.2}%", 2.0 * level("damage_armor_pen_mythic")),
                    ),
                    (
                        "  Asc1 damage/xp %".to_string(),
                        format!("+{:.2}%", 10.0 * level("asc1_damage_exp")),
                    ),
                    (
                        "Additive damage % total".to_string(),
                        format!("+{:.2}%", damage_pct),
                    ),
                    (
                        "Base hit before crits".to_string(),
                        format!(
                            "{:.2} x {:.4} = {:.2}",
                            flat_damage,
                            1.0 + damage_pct / 100.0,
                            flat_damage * (1.0 + damage_pct / 100.0)
                        ),
                    ),
                ],
            ),
            breakdown_section(
                "Crits",
                vec![
                    (
                        "Crit chance total".to_string(),
                        format!(
                            "{:.2}% = {:.2}% + {:.2}% + {:.2}%",
                            crit_chance,
                            s.agility as f64,
                            2.0 * s.luck as f64,
                            0.25 * level("crit_upgrade")
                        ),
                    ),
                    (
                        "  Agility crit".to_string(),
                        format!("+{:.2}%", s.agility as f64),
                    ),
                    (
                        "  Luck crit".to_string(),
                        format!("+{:.2}%", 2.0 * s.luck as f64),
                    ),
                    (
                        "  Crit upgrade chance".to_string(),
                        format!("+{:.2}%", 0.25 * level("crit_upgrade")),
                    ),
                    (
                        "Crit damage % total".to_string(),
                        format!(
                            "{:.2}% = {:.2}% + {:.2}% + {:.2}% + {:.2}%",
                            crit_damage_pct,
                            3.0 * s.strength as f64,
                            level("crit_upgrade"),
                            2.0 * level("crit_super_damage"),
                            asc1_strength_dmg
                        ),
                    ),
                    (
                        "  Strength crit dmg".to_string(),
                        format!("+{:.2}%", 3.0 * s.strength as f64),
                    ),
                    (
                        "  Crit upgrade dmg".to_string(),
                        format!("+{:.2}%", level("crit_upgrade")),
                    ),
                    (
                        "  Crit/Super dmg upgrade".to_string(),
                        format!("+{:.2}%", 2.0 * level("crit_super_damage")),
                    ),
                    (
                        "  Asc1 Strength skill dmg".to_string(),
                        format!("+{:.2}%", asc1_strength_dmg),
                    ),
                    (
                        "Base crit multiplier".to_string(),
                        format!(
                            "1.50 x {:.4} = {:.2}x",
                            1.0 + crit_damage_pct / 100.0,
                            crit_mult
                        ),
                    ),
                    (
                        "Enrage crit bonus when active".to_string(),
                        format!("+{:.2}%", enrage_crit_damage_bonus_pct),
                    ),
                    (
                        "Super crit chance".to_string(),
                        format!("{:.2}%", super_crit_chance),
                    ),
                    (
                        "Super crit multiplier".to_string(),
                        format!(
                            "2.50 + {:.4} = {:.2}x",
                            super_crit_damage_pct / 100.0,
                            2.5 + super_crit_damage_pct / 100.0
                        ),
                    ),
                    (
                        "Ultra crit chance".to_string(),
                        format!("{:.2}%", ultra_crit_chance),
                    ),
                    (
                        "Ultra crit multiplier".to_string(),
                        format!(
                            "4.00 + {:.4} = {:.2}x",
                            ultra_crit_damage_pct / 100.0,
                            4.0 + ultra_crit_damage_pct / 100.0
                        ),
                    ),
                ],
            ),
            breakdown_section(
                "Gain",
                vec![
                    (
                        "XP additive % total".to_string(),
                        format!("+{:.2}%", exp_gain_pct_breakdown),
                    ),
                    (
                        "  Intellect".to_string(),
                        format!("+{:.2}%", 5.0 * s.intellect as f64),
                    ),
                    (
                        "  Arch XP gems".to_string(),
                        format!("+{:.2}%", 5.0 * level("arch_exp_gems")),
                    ),
                    (
                        "  Arch XP common".to_string(),
                        format!("+{:.2}%", 2.0 * level("arch_exp_common")),
                    ),
                    (
                        "  Exp/Frag epic XP".to_string(),
                        format!("+{:.2}%", 3.0 * level("exp_frag_epic")),
                    ),
                    (
                        "  Exp/Stam legendary XP".to_string(),
                        format!("+{:.2}%", 5.0 * level("exp_stamina_legendary")),
                    ),
                    (
                        "  Intellect skill XP".to_string(),
                        format!(
                            "+{:.2}%",
                            level("intellect_skill_buff") * s.intellect as f64
                        ),
                    ),
                    (
                        "  Asc1 damage/xp".to_string(),
                        format!("+{:.2}%", 10.0 * level("asc1_damage_exp")),
                    ),
                    (
                        "XP multiplicative total".to_string(),
                        format!("{:.2}x", exp_gain_mult_breakdown),
                    ),
                    (
                        "  Exp Gain Double".to_string(),
                        if cfg.upgrades.get("exp_gain_double").copied().unwrap_or(0) > 0 {
                            "2.00x".to_string()
                        } else {
                            "1.00x".to_string()
                        },
                    ),
                    (
                        "  Arch Asc Shop Bundle".to_string(),
                        if cfg.arch_asc_shop_bundle {
                            "1.15x".to_string()
                        } else {
                            "1.00x".to_string()
                        },
                    ),
                    (
                        "Base XP per block".to_string(),
                        format!(
                            "{:.4} x {:.2} = {:.4}x",
                            exp_base_total,
                            exp_gain_mult_breakdown,
                            exp_base_total * exp_gain_mult_breakdown
                        ),
                    ),
                    (
                        "Fragment additive % total".to_string(),
                        format!("+{:.2}%", fragment_gain_pct_breakdown),
                    ),
                    (
                        "  Perception frags".to_string(),
                        format!("+{:.2}%", 4.0 * s.perception as f64),
                    ),
                    (
                        "  Fragment gems".to_string(),
                        format!("+{:.2}%", 2.0 * level("fragment_gain_gems")),
                    ),
                    (
                        "  Exp/Frag epic frags".to_string(),
                        format!("+{:.2}%", 2.0 * level("exp_frag_epic")),
                    ),
                    ("  Hestia".to_string(), format!("+{:.2}%", hestia_pct)),
                    (
                        "Fragment multiplicative total".to_string(),
                        format!("{:.4}x", fragment_gain_mult_breakdown),
                    ),
                    (
                        "  Arch Shop Bundle".to_string(),
                        if cfg.arch_shop_bundle {
                            "1.25x".to_string()
                        } else {
                            "1.00x".to_string()
                        },
                    ),
                    (
                        "  Fragment multiplier relic".to_string(),
                        if level("fragment_gain_multiplier") > 0.0 {
                            "1.25x".to_string()
                        } else {
                            "1.00x".to_string()
                        },
                    ),
                    (
                        "  Axolotl".to_string(),
                        format!("{:.4}x (+{:.2}%)", 1.0 + axolotl_pct / 100.0, axolotl_pct),
                    ),
                    (
                        "  Geoduck bonus".to_string(),
                        format!("{:.4}x (+{:.2}%)", 1.0 + geoduck_pct / 100.0, geoduck_pct),
                    ),
                    (
                        "Base fragments per block".to_string(),
                        format!(
                            "{:.4} x {:.4} = {:.4}x",
                            fragment_base_total,
                            fragment_gain_mult_breakdown,
                            fragment_base_total * fragment_gain_mult_breakdown
                        ),
                    ),
                ],
            ),
            breakdown_section(
                "XP Audit",
                vec![
                    (
                        "Verified additive sources".to_string(),
                        "Intellect +5%/pt, Arch XP Gems +5%/lvl, Arch XP Common +2%/lvl, Exp/Frag Epic +3%/lvl, Exp/Stam Legendary +5%/lvl"
                            .to_string(),
                    ),
                    (
                        "Verified multiplicative sources".to_string(),
                        if cfg.arch_asc_shop_bundle {
                            "Exp Gain Double 2.00x, Arch Asc Shop Bundle 1.15x".to_string()
                        } else {
                            "Exp Gain Double 2.00x".to_string()
                        },
                    ),
                    (
                        "Potentially uncertain".to_string(),
                        if cfg.arch_asc_shop_bundle {
                            "No XP fudge factors are currently applied; any remaining mismatch is formula uncertainty".to_string()
                        } else {
                            "Asc1 Damage/Exp XP contribution may not be a plain additive +10%/lvl"
                                .to_string()
                        },
                    ),
                    (
                        "Current modeled total".to_string(),
                        format!("{:.4}x", exp_base_total * exp_gain_mult_breakdown),
                    ),
                    (
                        "Expected target from notes".to_string(),
                        "10.35x".to_string(),
                    ),
                    (
                        "Current gap".to_string(),
                        format!(
                            "{:+.4}x",
                            (exp_base_total * exp_gain_mult_breakdown) - 10.35
                        ),
                    ),
                    (
                        "Observed clue".to_string(),
                        "When Asc1 Damage/Exp is set to 0, the current model matches 8.20x"
                            .to_string(),
                    ),
                ],
            ),
            breakdown_section(
                "Mods",
                vec![
                    (
                        "All-mod additive total".to_string(),
                        format!("+{:.2}%", all_mod),
                    ),
                    (
                        "  Luck all-mod".to_string(),
                        format!("+{:.2}%", 0.2 * s.luck as f64),
                    ),
                    (
                        "  All-mod upgrade".to_string(),
                        format!("+{:.2}%", 1.5 * level("all_mod_chances")),
                    ),
                    (
                        "  Asc1 all-mod upgrade".to_string(),
                        format!("+{:.2}%", 0.02 * level("asc1_all_mod")),
                    ),
                    (
                        "Loot mod chance total".to_string(),
                        format!("{:.2}%", loot_mod_chance_breakdown),
                    ),
                    (
                        "  Perception loot mod".to_string(),
                        format!("+{:.2}%", 0.3 * s.perception as f64),
                    ),
                    (
                        "  Fragment gems loot mod".to_string(),
                        format!("+{:.2}%", 0.05 * level("fragment_gain_gems")),
                    ),
                    (
                        "  Exp/Frag epic loot mod".to_string(),
                        "+0.00%".to_string(),
                    ),
                    (
                        "  Perception skill loot mod".to_string(),
                        format!(
                            "+{:.2}%",
                            0.01 * level("perception_skill_buff") * s.perception as f64
                        ),
                    ),
                    (
                        "  Arch Asc Shop Bundle".to_string(),
                        format!(
                            "+{:.2}%",
                            if cfg.arch_asc_shop_bundle { 2.0 } else { 0.0 }
                        ),
                    ),
                    (
                        "  Shared all-mod bonus".to_string(),
                        format!("+{:.2}%", all_mod),
                    ),
                    (
                        "Loot mod gain on proc".to_string(),
                        format!(
                            "{:.2}x ({:+.2}x extra loot)",
                            loot_mod_avg_breakdown, loot_mod_gain_extra
                        ),
                    ),
                    (
                        "Exp mod chance total".to_string(),
                        format!("{:.2}%", exp_mod_chance_breakdown),
                    ),
                    (
                        "  Intellect exp mod".to_string(),
                        format!("+{:.2}%", 0.3 * s.intellect as f64),
                    ),
                    (
                        "  Arch XP gems exp mod".to_string(),
                        format!("+{:.2}%", 0.05 * level("arch_exp_gems")),
                    ),
                    (
                        "  Exp mod gain upgrade chance".to_string(),
                        format!("+{:.2}%", 0.10 * level("exp_mod_gain")),
                    ),
                    (
                        "  Shared all-mod bonus".to_string(),
                        format!("+{:.2}%", all_mod),
                    ),
                    (
                        "Exp mod gain on proc".to_string(),
                        format!(
                            "{:.2}x ({:+.2}x extra xp)",
                            exp_mod_avg_breakdown, exp_mod_gain_extra
                        ),
                    ),
                ],
            ),
            breakdown_section(
                "Stamina",
                vec![
                    (
                        "Additive stamina total".to_string(),
                        format!("{:.2}", additive_stamina_total),
                    ),
                    (
                        "  Base".to_string(),
                        format!("{:.2}", stamina_additive_base),
                    ),
                    (
                        "  Agility".to_string(),
                        format!("+{:.2}", stamina_agility_bonus),
                    ),
                    (
                        "  Upgrades".to_string(),
                        format!("+{:.2}", stamina_upgrade_bonus),
                    ),
                    (
                        "  Skill bonus".to_string(),
                        format!("+{:.2}", stamina_skill_bonus),
                    ),
                    (
                        "Multipliers".to_string(),
                        format!(
                            "{:.4} x {:.4}",
                            1.0 + stamina_block_bonker_pct / 100.0,
                            1.0 + stamina_exp_legendary_pct / 100.0
                                + stamina_asc1_autotap_pct / 100.0
                                - stamina_corruption_pct / 100.0
                        ),
                    ),
                    (
                        "  Block Bonker multiplier".to_string(),
                        format!("{:.4}x", 1.0 + stamina_block_bonker_pct / 100.0),
                    ),
                    (
                        "  Other stamina multiplier".to_string(),
                        format!(
                            "{:.4}x",
                            1.0 + stamina_exp_legendary_pct / 100.0
                                + stamina_asc1_autotap_pct / 100.0
                                - stamina_corruption_pct / 100.0
                        ),
                    ),
                    (
                        "    Exp/Stam legendary".to_string(),
                        format!("+{:.2}%", stamina_exp_legendary_pct),
                    ),
                    (
                        "    Asc1 max stam/autotap".to_string(),
                        format!("+{:.2}%", stamina_asc1_autotap_pct),
                    ),
                    (
                        "    Corruption".to_string(),
                        format!("-{:.2}%", stamina_corruption_pct),
                    ),
                    (
                        "Max stamina".to_string(),
                        format!(
                            "{:.2} x {:.4} = {:.2}",
                            additive_stamina_total, max_stamina_mult, max_stamina
                        ),
                    ),
                    (
                        "Stamina mod chance".to_string(),
                        format!("{:.2}%", stamina_mod_chance),
                    ),
                    (
                        "Stamina mod gain on proc".to_string(),
                        format!("+{:.2}", stamina_mod_gain),
                    ),
                ],
            ),
            breakdown_section(
                "Crosshair",
                vec![
                    (
                        "Spawn chance".to_string(),
                        format!("{:.2}%", crosshair_spawn_chance),
                    ),
                    (
                        "Gold chance".to_string(),
                        format!("{:.2}%", gold_crosshair_chance),
                    ),
                    ("Base gold".to_string(), "+0.00%".to_string()),
                    (
                        "Luck gold".to_string(),
                        format!("+{:.2}%", 0.5 * s.luck as f64),
                    ),
                    (
                        "Asc1 gold crosshair".to_string(),
                        format!("+{:.2}%", level("asc1_gold_crosshair")),
                    ),
                    (
                        "Arch Asc Shop Bundle".to_string(),
                        format!(
                            "+{:.2}%",
                            if cfg.arch_asc_shop_bundle { 2.0 } else { 0.0 }
                        ),
                    ),
                    (
                        "Auto-tap chance".to_string(),
                        format!("{:.2}%", crosshair_auto_tap_chance),
                    ),
                    ("Base auto-tap".to_string(), "+0.00%".to_string()),
                    (
                        "Divinity auto-tap".to_string(),
                        format!("+{:.2}%", 2.0 * s.divinity as f64),
                    ),
                    (
                        "Asc1 gold crosshair auto-tap".to_string(),
                        format!("+{:.2}%", level("asc1_gold_crosshair")),
                    ),
                    (
                        "Arch Asc Shop Bundle".to_string(),
                        format!(
                            "+{:.2}%",
                            if cfg.arch_asc_shop_bundle { 5.0 } else { 0.0 }
                        ),
                    ),
                    (
                        "Asc1 max stam/autotap".to_string(),
                        format!("+{:.2}%", 0.20 * level("asc1_max_stam_autotap")),
                    ),
                    (
                        "Expected bonus hits/attack".to_string(),
                        format!("{:.4}", crosshair_bonus_hits_per_attack),
                    ),
                ],
            ),
            breakdown_section(
                "Abilities",
                vec![
                    (
                        "Enrage attacks/cast".to_string(),
                        enrage_attack_count.to_string(),
                    ),
                    (
                        "Enrage cooldown".to_string(),
                        format!("{:.2}s", enrage_cooldown),
                    ),
                    (
                        "Enrage bonus when active".to_string(),
                        format!(
                            "+{:.2}% dmg, +{:.2}% crit dmg",
                            enrage_damage_bonus_pct, enrage_crit_damage_bonus_pct
                        ),
                    ),
                    (
                        "Whirlwind attacks/cast".to_string(),
                        flurry_attack_count.to_string(),
                    ),
                    (
                        "Whirlwind cooldown".to_string(),
                        format!("{:.2}s", flurry_cooldown),
                    ),
                    (
                        "Whirlwind stamina refund".to_string(),
                        format!("+{:.2}", flurry_stamina_on_cast),
                    ),
                    (
                        "Quake attacks/cast".to_string(),
                        quake_attack_count.to_string(),
                    ),
                    (
                        "Quake cooldown".to_string(),
                        format!("{:.2}s", quake_cooldown),
                    ),
                    (
                        "Quake splash per active attack".to_string(),
                        "20% of rolled hit".to_string(),
                    ),
                ],
            ),
        ],
    };

    DerivedStats {
        flat_damage,
        damage_pct,
        crit_chance,
        crit_damage_pct,
        super_crit_chance,
        super_crit_damage_pct,
        ultra_crit_chance,
        ultra_crit_damage_pct,
        armor_pen_flat,
        armor_pen_pct,
        fragment_gain_pct,
        fragment_gain_mult,
        exp_gain_pct,
        exp_gain_mult,
        max_stamina,
        speed_mod_chance,
        stamina_mod_chance,
        stamina_mod_gain,
        speed_mod_gain_bonus,
        loot_mod_chance,
        exp_mod_chance,
        loot_mod_avg,
        exp_mod_avg,
        passive_attack_rate: (1.0 + speed_proc_bonus).max(0.10),
        cards: cfg.cards.clone(),
        unlocked_abilities,
        flurry_casts_per_second,
        quake_effective_bonus,
        enrage_attack_count,
        flurry_attack_count,
        quake_attack_count,
        enrage_damage_bonus_pct,
        enrage_crit_damage_bonus_pct,
        enrage_cooldown,
        flurry_cooldown,
        quake_cooldown,
        flurry_stamina_on_cast,
        ability_instacharge_chance,
        crosshair_spawn_chance,
        gold_crosshair_chance,
        crosshair_auto_tap_chance,
        crosshair_bonus_hits_per_attack,
        archaeology_poly_card_bonus: level("poly_archaeology_card_bonus") > 0.0,
        stamina_additive_base,
        stamina_agility_bonus,
        stamina_upgrade_bonus,
        stamina_skill_bonus,
        stamina_block_bonker_pct,
        stamina_exp_legendary_pct,
        stamina_asc1_autotap_pct,
        stamina_corruption_pct,
        breakdown,
    }
}

fn card_bonus_fraction(stats: &DerivedStats, quality: CardQuality) -> f64 {
    match quality {
        CardQuality::None => 0.0,
        CardQuality::Standard => 0.10,
        CardQuality::Gilded => 0.20,
        CardQuality::Polychrome | CardQuality::Infernal => {
            if stats.archaeology_poly_card_bonus {
                0.50
            } else {
                0.35
            }
        }
    }
}

#[derive(Clone)]
struct SimBlock {
    rarity: Rarity,
    hp: f64,
    armor: f64,
    base_xp: f64,
    base_fragments: f64,
}

#[derive(Default)]
struct AbilityTracker {
    next_ready_at: f64,
    attacks_remaining: u32,
    casts: u32,
    attacks_while_active: u64,
}

struct SimRng {
    state: u64,
}

impl SimRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(2685821657736338717)
    }

    fn next_f64(&mut self) -> f64 {
        let v = self.next_u64() >> 11;
        (v as f64) / ((1u64 << 53) as f64)
    }

    fn index(&mut self, upper_exclusive: usize) -> usize {
        if upper_exclusive <= 1 {
            0
        } else {
            (self.next_u64() as usize) % upper_exclusive
        }
    }

    fn roll(&mut self, chance: f64) -> bool {
        self.next_f64() < chance.clamp(0.0, 1.0)
    }
}

fn sample_weighted_rarity(weights: &[(Rarity, f64)], rng: &mut SimRng) -> Rarity {
    let total: f64 = weights.iter().map(|(_, weight)| *weight).sum();
    if total <= 0.0 {
        return weights
            .first()
            .map(|(rarity, _)| *rarity)
            .unwrap_or(Rarity::Dirt);
    }
    let mut roll = rng.next_f64() * total;
    for (rarity, weight) in weights {
        roll -= *weight;
        if roll <= 0.0 {
            return *rarity;
        }
    }
    weights
        .last()
        .map(|(rarity, _)| *rarity)
        .unwrap_or(Rarity::Dirt)
}

fn concrete_stage_blocks(stage: u32, stats: &DerivedStats, rng: &mut SimRng) -> Vec<SimBlock> {
    let layout = stage_layout(stage);
    let sampled_rarities: Vec<Rarity> = if BOSS_FLOORS
        .iter()
        .any(|(boss_stage, _)| *boss_stage == stage)
    {
        let mut out = Vec::new();
        for (rarity, count) in layout {
            for _ in 0..count.round() as u32 {
                out.push(rarity);
            }
        }
        // Boss floors keep exact counts, but shuffle order so attack progression is still variable.
        for i in (1..out.len()).rev() {
            let j = rng.index(i + 1);
            out.swap(i, j);
        }
        out
    } else {
        (0..BLOCKS_PER_STAGE as usize)
            .map(|_| sample_weighted_rarity(&layout, rng))
            .collect()
    };

    let mut out = Vec::new();
    for rarity in sampled_rarities {
        let block = block_for_stage(rarity, stage);
        let (mut hp, armor) = scaled_stats(block, stage);
        let card_quality = stats
            .cards
            .get(&CardKey {
                rarity,
                tier: block.tier,
            })
            .copied()
            .unwrap_or(CardQuality::None);
        let card_bonus = card_bonus_fraction(stats, card_quality);
        hp *= 1.0 - card_bonus;
        out.push(SimBlock {
            rarity,
            hp,
            armor,
            base_xp: block.xp
                * (1.0 + card_bonus)
                * (1.0 + stats.exp_gain_pct / 100.0)
                * stats.exp_gain_mult,
            base_fragments: block.fragments
                * (1.0 + card_bonus)
                * (1.0 + stats.fragment_gain_pct / 100.0)
                * stats.fragment_gain_mult,
        });
    }
    out
}

fn autocast_abilities(
    stats: &DerivedStats,
    stamina: &mut f64,
    now: &mut f64,
    enrage: &mut AbilityTracker,
    flurry: &mut AbilityTracker,
    quake: &mut AbilityTracker,
) {
    let trackers = [
        (
            stats.unlocked_abilities >= 1,
            stats.enrage_attack_count,
            stats.enrage_cooldown,
            0.0,
            enrage,
        ),
        (
            stats.unlocked_abilities >= 2,
            stats.flurry_attack_count,
            stats.flurry_cooldown,
            stats.flurry_stamina_on_cast,
            flurry,
        ),
        (
            stats.unlocked_abilities >= 3,
            stats.quake_attack_count,
            stats.quake_cooldown,
            0.0,
            quake,
        ),
    ];
    for (enabled, attacks, cooldown, stamina_gain, tracker) in trackers {
        if enabled && tracker.attacks_remaining == 0 && *now >= tracker.next_ready_at {
            tracker.attacks_remaining = attacks;
            tracker.next_ready_at = *now + cooldown;
            tracker.casts += 1;
            if stamina_gain > 0.0 {
                *stamina = (*stamina + stamina_gain).min(stats.max_stamina);
            }
        }
    }
}

fn apply_attack_to_stage(
    blocks: &mut Vec<SimBlock>,
    direct_damage: f64,
    quake_damage: f64,
    stats: &DerivedStats,
    stamina: &mut f64,
    stage_xp: &mut f64,
    stage_fragments: &mut HashMap<Currency, f64>,
    rarity_totals: &mut [RarityTotal],
    destroyed_blocks: &mut f64,
    total_damage_done: &mut f64,
    rng: &mut SimRng,
) {
    if blocks.is_empty() {
        return;
    }
    let armor_pen_flat = stats.armor_pen_flat;
    let armor_pen_pct = stats.armor_pen_pct;
    let effective_armor =
        |armor: f64| ((armor * (1.0 - armor_pen_pct / 100.0)) - armor_pen_flat).max(0.0);
    let mut damages = vec![0.0; blocks.len()];
    damages[0] += (direct_damage - effective_armor(blocks[0].armor)).max(1.0);
    if quake_damage > 0.0 {
        for (i, block) in blocks.iter().enumerate() {
            damages[i] += (quake_damage - effective_armor(block.armor)).max(1.0);
        }
    }

    let mut i = 0usize;
    while i < blocks.len() {
        let applied = damages[i].min(blocks[i].hp);
        blocks[i].hp -= damages[i];
        *total_damage_done += applied.max(0.0);
        if blocks[i].hp <= 0.0 {
            let block = blocks.remove(i);
            *destroyed_blocks += 1.0;
            let xp_mult = if rng.roll(stats.exp_mod_chance / 100.0) {
                stats.exp_mod_avg
            } else {
                1.0
            };
            let frag_mult = if rng.roll(stats.loot_mod_chance / 100.0) {
                stats.loot_mod_avg
            } else {
                1.0
            };
            let block_xp = block.base_xp * xp_mult;
            let block_fragments = block.base_fragments * frag_mult;
            *stage_xp += block_xp;
            if rng.roll(stats.stamina_mod_chance / 100.0) {
                *stamina = (*stamina + stats.stamina_mod_gain).min(stats.max_stamina);
            }
            if let Some(currency) = block.rarity.fragment_currency() {
                *stage_fragments.entry(currency).or_insert(0.0) += block_fragments;
            }
            if let Some(total) = rarity_totals
                .iter_mut()
                .find(|t| t.rarity == Some(block.rarity))
            {
                total.blocks_destroyed += 1.0;
                total.xp += block_xp;
                total.fragments += block_fragments;
            }
            damages.remove(i);
        } else {
            i += 1;
        }
    }
}

fn hit_base_damage(stats: &DerivedStats, enrage_active: bool) -> f64 {
    stats.flat_damage
        * (1.0
            + (stats.damage_pct
                + if enrage_active {
                    stats.enrage_damage_bonus_pct
                } else {
                    0.0
                })
                / 100.0)
}

fn sample_hit_damage(stats: &DerivedStats, enrage_active: bool, rng: &mut SimRng) -> f64 {
    let base = hit_base_damage(stats, enrage_active);
    let crit_chance = (stats.crit_chance / 100.0).clamp(0.0, 1.0);
    let super_crit_chance = (stats.super_crit_chance / 100.0).clamp(0.0, 1.0);
    let ultra_crit_chance = (stats.ultra_crit_chance / 100.0).clamp(0.0, 1.0);
    let crit_mult = 1.5
        * (1.0
            + (stats.crit_damage_pct
                + if enrage_active {
                    stats.enrage_crit_damage_bonus_pct
                } else {
                    0.0
                })
                / 100.0);
    let super_mult = 2.5 + stats.super_crit_damage_pct / 100.0;
    let ultra_mult = 4.0 + stats.ultra_crit_damage_pct / 100.0;
    let mut damage = base;
    if rng.roll(crit_chance) {
        damage *= crit_mult;
        if rng.roll(super_crit_chance) {
            damage *= super_mult;
            if rng.roll(ultra_crit_chance) {
                damage *= ultra_mult;
            }
        }
    }
    damage
}

fn direct_attack_damage_from_roll(
    stats: &DerivedStats,
    main_hit_damage: f64,
    enrage_active: bool,
    rng: &mut SimRng,
) -> f64 {
    let mut total = main_hit_damage;
    let crosshair_spawn_chance = (stats.crosshair_spawn_chance / 100.0).clamp(0.0, 1.0);
    let crosshair_auto_tap_chance = (stats.crosshair_auto_tap_chance / 100.0).clamp(0.0, 1.0);
    let gold_crosshair_chance = (stats.gold_crosshair_chance / 100.0).clamp(0.0, 1.0);
    if rng.roll(crosshair_spawn_chance) && rng.roll(crosshair_auto_tap_chance) {
        let mut bonus_hit = sample_hit_damage(stats, enrage_active, rng);
        if rng.roll(gold_crosshair_chance) {
            bonus_hit *= 3.0;
        }
        total += bonus_hit;
    }
    total
}

fn block_for_stage(rarity: Rarity, stage: u32) -> BlockTier {
    BLOCK_TIERS
        .iter()
        .filter(|b| b.rarity == rarity && b.unlock_wave <= stage)
        .max_by_key(|b| b.unlock_wave)
        .copied()
        .unwrap_or_else(|| {
            BLOCK_TIERS
                .iter()
                .find(|b| b.rarity == rarity)
                .copied()
                .expect("missing block tier")
        })
}

fn scaled_stats(block: BlockTier, stage: u32) -> (f64, f64) {
    if stage >= 150 {
        (block.hp150, block.armor150)
    } else if stage >= 100 {
        (block.hp100, block.armor100)
    } else {
        (block.hp, block.armor)
    }
}

fn stage_layout(stage: u32) -> Vec<(Rarity, f64)> {
    if let Some((_, counts)) = BOSS_FLOORS
        .iter()
        .find(|(boss_stage, _)| *boss_stage == stage)
    {
        return counts
            .iter()
            .map(|(rarity, count)| (*rarity, *count as f64))
            .collect();
    }

    let weights = SPAWN_TABLE
        .iter()
        .find(|(start, end, _)| stage >= *start && stage <= *end)
        .map(|(_, _, weights)| *weights)
        .unwrap_or_else(|| SPAWN_TABLE.last().expect("spawn table").2);

    let total_weight: f64 = weights.iter().map(|(_, w)| *w).sum();
    weights
        .iter()
        .map(|(rarity, weight)| (*rarity, BLOCKS_PER_STAGE * (*weight / total_weight)))
        .collect()
}
