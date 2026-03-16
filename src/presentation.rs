use std::collections::HashMap;
use std::fmt::Write as _;

use crate::game_data::{CardQuality, Currency, UPGRADE_SPECS, UpgradeSpec};
use crate::sim::{
    additive_damage_pct_from_config, block_bonker_stage_bonus_pct, derive_stats,
    evaluate_optimizer, flat_damage_from_config, optimize_skills, simulate_run,
};
use crate::state::{Config, Objective};
use crate::types::{
    DerivedStats, OptimizerEvaluation, Recommendation, SimulationResult, UpgradeCatalog,
};

const SHEET_DAMAGE_CALIBRATION: f64 = 1.0;

fn matching_card_bonus_fraction(stats: &DerivedStats) -> f64 {
    stats.cards.values().fold(0.0, |best, quality| {
        let bonus = match quality {
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
        };
        best.max(bonus)
    })
}

pub(crate) fn explicit_upgrade_max_level(id: &str) -> Option<u32> {
    match id {
        "unlock_ability" => Some(3),
        "max_stamina_gems" => Some(50),
        "arch_exp_gems" => Some(25),
        "fragment_gain_gems" => Some(25),
        "max_stamina_rare" => Some(20),
        "asc1_crosshair_armor_pen" => Some(15),
        "asc1_gold_crosshair" => Some(5),
        _ => None,
    }
}

pub(crate) fn upgrade_gui_label(spec: UpgradeSpec, level: u32) -> String {
    match spec.id {
        "unlock_ability" => {
            let next_unlock = match level {
                0 => "next: Enrage",
                1 => "next: Whirlwind",
                2 => "next: Quake",
                _ => "maxed",
            };
            format!("{} ({next_unlock})", spec.caption)
        }
        _ => spec.caption.to_string(),
    }
}

