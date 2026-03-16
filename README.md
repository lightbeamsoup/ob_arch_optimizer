# Idle Obelisk Archaeology Simulator

This is a Rust archaeology simulator and optimizer for Idle Obelisk Miner. It ships with a compact extracted upgrade-cost data file and uses a pragmatic archaeology run model to estimate:

- full-run progression from stage 1 until stamina depletion
- XP per run and XP per second
- fragment income by rarity per run and per second
- the next best supported upgrade for the chosen objective
- a simple desktop GUI for editing state and running sim/optimizer

## Usage

Preferred:

```bash
cargo run -- gui
```

If you want to load a specific state file instead of relying on GUI auto-load:

```bash
cargo run -- gui --load-state archaeology_state.txt
```

CLI examples:

```bash
cargo run -- simulate --stage-cap 75 --highest-stage 42 --strength 30 --agility 20 --perception 25 --intellect 25 --luck 25 --upgrade unlock_ability=2
cargo run -- optimize --stage-cap 75 --strength 30 --agility 20 --perception 25 --intellect 25 --luck 25 --upgrade unlock_ability=2 --upgrade fragment_gain_gems=10 --top 5
cargo run -- optimize --objective max_level --stage-cap 75 --strength 30 --agility 20 --perception 25 --intellect 25 --luck 25 --top 5
cargo run -- optimize --objective fragments --fragment-objective mythic --stage-cap 75 --load-state archaeology_state.txt
cargo run -- save --save-state archaeology_state.txt --stage-cap 75 --strength 30 --agility 20 --perception 25 --intellect 25 --luck 25 --upgrade unlock_ability=2
cargo run -- simulate --load-state archaeology_state.txt --upgrade flat_damage_common=10
cargo run -- simulate --stage-cap 75 --ascension 1 --highest-stage 42 --arch-shop-bundle true --block-bonker true --avada-keda true --glimmering-geoduck true --mythic-chests 300 --strength 30 --agility 20 --perception 25 --intellect 25 --luck 25 --upgrade unlock_ability=3
cargo run -- simulate --stage-cap 75 --card dirt.1=polychrome --card common.3=gilded --upgrade poly_archaeology_card_bonus=1
cargo run -- simulate --stage-cap 75 --axolotl-pet-quest true --axolotl-pet-level 11 --hestia-idol-level 3000 --strength 30 --agility 20 --perception 25 --intellect 25 --luck 25
cargo run -- gui --load-state archaeology_state.txt
cargo run -- gui --no-auto-load
```

## State File

The CLI supports a simple text save format:

```text
stage_cap=75
ascension=1
highest_stage_reached=42
objective=fragments
fragment_objective=mythic
arch_shop_bundle=true
block_bonker=true
avada_keda=true
axolotl_pet_quest_unlocked=true
axolotl_pet_level=11
hestia_idol_level=3000
glimmering_geoduck=true
mythic_chests_owned=300
strength=30
agility=20
perception=25
intellect=25
luck=25
upgrade.unlock_ability=2
upgrade.fragment_gain_gems=10
balance.common=50
balance.rare=100
card.dirt.1=polychrome
card.common.3=gilded
```

## Notes

- Upgrade costs now ship in a compact extracted data file: [upgrade_costs.json](/home/jbk/arch_scratch/data/upgrade_costs.json).
- The loader for that data lives in [cost_data.rs](/home/jbk/arch_scratch/src/cost_data.rs).
- Credit: the embedded cost data and the block/spawn data were originally derived from the Idle Obelisk Miner archaeology wiki and the local HTML dumps used during initial extraction.
- Divine normal-block spawn rates at stage `50+` are currently estimated from the late Mythic trend and should be treated as provisional until real data is available.
- The simulator now models the core archaeology loop as a run that starts at stage 1 and progresses upward until stamina runs out.
- The run model now uses explicit attack-by-attack autocasting for Enrage, Whirlwind, and Quake instead of only uptime averaging:
  - Enrage is cast on cooldown and applies its buff for the configured number of attacks
  - Whirlwind is cast on cooldown, doubles attack speed for its active attacks, and refunds stamina on cast
  - Quake is cast on cooldown and each active attack splashes damage onto every live block on the current stage
