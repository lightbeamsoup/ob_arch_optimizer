use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::game_data::{CardKey, CardQuality, Currency, parse_card_key};

pub(crate) const DEFAULT_STATE_FILE: &str = "archaeology_state.txt";

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Objective {
    Fragments,
    Experience,
    MaxLevel,
}

impl Objective {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "fragments" => Some(Self::Fragments),
            "xp" | "experience" => Some(Self::Experience),
            "max_level" | "maxlevel" | "level" => Some(Self::MaxLevel),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Fragments => "Fragments",
            Self::Experience => "Experience",
            Self::MaxLevel => "Max Level",
        }
    }
}

impl fmt::Display for Objective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fragments => f.write_str("fragments"),
            Self::Experience => f.write_str("experience"),
            Self::MaxLevel => f.write_str("max_level"),
        }
    }
}

#[derive(Default, Clone)]
pub(crate) struct SkillAllocation {
    pub(crate) strength: u32,
    pub(crate) agility: u32,
    pub(crate) perception: u32,
    pub(crate) intellect: u32,
    pub(crate) luck: u32,
    pub(crate) divinity: u32,
    pub(crate) corruption: u32,
}

#[derive(Clone)]
pub(crate) struct Config {
    pub(crate) archaeology_level: u32,
    pub(crate) stage_cap: u32,
    pub(crate) ascension: u32,
    pub(crate) highest_stage_reached: u32,
    pub(crate) base_crosshair_chance: f64,
    pub(crate) sheet_speed_mod_always_active: bool,
    pub(crate) verbose_output: bool,
    pub(crate) arch_shop_bundle: bool,
    pub(crate) arch_asc_shop_bundle: bool,
    pub(crate) block_bonker: bool,
    pub(crate) avada_keda: bool,
    pub(crate) axolotl_pet_quest_unlocked: bool,
    pub(crate) axolotl_pet_level: u32,
    pub(crate) hestia_idol_level: u32,
    pub(crate) glimmering_geoduck: bool,
    pub(crate) mythic_chests_owned: u32,
    pub(crate) objective: Objective,
    pub(crate) fragment_objective_currency: Option<Currency>,
    pub(crate) optimize_skills: bool,
    pub(crate) optimizer_runs_per_eval: u32,
    pub(crate) optimizer_convergence_pct: f64,
    pub(crate) optimizer_max_loops: u32,
    pub(crate) top: usize,
    pub(crate) load_state: Option<PathBuf>,
    pub(crate) save_state: Option<PathBuf>,
    pub(crate) no_auto_load: bool,
    pub(crate) skills: SkillAllocation,
    pub(crate) upgrades: HashMap<String, u32>,
    pub(crate) balances: HashMap<Currency, f64>,
    pub(crate) cards: HashMap<CardKey, CardQuality>,
}

