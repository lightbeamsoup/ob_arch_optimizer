// Upgrade cost data derived from the Idle Obelisk Miner archaeology wiki dumps.
use std::collections::HashMap;

use serde::Deserialize;

use crate::game_data::Currency;
use crate::types::{UpgradeCatalog, UpgradeCostTable};

#[derive(Deserialize)]
struct RawUpgradeCostTable {
    currency: String,
    costs: Vec<f64>,
}

pub fn built_in_upgrade_catalog() -> UpgradeCatalog {
    let raw_tables: HashMap<String, RawUpgradeCostTable> =
        serde_json::from_str(include_str!("../data/upgrade_costs.json"))
            .expect("embedded upgrade cost data must parse");

    let mut tables = HashMap::with_capacity(raw_tables.len());
    for (id, raw) in raw_tables {
        let currency = Currency::parse(&raw.currency)
            .unwrap_or_else(|| panic!("unknown embedded currency for {id}: {}", raw.currency));
        tables.insert(
            id,
            UpgradeCostTable {
                currency,
                costs: raw.costs,
            },
        );
    }

    UpgradeCatalog { tables }
}
