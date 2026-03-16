use std::env;

mod cost_data;
mod game_data;
mod gui;
mod presentation;
mod sim;
mod state;
mod types;

use gui::launch_gui;
use presentation::{format_recommendations, format_simulation};
use sim::{simulate_run, simulate_run_internal};
use state::{Config, parse_args, print_help, save_state};
use types::SimulationResult;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() == 1 {
        print_help();
        return Ok(());
    }

    let command = args[1].as_str();
    if command == "gui" {
        let cfg = parse_args(&args[2..])?;
        launch_gui(cfg)?;
        return Ok(());
    }

    let cfg = parse_args(&args[2..])?;
    match command {
        "simulate" => {
            let result = simulate_run(&cfg);
            print!("{}", format_simulation(&cfg, &result));
            maybe_save_updated_state(&cfg, &result)?;
        }
        "optimize" => {
            let catalog = cost_data::built_in_upgrade_catalog();
            let result = simulate_run(&cfg);
            let optimize_result = simulate_run_internal(&cfg, false);
            print!("{}", format_simulation(&cfg, &result));
            println!();
            print!(
                "{}",
                format_recommendations(&cfg, &catalog, &optimize_result)
            );
            maybe_save_updated_state(&cfg, &result)?;
        }
        "save" => {
            let Some(path) = &cfg.save_state else {
                return Err("`save` requires `--save-state PATH`".to_string());
            };
            save_state(path, &cfg)?;
            println!("Saved state to {}", path.display());
        }
        _ => return Err(format!("unknown command `{command}`")),
    }

    Ok(())
}

fn maybe_save_updated_state(cfg: &Config, result: &SimulationResult) -> Result<(), String> {
    let Some(path) = &cfg.save_state else {
        return Ok(());
    };
    let mut updated = cfg.clone();
    updated.highest_stage_reached = result.updated_highest_stage.max(cfg.highest_stage_reached);
    save_state(path, &updated)
}