pub(crate) fn upgrade_total_effect(spec: UpgradeSpec, level: u32) -> String {
    let lvl = level as f64;
    match spec.id {
        "max_stamina_gems" | "max_stamina_rare" => {
            format!(
                "+{:.0} stamina, +{:.2}% stam mod ch.",
                2.0 * lvl,
                0.05 * lvl
            )
        }
        "arch_exp_gems" => format!("+{:.0}% xp, +{:.2}% exp mod ch.", 5.0 * lvl, 0.05 * lvl),
        "fragment_gain_gems" => {
            format!(
                "+{:.0}% fragments, +{:.2}% loot mod ch.",
                2.0 * lvl,
                0.05 * lvl
            )
        }
        "unlock_ability" => match level {
            0 => "no abilities".to_string(),
            1 => "Enrage".to_string(),
            2 => "Enrage, Whirlwind".to_string(),
            _ => "Enrage, Whirlwind, Quake".to_string(),
        },
        "flat_damage_common" => format!("+{level} flat damage"),
        "armor_pen_common" => format!("+{level} armor pen"),
        "arch_exp_common" => format!("+{:.0}% xp", 2.0 * lvl),
        "crit_upgrade" => format!("+{:.2}% crit, +{:.0}% crit dmg", 0.25 * lvl, lvl),
        "flat_damage_rare" => format!("+{:.0} flat damage", 2.0 * lvl),
        "loot_mod_gain" => format!("+{:.2}x loot mod", 0.30 * lvl),
        "enrage_upgrade" => format!("+{:.0}% enrage/crit dmg, -{:.0}s cd", 2.0 * lvl, lvl),
        "flat_damage_super_crit" => {
            format!(
                "+{:.0} flat damage, +{:.2}% super crit",
                2.0 * lvl,
                0.35 * lvl
            )
        }
        "exp_frag_epic" => format!("+{:.0}% xp, +{:.0}% fragments", 3.0 * lvl, 2.0 * lvl),
        "flurry_upgrade" => format!("+{level} flurry stamina, -{level}s cd"),
        "max_stamina_epic" => format!("+{:.0} stamina, +{level} stam mod gain", 4.0 * lvl),
        "strength_skill_buff" => format!("+{:.1} flat/str, +{:.1}% dmg/str", 0.2 * lvl, 0.1 * lvl),
        "agility_skill_buff" => format!("+{level} stamina/agi, +{:.2}% mod ch./agi", 0.02 * lvl),
        "exp_stamina_legendary" => format!("+{:.0}% xp, +{:.0}% max stamina", 5.0 * lvl, lvl),
        "armor_pen_pct_cdr" => format!("+{:.0}% armor pen, -{:.0}s cd", 2.0 * lvl, lvl),
        "crit_super_damage" => format!("+{:.0}% crit dmg, +{:.0}% super dmg", 2.0 * lvl, 2.0 * lvl),
        "quake_upgrade" => format!("+{level} quake hits, -{:.0}s cd", 2.0 * lvl),
        "perception_skill_buff" => {
            format!("+{:.2}% mod ch./per, +{level} armor pen/per", 0.01 * lvl)
        }
        "intellect_skill_buff" => format!("+{level}% xp/int, +{:.2}% mod ch./int", 0.01 * lvl),
        "damage_armor_pen_mythic" => format!("+{:.0}% dmg, +{:.0} armor pen", 2.0 * lvl, 3.0 * lvl),
        "super_ultra_crit" => format!("+{:.2}% super crit, +{:.0}% ultra crit", 0.35 * lvl, lvl),
        "exp_mod_gain" => format!(
            "+{:.2}x exp mod, +{:.2}% exp mod ch.",
            0.10 * lvl,
            0.10 * lvl
        ),
        "instacharge_stamina" => {
            format!("+{:.2}% instacharge, +{:.0} stamina", 0.30 * lvl, 4.0 * lvl)
        }
        "poly_archaeology_card_bonus" => {
            if level > 0 {
                "+15% poly card bonus".to_string()
            } else {
                "inactive".to_string()
            }
        }
        "fragment_gain_multiplier" => {
            if level > 0 {
                "1.25x fragments".to_string()
            } else {
                "inactive".to_string()
            }
        }
        "stamina_mod_gain_once" => format!("+{:.0} stam mod gain", 2.0 * lvl),
        "all_mod_chances" => format!("+{:.2}% all mod chances", 1.5 * lvl),
        "exp_gain_double" => {
            if level > 0 {
                "2.00x xp, +5 all stat caps".to_string()
            } else {
                "inactive".to_string()
            }
        }
        "asc1_stat_points" => format!("+{level} stat points"),
        "asc1_all_mod" => format!("+{:.2}% all mod chances", 0.02 * lvl),
        "asc1_crosshair_armor_pen" => format!("+{:.0} armor pen, unlocks fairies", 3.0 * lvl),
        "asc1_flat_damage_enrage" => format!("+{:.0} flat damage, -{level}s enrage cd", 3.0 * lvl),
        "asc1_ultra_damage" => format!(
            "+{:.0}% ultra dmg, +{:.2}% stam mod ch.",
            2.0 * lvl,
            0.03 * lvl
        ),
        "asc1_strength_skill_buff" => {
            format!("+{level}% dmg/str buff, +{level}% crit dmg/str buff")
        }
        "asc1_gold_crosshair" => format!("+{level}% gold crosshair, +{level}% auto-tap"),
        "asc1_flat_damage_ultra" => format!(
            "+{:.0} flat damage, +{:.2}% ultra crit",
            3.0 * lvl,
            0.50 * lvl
        ),
        "asc1_instacharge_stamina" => format!(
            "+{:.2}% instacharge, +{:.2}% stam mod ch.",
            0.10 * lvl,
            0.10 * lvl
        ),
        "asc1_damage_exp" => format!("+{:.0}% damage, +{:.0}% xp", 10.0 * lvl, 10.0 * lvl),
        "asc1_supercrit_expmod" => format!(
            "+{:.2}% super dmg, +{:.0}% exp mod gain",
            0.50 * lvl,
            2.0 * lvl
        ),
        "asc1_max_stam_autotap" => format!(
            "+{:.2}% max stamina, +{:.2}% auto-tap",
            0.50 * lvl,
            0.20 * lvl
        ),
        _ => format!("level {level}"),
    }
}