pub(crate) fn print_help() {
    println!("Idle Obelisk Miner archaeology simulator/optimizer");
    println!();
    println!("Commands:");
    println!("  cargo run -- simulate [options]");
    println!("  cargo run -- optimize [options]");
    println!("  cargo run -- gui [options]");
    println!("  cargo run -- save --save-state state.txt [options]");
    println!();
    println!("Core options:");
    println!("  --level N                   Archaeology level used for skill point total");
    println!(
        "  --stage-cap N               Simulate progression from stage 1 up to depletion or this cap"
    );
    println!("  --ascension N               Used for capped tribute bonuses");
    println!("  --highest-stage N           Stored highest archaeology stage reached");
    println!(
        "  --base-crosshair-chance N   Base crosshair spawn chance percent for the first-pass model"
    );
    println!(
        "  --sheet-speed-mod-always-active true|false  Display Atk Speed as 2/sec in Print Stats"
    );
    println!("  --verbose-output true|false   Show detailed sim diagnostics");
    println!("  --objective fragments|xp|max_level");
    println!("  --fragment-objective all|common|rare|epic|legendary|mythic|divine");
    println!("  --optimize-skills true|false");
    println!("  --optimizer-runs-per-eval N");
    println!("  --optimizer-convergence-pct N");
    println!("  --optimizer-max-loops N");
    println!("  --strength N --agility N --perception N --intellect N --luck N");
    println!("  --divinity N --corruption N");
    println!("  --arch-shop-bundle true|false");
    println!("  --arch-asc-shop-bundle true|false");
    println!("  --block-bonker true|false");
    println!("  --avada-keda true|false");
    println!("  --axolotl-pet-quest true|false --axolotl-pet-level N");
    println!("  --hestia-idol-level N");
    println!("  --glimmering-geoduck true|false --mythic-chests N");
    println!("  --upgrade id=level          Repeatable");
    println!("  --balance currency=value    Repeatable current balance for time-to-afford");
    println!("  --card rarity.tier=quality  Example: --card dirt.1=polychrome");
    println!("  --load-state PATH           Load a saved build/state file");
    println!("  --save-state PATH           Save the effective build/state file");
    println!("  --no-auto-load              Disable GUI auto-loading of the default state file");
    println!();
    println!("State file format:");
    println!("  key=value");
    println!("  upgrade.some_id=12");
    println!("  balance.common=12345");
}