- Optimization objectives now support `fragments`, `experience`, and `max_level`.
- For fragment optimization, you can target all fragments together or a specific fragment currency with `fragment_objective=all|common|rare|epic|legendary|mythic|divine`.
- `cargo run -- gui` is the preferred way to use the project.
- The GUI edits the same state-file fields, balances, upgrades, and cards, and includes live stats, batch sim controls, and the optimizer trace.
- The GUI auto-loads `archaeology_state.txt` on startup if it exists, unless you pass `--no-auto-load`.
- The optimizer trace in the GUI shows the active target, plots objective value by evaluation, and lets you apply the best discovered skill allocation back into the current build when the run finishes.
- The state/config file also supports `sheet_speed_mod_always_active=true|false`.
  - This is a feedback-only toggle used by `simulate` and the GUI to double displayed attack rate and all per-second rates when speed mod is effectively permanent.
  - It does not change per-run rewards or optimizer scoring.
- The state/config file now supports extra account bonuses:
  - `arch_shop_bundle`: `1.25x` fragment gain
  - `block_bonker`: `+1% damage` and `+1% max stamina` per highest stage, using highest stage capped at `100`, plus a flat `+15 speed mod gain`
  - `avada_keda`: `+5` ability duration, `-10s` ability cooldown, `+3%` ability instacharge
  - `axolotl_pet_quest_unlocked` with `axolotl_pet_level`: `+3%` fragment gain when unlocked, plus `+3%` per pet level, capped at level `11`
  - `hestia_idol_level`: `+0.01%` fragment gain per idol level, capped at `3000`
  - `glimmering_geoduck` with `mythic_chests_owned`
- The state/config file also supports archaeology block cards:
  - `card.<rarity>.<tier>=standard|gilded|polychrome|infernal`
  - Example: `card.dirt.1=polychrome`
  - Card effects are modeled as `-X% health` and `+X% exp/loot`
  - `standard=10%`, `gilded=20%`, `polychrome=35%` or `50%` if `Polychrome Archaeology Card Bonus +15%` is owned
  - `infernal` is treated the same as `polychrome` for now because archaeology infernals are not implemented yet
- Simulation output now prints the active global fragment bonuses and the configured cards, including their effective percentages, so saved configs are easier to sanity-check.
- If you run `simulate` or `optimize` with `--save-state`, the stored `highest_stage_reached` is automatically increased when the simulation beats it.
- Critical hits are modeled hierarchically: crit is checked first, then super crit after a successful crit, then ultra crit after a successful super crit.
- The wiki still leaves some archaeology behavior ambiguous. Some timing, proc, and exact in-game display rules are still modeled heuristically and should be treated as directional rather than frame-perfect.
- The optimizer only scores upgrades that materially affect archaeology farming in this model. Cosmetic or off-loop upgrades are ignored.
- Ascension 2 is still incomplete because the local wiki dump explicitly says those upgrades are missing.

## TODO

- Replace the provisional stage `50+` Divine normal-spawn weights with real measured data.
- Fill in missing Ascension 2 archaeology upgrades and verify their exact formulas.
- Add regression tests for saved-state stat sheets, optimizer evaluations, and representative full runs.
- Improve GUI polish for larger configs:
  - better grouping/filtering for upgrades
  - clearer optimizer summaries and graph labeling
- Add import/export helpers for sharing builds more easily.

## Contributing

If you are new to Rust, the shortest path to a working local build is:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
cargo build
cargo run -- simulate --load-state archaeology_state.txt
```

Useful commands while working on the project:

```bash
cargo build
cargo fmt
cargo run -- gui
cargo run -- simulate --load-state archaeology_state.txt
cargo run -- optimize --load-state archaeology_state.txt
```

If you want to start a fresh repository here:

```bash
git init
git add .
git commit -m "Initial archaeology simulator"
```

Contribution targets that would be especially helpful:

- replace guessed Divine spawn rates with measured in-game data
- verify exact archaeology formulas where the model still uses heuristics
- expand Ascension 2 coverage
- add regression tests for representative saves and runs