pub(crate) fn format_current_stats(cfg: &Config) -> String {
    let stats = derive_stats(cfg);
    let sim = simulate_run(cfg);
    let damage_sheet = sheet_damage(cfg, &stats);
    let armor_pen_sheet = stats.armor_pen_flat * (1.0 + stats.armor_pen_pct / 100.0);
    let crit_mult = 1.5 * (1.0 + stats.crit_damage_pct / 100.0);
    let super_mult = sheet_super_crit_damage_mult(cfg);
    let ultra_mult = 4.0 + stats.ultra_crit_damage_pct / 100.0;
    let exp_gain_total = (1.0 + stats.exp_gain_pct / 100.0) * stats.exp_gain_mult;
    let fragment_gain_total = (1.0 + stats.fragment_gain_pct / 100.0) * stats.fragment_gain_mult;
    let matching_card_bonus = matching_card_bonus_fraction(&stats);
    let matching_card_mult = 1.0 + matching_card_bonus;
    let exp_with_matching_card = exp_gain_total * matching_card_mult;
    let fragment_with_matching_card = fragment_gain_total * matching_card_mult;
    let exp_on_proc = exp_gain_total * stats.exp_mod_avg;
    let exp_card_and_proc = exp_with_matching_card * stats.exp_mod_avg;
    let fragment_on_proc = fragment_gain_total * stats.loot_mod_avg;
    let fragment_card_and_proc = fragment_with_matching_card * stats.loot_mod_avg;
    let loot_mod_chance_sheet = stats.loot_mod_chance;
    let speed_mod_attack_rate_mult = 2.0 * (1.0 + 0.03 * cfg.skills.corruption as f64);
    let whirlwind_attack_speed_bonus = if stats.unlocked_abilities >= 2 {
        100.0 * 0.20 * stats.flurry_casts_per_second * 120.0 / 5.0
    } else {
        0.0
    };
    let quake_bonus_pct = 100.0 * stats.quake_effective_bonus;
    let active_cards = cfg.cards.len();
    let polychrome_cards = cfg
        .cards
        .values()
        .filter(|quality| matches!(quality, CardQuality::Polychrome | CardQuality::Infernal))
        .count();
    let mut out = String::new();
    let left = vec![
        format!("Archaeology Level: {}", cfg.archaeology_level),
        format!("Highest Stage: {}", cfg.highest_stage_reached),
        format!("Max Stamina: {:.0}", stats.max_stamina),
        format!("Current Stamina: {:.0}", sim.ending_stamina),
        format!("Damage: {:.0}", damage_sheet),
        format!("Armor Penetration: {:.0}", armor_pen_sheet),
        format!(
            "Atk Speed: {}/sec",
            if cfg.sheet_speed_mod_always_active {
                2
            } else {
                1
            }
        ),
        format!("Crit Chance: {:.2}%", stats.crit_chance),
        format!("Crit Damage: {:.2}x", crit_mult),
        format!("Super Crit Chance: {:.2}%", stats.super_crit_chance),
        format!("Super Crit Damage: {:.2}x", super_mult),
        format!("Ultra Crit Chance: {:.2}%", stats.ultra_crit_chance),
        format!("Ultra Crit Damage: {:.2}x", ultra_mult),
        format!(
            "Ability Instacharge: {:.2}%",
            stats.ability_instacharge_chance
        ),
        format!(
            "Crosshair Auto-Tap: {:.2}%",
            stats.crosshair_auto_tap_chance
        ),
        format!("Gold Crosshair Chance: {:.0}%", stats.gold_crosshair_chance),
        "Gold Crosshair Multi: 3x".to_string(),
    ];
    let right = vec![
        format!("Base Exp Gain: {:.2}x", exp_gain_total),
        format!("Exp w/ Matching Card: {:.2}x", exp_with_matching_card),
        format!("Exp Mod Chance: {:.2}%", stats.exp_mod_chance),
        format!("Exp on Exp Mod Proc: {:.2}x", exp_on_proc),
        format!("Exp Card + Proc: {:.2}x", exp_card_and_proc),
        format!("Base Fragment Gain: {:.2}x", fragment_gain_total),
        format!("Frag w/ Matching Card: {:.2}x", fragment_with_matching_card),
        format!("Loot Mod Chance: {:.2}%", loot_mod_chance_sheet),
        format!("Frags on Loot Proc: {:.2}x", fragment_on_proc),
        format!("Frag Card + Proc: {:.2}x", fragment_card_and_proc),
        format!("Speed Mod Chance: {:.2}%", stats.speed_mod_chance),
        format!("Speed Mod Gain: +{:.0}", stats.speed_mod_gain_bonus),
        format!("Speed Mod Atk Rate: {:.2}x", speed_mod_attack_rate_mult),
        format!("Stamina Mod Chance: {:.2}%", stats.stamina_mod_chance),
        format!("Stamina Mod Gain: +{:.0}", stats.stamina_mod_gain),
        format!("Whirlwind casts/s: {:.4}", stats.flurry_casts_per_second),
        format!(
            "Whirlwind atk speed bonus: {:.2}%",
            whirlwind_attack_speed_bonus
        ),
        format!("Quake bonus damage: {:.2}%", quake_bonus_pct),
        format!(
            "Quake cooldown model: lvl {}",
            cfg.upgrades.get("quake_upgrade").copied().unwrap_or(0)
        ),
        format!("Active cards: {active_cards}"),
        format!("Poly cards: {polychrome_cards}"),
    ];
    let _ = writeln!(out, "Current Build Stats");
    let _ = writeln!(
        out,
        "Skills: str {}  agi {}  per {}  int {}  luck {}  div {}  corr {}",
        cfg.skills.strength,
        cfg.skills.agility,
        cfg.skills.perception,
        cfg.skills.intellect,
        cfg.skills.luck,
        cfg.skills.divinity,
        cfg.skills.corruption
    );
    let _ = writeln!(out);
    for i in 0..left.len().max(right.len()) {
        let lhs = left.get(i).map(String::as_str).unwrap_or("");
        let rhs = right.get(i).map(String::as_str).unwrap_or("");
        let _ = writeln!(out, "{lhs:<34} {rhs}");
    }
    out
}