pub(crate) fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut cfg = Config {
        stage_cap: 150,
        ascension: 0,
        highest_stage_reached: 0,
        base_crosshair_chance: 0.0,
        sheet_speed_mod_always_active: false,
        verbose_output: true,
        arch_shop_bundle: false,
        arch_asc_shop_bundle: false,
        block_bonker: false,
        avada_keda: false,
        axolotl_pet_quest_unlocked: false,
        axolotl_pet_level: 0,
        hestia_idol_level: 0,
        glimmering_geoduck: false,
        mythic_chests_owned: 0,
        objective: Objective::Fragments,
        fragment_objective_currency: None,
        optimize_skills: true,
        optimizer_runs_per_eval: 50,
        optimizer_convergence_pct: 0.10,
        optimizer_max_loops: 20,
        top: 5,
        load_state: None,
        save_state: None,
        no_auto_load: false,
        skills: SkillAllocation::default(),
        upgrades: HashMap::new(),
        balances: HashMap::new(),
        cards: HashMap::new(),
        archaeology_level: 0,
    };

    let mut i = 0usize;
    while i < args.len() {
        let flag = args[i].as_str();
        let next = |i: &mut usize| -> Result<&str, String> {
            *i += 1;
            args.get(*i)
                .map(|s| s.as_str())
                .ok_or_else(|| format!("missing value after `{flag}`"))
        };

        match flag {
            "--level" => cfg.archaeology_level = parse_u32(next(&mut i)?)?,
            "--stage" | "--stage-cap" => cfg.stage_cap = parse_u32(next(&mut i)?)?,
            "--ascension" => cfg.ascension = parse_u32(next(&mut i)?)?,
            "--highest-stage" => cfg.highest_stage_reached = parse_u32(next(&mut i)?)?,
            "--base-crosshair-chance" => cfg.base_crosshair_chance = parse_f64(next(&mut i)?)?,
            "--sheet-speed-mod-always-active" => {
                cfg.sheet_speed_mod_always_active = parse_bool_flag(next(&mut i)?)?
            }
            "--verbose-output" => cfg.verbose_output = parse_bool_flag(next(&mut i)?)?,
            "--strength" => cfg.skills.strength = parse_u32(next(&mut i)?)?,
            "--agility" => cfg.skills.agility = parse_u32(next(&mut i)?)?,
            "--perception" => cfg.skills.perception = parse_u32(next(&mut i)?)?,
            "--intellect" => cfg.skills.intellect = parse_u32(next(&mut i)?)?,
            "--luck" => cfg.skills.luck = parse_u32(next(&mut i)?)?,
            "--divinity" => cfg.skills.divinity = parse_u32(next(&mut i)?)?,
            "--corruption" => cfg.skills.corruption = parse_u32(next(&mut i)?)?,
            "--objective" => {
                cfg.objective = Objective::parse(next(&mut i)?).ok_or_else(|| {
                    "objective must be `fragments`, `xp`, or `max_level`".to_string()
                })?;
            }
            "--fragment-objective" => {
                let raw = next(&mut i)?;
                cfg.fragment_objective_currency = if raw.eq_ignore_ascii_case("all") {
                    None
                } else {
                    Some(
                        Currency::parse(raw).ok_or_else(|| {
                            "fragment objective must be `all`, `common`, `rare`, `epic`, `legendary`, `mythic`, or `divine`".to_string()
                        })?,
                    )
                };
            }
            "--optimize-skills" => cfg.optimize_skills = parse_bool_flag(next(&mut i)?)?,
            "--optimizer-runs-per-eval" => cfg.optimizer_runs_per_eval = parse_u32(next(&mut i)?)?,
            "--optimizer-convergence-pct" => {
                cfg.optimizer_convergence_pct = parse_f64(next(&mut i)?)?
            }
            "--optimizer-max-loops" => cfg.optimizer_max_loops = parse_u32(next(&mut i)?)?,
            "--top" => cfg.top = parse_u32(next(&mut i)?)? as usize,
            "--load-state" => cfg.load_state = Some(PathBuf::from(next(&mut i)?)),
            "--save-state" => cfg.save_state = Some(PathBuf::from(next(&mut i)?)),
            "--no-auto-load" => cfg.no_auto_load = true,
            "--arch-shop-bundle" => cfg.arch_shop_bundle = parse_bool_flag(next(&mut i)?)?,
            "--arch-asc-shop-bundle" => cfg.arch_asc_shop_bundle = parse_bool_flag(next(&mut i)?)?,
            "--block-bonker" => cfg.block_bonker = parse_bool_flag(next(&mut i)?)?,
            "--avada-keda" => cfg.avada_keda = parse_bool_flag(next(&mut i)?)?,
            "--axolotl-pet-quest" => {
                cfg.axolotl_pet_quest_unlocked = parse_bool_flag(next(&mut i)?)?
            }
            "--axolotl-pet-level" => cfg.axolotl_pet_level = parse_u32(next(&mut i)?)?,
            "--hestia-idol-level" => cfg.hestia_idol_level = parse_u32(next(&mut i)?)?,
            "--glimmering-geoduck" => cfg.glimmering_geoduck = parse_bool_flag(next(&mut i)?)?,
            "--mythic-chests" => cfg.mythic_chests_owned = parse_u32(next(&mut i)?)?,
            "--upgrade" => {
                let raw = next(&mut i)?;
                let (name, level) = raw
                    .split_once('=')
                    .ok_or_else(|| format!("upgrade must look like id=level, got `{raw}`"))?;
                cfg.upgrades.insert(name.to_string(), parse_u32(level)?);
            }
            "--balance" => {
                let raw = next(&mut i)?;
                let (currency, amount) = raw
                    .split_once('=')
                    .ok_or_else(|| format!("balance must look like currency=value, got `{raw}`"))?;
                let currency = Currency::parse(currency)
                    .ok_or_else(|| format!("unknown currency `{currency}`"))?;
                cfg.balances.insert(currency, parse_f64(amount)?);
            }
            "--card" => {
                let raw = next(&mut i)?;
                let (key, quality) = raw.split_once('=').ok_or_else(|| {
                    format!("card must look like rarity.tier=quality, got `{raw}`")
                })?;
                let key = parse_card_key(key).ok_or_else(|| format!("invalid card key `{key}`"))?;
                let quality = CardQuality::parse(quality)
                    .ok_or_else(|| format!("invalid card quality `{quality}`"))?;
                cfg.cards.insert(key, quality);
            }
            other => return Err(format!("unknown flag `{other}`")),
        }
        i += 1;
    }

    if let Some(path) = cfg.load_state.clone() {
        load_state_into(&path, &mut cfg)?;
    }

    normalize_config(&mut cfg);

    Ok(cfg)
}