pub(crate) fn format_stat_breakdown(cfg: &Config) -> String {
    let stats = derive_stats(cfg);
    let mut out = String::new();
    let _ = writeln!(out, "Stat Breakdown");
    let _ = writeln!(out);
    for section in &stats.breakdown.sections {
        let _ = writeln!(out, "{}", section.title);
        for line in &section.lines {
            let _ = writeln!(out, "  {}: {}", line.label, line.value);
        }
        let _ = writeln!(out);
    }
    out
}

pub(crate) fn format_simulation(cfg: &Config, result: &SimulationResult) -> String {
    let stats = derive_stats(cfg);
    let mut out = String::new();
    let left = vec![
        "Run model: stage 1 -> depletion/reset".to_string(),
        format!("Objective: {}", cfg.objective),
        format!("Ascension: {}", cfg.ascension),
        format!("Stage cap: {}", cfg.stage_cap),
        format!(
            "Highest stage: {} -> {}",
            result.previous_highest_stage, result.updated_highest_stage
        ),
        format!("Stages fully cleared: {}", result.full_stages_cleared),
        format!("Highest stage reached: {}", result.max_stage_reached),
        format!("Run time: {:.2}s", result.run_time),
        format!("Attack rate: {:.3}/s", result.attacks_per_second),
        format!("Total attacks: {}", result.total_attacks),
    ];
    let right = vec![
        format!("Average hit: {:.2}", result.avg_damage_per_hit),
        format!("Max stamina: {:.2}", result.max_stamina),
        format!("Starting stamina: {:.2}", result.starting_stamina),
        format!("Ending stamina: {:.2}", result.ending_stamina),
        format!(
            "Experience: {:.3}/run ({:.5}/s)",
            result.xp_per_run, result.xp_per_second
        ),
        format!("Enrage casts: {}", result.enrage_casts),
        format!("Whirlwind casts: {}", result.flurry_casts),
        format!("Quake casts: {}", result.quake_casts),
        format!(
            "Crosshair hits/atk: {:.4}",
            stats.crosshair_bonus_hits_per_attack
        ),
        format!("Speed mod permanent: {}", cfg.sheet_speed_mod_always_active),
    ];
    for i in 0..left.len().max(right.len()) {
        let lhs = left.get(i).map(String::as_str).unwrap_or("");
        let rhs = right.get(i).map(String::as_str).unwrap_or("");
        let _ = writeln!(out, "{lhs:<42} {rhs}");
    }
    if cfg.stage_cap >= 50 || result.max_stage_reached >= 50 {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "Divine spawn note: stage 50+ normal Divine block spawn weights are provisional estimates extrapolated from Mythic trends."
        );
    }
    if result.ended_at_cap {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "Stopped because stage cap was reached before depletion"
        );
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "Fragments");
    let mut currencies: Vec<_> = result.fragment_per_run.iter().collect();
    currencies.sort_by_key(|(currency, _)| **currency);
    for (currency, amount) in currencies {
        let rate = result
            .fragment_per_second
            .get(currency)
            .copied()
            .unwrap_or(0.0);
        let _ = writeln!(
            out,
            "{currency} fragments: {:.4}/run ({:.6}/s)",
            amount, rate
        );
    }

    if cfg.verbose_output {
        let _ = writeln!(out);
        let _ = writeln!(out, "Verbose Diagnostics");
        let _ = writeln!(
            out,
            "Crosshair spawn chance: {:.2}%",
            stats.crosshair_spawn_chance
        );
        let _ = writeln!(
            out,
            "Enrage: {} casts, {:.2}% attack uptime, {} attacks/cast, {:.2}s cooldown",
            result.enrage_casts,
            result.enrage_uptime * 100.0,
            stats.enrage_attack_count,
            stats.enrage_cooldown
        );
        let _ = writeln!(
            out,
            "Whirlwind: {} casts, {:.2}% attack uptime, {} attacks/cast, {:.2}s cooldown, +{:.0} stamina/cast",
            result.flurry_casts,
            result.flurry_uptime * 100.0,
            stats.flurry_attack_count,
            stats.flurry_cooldown,
            stats.flurry_stamina_on_cast
        );
        let _ = writeln!(
            out,
            "Quake: {} casts, {:.2}% attack uptime, {} attacks/cast, {:.2}s cooldown, 20% splash",
            result.quake_casts,
            result.quake_uptime * 100.0,
            stats.quake_attack_count,
            stats.quake_cooldown
        );
        let _ = writeln!(out, "Block Bonker active: {}", cfg.block_bonker);
        let _ = writeln!(out, "Avada Keda active: {}", cfg.avada_keda);
        let _ = writeln!(out, "Arch Shop Bundle active: {}", cfg.arch_shop_bundle);
        let _ = writeln!(
            out,
            "Arch Asc Shop Bundle active: {}",
            cfg.arch_asc_shop_bundle
        );
        let stamina_additive_total = stats.stamina_additive_base
            + stats.stamina_agility_bonus
            + stats.stamina_upgrade_bonus
            + stats.stamina_skill_bonus;
        let stamina_multiplier = (1.0 + stats.stamina_block_bonker_pct / 100.0)
            * (1.0
                + stats.stamina_exp_legendary_pct / 100.0
                + stats.stamina_asc1_autotap_pct / 100.0
                - stats.stamina_corruption_pct / 100.0);
        let _ = writeln!(
            out,
            "Total additive fragment gain: +{:.2}%",
            stats.fragment_gain_pct
        );
        let _ = writeln!(
            out,
            "Total multiplicative fragment gain: {:.4}x",
            stats.fragment_gain_mult
        );
        let _ = writeln!(
            out,
            "Stamina breakdown: {:.2} base + {:.2} agility + {:.2} upgrades + {:.2} skill = {:.2}",
            stats.stamina_additive_base,
            stats.stamina_agility_bonus,
            stats.stamina_upgrade_bonus,
            stats.stamina_skill_bonus,
            stamina_additive_total
        );
        let _ = writeln!(
            out,
            "Stamina multiplier: (1 + {:.2}% Block Bonker) * (1 + {:.2}% Exp/Stam + {:.2}% Asc1 Max Stam - {:.2}% Corruption) = {:.4}x",
            stats.stamina_block_bonker_pct,
            stats.stamina_exp_legendary_pct,
            stats.stamina_asc1_autotap_pct,
            stats.stamina_corruption_pct,
            stamina_multiplier
        );
        let _ = writeln!(out);
        out.push_str(&format_stat_breakdown(cfg));
    }

    out
}

pub(crate) fn format_recommendations(
    cfg: &Config,
    catalog: &UpgradeCatalog,
    _base: &SimulationResult,
) -> String {
    let objective_label = match cfg.objective {
        Objective::Fragments => match cfg.fragment_objective_currency {
            Some(currency) => format!("{currency} fragments/sec"),
            None => "total fragments/sec".to_string(),
        },
        Objective::Experience => "xp/sec".to_string(),
        Objective::MaxLevel => "max level score".to_string(),
    };
    let base_eval = evaluate_optimizer(cfg);
    let recs = recommendations(cfg, catalog, &base_eval);
    let mut out = String::new();
    if recs.is_empty() {
        let _ = writeln!(
            out,
            "No supported upgrade recommendations were available from the parsed cost tables."
        );
        let _ = writeln!(
            out,
            "Ascension 2 remains under-modeled because the local wiki dump explicitly says those upgrades are missing."
        );
    } else {
        let _ = writeln!(out, "Next-upgrade recommendations");
        let mut by_currency: HashMap<Currency, Vec<Recommendation>> = HashMap::new();
        for rec in recs {
            by_currency.entry(rec.currency).or_default().push(rec);
        }

        let mut currencies: Vec<_> = by_currency.into_iter().collect();
        currencies.sort_by_key(|(currency, _)| *currency);
        for (currency, mut items) in currencies {
            items.sort_by(|a, b| {
                b.roi
                    .partial_cmp(&a.roi)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let _ = writeln!(out, "{currency}:");
            for rec in items.into_iter().take(cfg.top) {
                match rec.time_to_afford {
                    Some(seconds) => {
                        let _ = writeln!(
                            out,
                            "  {:42} lvl {:>2} cost {:>10.0}  delta {:>10.6}  roi {:>10.8}  afford {:>8.1}s",
                            rec.display_name,
                            rec.next_level,
                            rec.next_cost,
                            rec.delta_value,
                            rec.roi,
                            seconds
                        );
                    }
                    None => {
                        let _ = writeln!(
                            out,
                            "  {:42} lvl {:>2} cost {:>10.0}  delta {:>10.6}  roi {:>10.8}",
                            rec.display_name,
                            rec.next_level,
                            rec.next_cost,
                            rec.delta_value,
                            rec.roi
                        );
                    }
                }
            }
        }
    }

    if let Some(skill_result) = optimize_skills(cfg) {
        let _ = writeln!(out);
        let _ = writeln!(out, "Skill optimization");
        let _ = writeln!(out, "  target {objective_label}");
        let _ = writeln!(
            out,
            "  value {:.6} -> {:.6} over {} loops ({} evals, {} sims)",
            skill_result.initial_value,
            skill_result.best_value,
            skill_result.loops,
            skill_result.evaluations,
            skill_result.simulations
        );
        let _ = writeln!(
            out,
            "  each eval averages {} sims; each loop is one full sweep across neighboring skill allocations",
            cfg.optimizer_runs_per_eval.max(1)
        );
        let _ = writeln!(
            out,
            "  start  str {} agi {} per {} int {} luck {} div {} corr {}",
            skill_result.initial_skills.strength,
            skill_result.initial_skills.agility,
            skill_result.initial_skills.perception,
            skill_result.initial_skills.intellect,
            skill_result.initial_skills.luck,
            skill_result.initial_skills.divinity,
            skill_result.initial_skills.corruption
        );
        let _ = writeln!(
            out,
            "  best   str {} agi {} per {} int {} luck {} div {} corr {}",
            skill_result.best_skills.strength,
            skill_result.best_skills.agility,
            skill_result.best_skills.perception,
            skill_result.best_skills.intellect,
            skill_result.best_skills.luck,
            skill_result.best_skills.divinity,
            skill_result.best_skills.corruption
        );
    }

    out
}

fn recommendations(
    cfg: &Config,
    catalog: &UpgradeCatalog,
    base: &OptimizerEvaluation,
) -> Vec<Recommendation> {
    let base_value = base.objective_value;
    let mut out = Vec::new();

    for spec in UPGRADE_SPECS.iter().copied().filter(|s| s.supported) {
        let Some(table) = catalog.tables.get(spec.id) else {
            continue;
        };
        let current = cfg.upgrades.get(spec.id).copied().unwrap_or(0);
        let max_level = explicit_upgrade_max_level(spec.id).unwrap_or(table.costs.len() as u32);
        if current >= max_level || current as usize >= table.costs.len() {
            continue;
        }

        let mut next_cfg = cfg.clone();
        next_cfg.upgrades.insert(spec.id.to_string(), current + 1);
        let next_eval = evaluate_optimizer(&next_cfg);
        let next_value = next_eval.objective_value;
        let delta = (next_value - base_value).max(0.0);
        if delta <= 0.0 {
            continue;
        }

        let cost = table.costs[current as usize];
        let balance = cfg.balances.get(&table.currency).copied().unwrap_or(0.0);
        let remaining = (cost - balance).max(0.0);
        let rate = base
            .fragment_per_second
            .get(&table.currency)
            .copied()
            .unwrap_or(0.0);
        out.push(Recommendation {
            display_name: spec.caption,
            currency: table.currency,
            next_level: current + 1,
            next_cost: cost,
            delta_value: delta,
            roi: delta / cost.max(1.0),
            time_to_afford: if table.currency == Currency::Gems || rate <= 0.0 {
                None
            } else {
                Some(remaining / rate)
            },
        });
    }

    out
}

fn sheet_damage(cfg: &Config, _stats: &DerivedStats) -> f64 {
    flat_damage_from_config(cfg)
        * (1.0 + additive_damage_pct_from_config(cfg) / 100.0)
        * (1.0 + block_bonker_stage_bonus_pct(cfg) / 100.0)
        * SHEET_DAMAGE_CALIBRATION
}

fn sheet_super_crit_damage_mult(cfg: &Config) -> f64 {
    let level = |id: &str| cfg.upgrades.get(id).copied().unwrap_or(0) as f64;
    let pct = 2.0 * level("crit_super_damage") + 0.50 * level("asc1_supercrit_expmod");
    ((2.5 + pct / 100.0) / 0.05).ceil() * 0.05
}