pub(crate) fn load_state_into(path: &Path, cfg: &mut Config) -> Result<(), String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "archaeology_level" => cfg.archaeology_level = parse_u32(value)?,
            "stage_cap" => cfg.stage_cap = parse_u32(value)?,
            "ascension" => cfg.ascension = parse_u32(value)?,
            "highest_stage_reached" => cfg.highest_stage_reached = parse_u32(value)?,
            "base_crosshair_chance" => cfg.base_crosshair_chance = parse_f64(value)?,
            "sheet_speed_mod_always_active" => {
                cfg.sheet_speed_mod_always_active = parse_bool_flag(value)?
            }
            "verbose_output" => cfg.verbose_output = parse_bool_flag(value)?,
            "objective" => {
                cfg.objective = Objective::parse(value)
                    .ok_or_else(|| format!("invalid objective `{value}` in {}", path.display()))?;
            }
            "fragment_objective" => {
                cfg.fragment_objective_currency = if value.eq_ignore_ascii_case("all") {
                    None
                } else {
                    Some(Currency::parse(value).ok_or_else(|| {
                        format!("invalid fragment objective `{value}` in {}", path.display())
                    })?)
                };
            }
            "optimize_skills" => cfg.optimize_skills = parse_bool_flag(value)?,
            "optimizer_runs_per_eval" => cfg.optimizer_runs_per_eval = parse_u32(value)?,
            "optimizer_convergence_pct" => cfg.optimizer_convergence_pct = parse_f64(value)?,
            "optimizer_max_loops" => cfg.optimizer_max_loops = parse_u32(value)?,
            "top" => cfg.top = parse_u32(value)? as usize,
            "arch_shop_bundle" => cfg.arch_shop_bundle = parse_bool_flag(value)?,
            "arch_asc_shop_bundle" => cfg.arch_asc_shop_bundle = parse_bool_flag(value)?,
            "block_bonker" => cfg.block_bonker = parse_bool_flag(value)?,
            "avada_keda" => cfg.avada_keda = parse_bool_flag(value)?,
            "axolotl_pet_quest_unlocked" => {
                cfg.axolotl_pet_quest_unlocked = parse_bool_flag(value)?
            }
            "axolotl_pet_level" => cfg.axolotl_pet_level = parse_u32(value)?,
            "hestia_idol_level" => cfg.hestia_idol_level = parse_u32(value)?,
            "glimmering_geoduck" => cfg.glimmering_geoduck = parse_bool_flag(value)?,
            "mythic_chests_owned" => cfg.mythic_chests_owned = parse_u32(value)?,
            "strength" => cfg.skills.strength = parse_u32(value)?,
            "agility" => cfg.skills.agility = parse_u32(value)?,
            "perception" => cfg.skills.perception = parse_u32(value)?,
            "intellect" => cfg.skills.intellect = parse_u32(value)?,
            "luck" => cfg.skills.luck = parse_u32(value)?,
            "divinity" => cfg.skills.divinity = parse_u32(value)?,
            "corruption" => cfg.skills.corruption = parse_u32(value)?,
            _ if key.starts_with("upgrade.") => {
                cfg.upgrades
                    .insert(key["upgrade.".len()..].to_string(), parse_u32(value)?);
            }
            _ if key.starts_with("balance.") => {
                let currency = Currency::parse(&key["balance.".len()..])
                    .ok_or_else(|| format!("invalid balance key `{key}`"))?;
                cfg.balances.insert(currency, parse_f64(value)?);
            }
            _ if key.starts_with("card.") => {
                let card_key = parse_card_key(&key["card.".len()..])
                    .ok_or_else(|| format!("invalid card key `{key}`"))?;
                let quality = CardQuality::parse(value)
                    .ok_or_else(|| format!("invalid card quality `{value}`"))?;
                cfg.cards.insert(card_key, quality);
            }
            _ => {}
        }
    }
    normalize_config(cfg);
    Ok(())
}

pub(crate) fn save_state(path: &Path, cfg: &Config) -> Result<(), String> {
    let mut lines = vec![
        format!("archaeology_level={}", cfg.archaeology_level),
        format!("stage_cap={}", cfg.stage_cap),
        format!("ascension={}", cfg.ascension),
        format!("highest_stage_reached={}", cfg.highest_stage_reached),
        format!("base_crosshair_chance={}", cfg.base_crosshair_chance),
        format!(
            "sheet_speed_mod_always_active={}",
            cfg.sheet_speed_mod_always_active
        ),
        format!("verbose_output={}", cfg.verbose_output),
        format!("objective={}", cfg.objective),
        format!(
            "fragment_objective={}",
            cfg.fragment_objective_currency
                .map(|currency| currency.to_string())
                .unwrap_or_else(|| "all".to_string())
        ),
        format!("optimize_skills={}", cfg.optimize_skills),
        format!("optimizer_runs_per_eval={}", cfg.optimizer_runs_per_eval),
        format!(
            "optimizer_convergence_pct={}",
            cfg.optimizer_convergence_pct
        ),
        format!("optimizer_max_loops={}", cfg.optimizer_max_loops),
        format!("top={}", cfg.top),
        format!("arch_shop_bundle={}", cfg.arch_shop_bundle),
        format!("arch_asc_shop_bundle={}", cfg.arch_asc_shop_bundle),
        format!("block_bonker={}", cfg.block_bonker),
        format!("avada_keda={}", cfg.avada_keda),
        format!(
            "axolotl_pet_quest_unlocked={}",
            cfg.axolotl_pet_quest_unlocked
        ),
        format!("axolotl_pet_level={}", cfg.axolotl_pet_level),
        format!("hestia_idol_level={}", cfg.hestia_idol_level),
        format!("glimmering_geoduck={}", cfg.glimmering_geoduck),
        format!("mythic_chests_owned={}", cfg.mythic_chests_owned),
        format!("strength={}", cfg.skills.strength),
        format!("agility={}", cfg.skills.agility),
        format!("perception={}", cfg.skills.perception),
        format!("intellect={}", cfg.skills.intellect),
        format!("luck={}", cfg.skills.luck),
        format!("divinity={}", cfg.skills.divinity),
        format!("corruption={}", cfg.skills.corruption),
    ];

    let mut upgrades: Vec<_> = cfg.upgrades.iter().collect();
    upgrades.sort_by(|a, b| a.0.cmp(b.0));
    for (name, level) in upgrades {
        lines.push(format!("upgrade.{name}={level}"));
    }

    let mut balances: Vec<_> = cfg.balances.iter().collect();
    balances.sort_by_key(|(currency, _)| **currency);
    for (currency, amount) in balances {
        lines.push(format!("balance.{currency}={amount:.6}"));
    }

    let mut cards: Vec<_> = cfg.cards.iter().collect();
    cards.sort_by(|a, b| {
        a.0.rarity
            .name()
            .cmp(b.0.rarity.name())
            .then(a.0.tier.cmp(&b.0.tier))
    });
    for (key, quality) in cards {
        lines.push(format!(
            "card.{}.{}={}",
            key.rarity.name(),
            key.tier,
            quality.as_str()
        ));
    }

    fs::write(path, lines.join("\n") + "\n")
        .map_err(|e| format!("failed to write {}: {e}", path.display()))
}

fn parse_u32(s: &str) -> Result<u32, String> {
    s.parse::<u32>()
        .map_err(|e| format!("invalid integer `{s}`: {e}"))
}

fn parse_bool_flag(s: &str) -> Result<bool, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!("invalid boolean `{s}`")),
    }
}

fn parse_f64(s: &str) -> Result<f64, String> {
    s.parse::<f64>()
        .map_err(|e| format!("invalid number `{s}`: {e}"))
}

fn normalize_config(cfg: &mut Config) {
    if cfg.ascension < 2 {
        cfg.skills.corruption = 0;
    }
}
