use std::fmt;

pub(crate) const BLOCKS_PER_STAGE: f64 = 24.0;
pub(crate) const BASE_STAMINA: f64 = 100.0;
pub(crate) const BASE_DAMAGE: f64 = 10.0;
pub(crate) const BASE_STAMINA_MOD_GAIN: f64 = 3.0;
pub(crate) const BASE_SPEED_MOD_GAIN: f64 = 10.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub(crate) enum Currency {
    Gems,
    Common,
    Rare,
    Epic,
    Legendary,
    Mythic,
    Divine,
}

impl Currency {
    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "gems" | "gem" => Some(Self::Gems),
            "common" => Some(Self::Common),
            "rare" => Some(Self::Rare),
            "epic" => Some(Self::Epic),
            "legendary" => Some(Self::Legendary),
            "mythic" => Some(Self::Mythic),
            "divine" => Some(Self::Divine),
            _ => None,
        }
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Gems => "gems",
            Self::Common => "common",
            Self::Rare => "rare",
            Self::Epic => "epic",
            Self::Legendary => "legendary",
            Self::Mythic => "mythic",
            Self::Divine => "divine",
        };
        f.write_str(s)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum Rarity {
    Dirt,
    Common,
    Rare,
    Epic,
    Legendary,
    Mythic,
    Divine,
}

impl Rarity {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Dirt => "dirt",
            Self::Common => "common",
            Self::Rare => "rare",
            Self::Epic => "epic",
            Self::Legendary => "legendary",
            Self::Mythic => "mythic",
            Self::Divine => "divine",
        }
    }

    pub(crate) fn fragment_currency(self) -> Option<Currency> {
        match self {
            Self::Dirt => None,
            Self::Common => Some(Currency::Common),
            Self::Rare => Some(Currency::Rare),
            Self::Epic => Some(Currency::Epic),
            Self::Legendary => Some(Currency::Legendary),
            Self::Mythic => Some(Currency::Mythic),
            Self::Divine => Some(Currency::Divine),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) struct CardKey {
    pub(crate) rarity: Rarity,
    pub(crate) tier: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CardQuality {
    None,
    Standard,
    Gilded,
    Polychrome,
    Infernal,
}

impl CardQuality {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "standard" | "regular" => Some(Self::Standard),
            "gilded" => Some(Self::Gilded),
            "polychrome" | "poly" => Some(Self::Polychrome),
            "infernal" => Some(Self::Infernal),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Standard => "standard",
            Self::Gilded => "gilded",
            Self::Polychrome => "polychrome",
            Self::Infernal => "infernal",
        }
    }
}

pub(crate) fn parse_card_key(value: &str) -> Option<CardKey> {
    let mut parts = value.split('.');
    let rarity = match parts.next()?.to_ascii_lowercase().as_str() {
        "dirt" => Rarity::Dirt,
        "common" => Rarity::Common,
        "rare" => Rarity::Rare,
        "epic" => Rarity::Epic,
        "legendary" => Rarity::Legendary,
        "mythic" => Rarity::Mythic,
        "divine" => Rarity::Divine,
        _ => return None,
    };
    let tier = parts.next()?.parse::<u8>().ok()?;
    if !(1..=4).contains(&tier) || parts.next().is_some() {
        return None;
    }
    Some(CardKey { rarity, tier })
}

#[derive(Clone, Copy)]
pub(crate) struct BlockTier {
    pub(crate) rarity: Rarity,
    #[allow(dead_code)]
    pub(crate) tier: u8,
    pub(crate) unlock_wave: u32,
    pub(crate) hp: f64,
    pub(crate) armor: f64,
    pub(crate) xp: f64,
    pub(crate) fragments: f64,
    pub(crate) hp100: f64,
    pub(crate) armor100: f64,
    pub(crate) hp150: f64,
    pub(crate) armor150: f64,
}

pub(crate) const BLOCK_TIERS: &[BlockTier] = &[
    BlockTier {
        rarity: Rarity::Dirt,
        tier: 1,
        unlock_wave: 1,
        hp: 100.0,
        armor: 0.0,
        xp: 0.05,
        fragments: 0.0,
        hp100: 200.0,
        armor100: 0.0,
        hp150: 400.0,
        armor150: 0.0,
    },
    BlockTier {
        rarity: Rarity::Dirt,
        tier: 2,
        unlock_wave: 12,
        hp: 300.0,
        armor: 0.0,
        xp: 0.15,
        fragments: 0.0,
        hp100: 600.0,
        armor100: 0.0,
        hp150: 1200.0,
        armor150: 0.0,
    },
    BlockTier {
        rarity: Rarity::Dirt,
        tier: 3,
        unlock_wave: 24,
        hp: 900.0,
        armor: 0.0,
        xp: 0.45,
        fragments: 0.0,
        hp100: 1800.0,
        armor100: 0.0,
        hp150: 3600.0,
        armor150: 0.0,
    },
    BlockTier {
        rarity: Rarity::Dirt,
        tier: 4,
        unlock_wave: 81,
        hp: 2700.0,
        armor: 0.0,
        xp: 1.35,
        fragments: 0.0,
        hp100: 5400.0,
        armor100: 0.0,
        hp150: 10800.0,
        armor150: 0.0,
    },
    BlockTier {
        rarity: Rarity::Common,
        tier: 1,
        unlock_wave: 1,
        hp: 250.0,
        armor: 5.0,
        xp: 0.15,
        fragments: 0.01,
        hp100: 500.0,
        armor100: 7.5,
        hp150: 1000.0,
        armor150: 7.5,
    },
    BlockTier {
        rarity: Rarity::Common,
        tier: 2,
        unlock_wave: 18,
        hp: 750.0,
        armor: 8.25,
        xp: 0.45,
        fragments: 0.02,
        hp100: 1500.0,
        armor100: 12.38,
        hp150: 3000.0,
        armor150: 12.38,
    },
    BlockTier {
        rarity: Rarity::Common,
        tier: 3,
        unlock_wave: 30,
        hp: 2250.0,
        armor: 13.61,
        xp: 1.35,
        fragments: 0.04,
        hp100: 4500.0,
        armor100: 20.42,
        hp150: 9000.0,
        armor150: 20.42,
    },
    BlockTier {
        rarity: Rarity::Common,
        tier: 4,
        unlock_wave: 96,
        hp: 6750.0,
        armor: 22.46,
        xp: 4.05,
        fragments: 0.08,
        hp100: 13500.0,
        armor100: 33.69,
        hp150: 27000.0,
        armor150: 33.69,
    },
    BlockTier {
        rarity: Rarity::Rare,
        tier: 1,
        unlock_wave: 3,
        hp: 550.0,
        armor: 12.0,
        xp: 0.35,
        fragments: 0.01,
        hp100: 1100.0,
        armor100: 18.0,
        hp150: 2200.0,
        armor150: 18.0,
    },
    BlockTier {
        rarity: Rarity::Rare,
        tier: 2,
        unlock_wave: 26,
        hp: 1650.0,
        armor: 19.8,
        xp: 1.05,
        fragments: 0.02,
        hp100: 3300.0,
        armor100: 29.7,
        hp150: 6600.0,
        armor150: 29.7,
    },
    BlockTier {
        rarity: Rarity::Rare,
        tier: 3,
        unlock_wave: 36,
        hp: 4950.0,
        armor: 32.67,
        xp: 3.15,
        fragments: 0.04,
        hp100: 9900.0,
        armor100: 49.0,
        hp150: 19800.0,
        armor150: 49.0,
    },
    BlockTier {
        rarity: Rarity::Rare,
        tier: 4,
        unlock_wave: 111,
        hp: 14850.0,
        armor: 53.91,
        xp: 9.45,
        fragments: 0.08,
        hp100: 29700.0,
        armor100: 80.86,
        hp150: 59400.0,
        armor150: 80.86,
    },
    BlockTier {
        rarity: Rarity::Epic,
        tier: 1,
        unlock_wave: 6,
        hp: 1150.0,
        armor: 25.0,
        xp: 1.0,
        fragments: 0.01,
        hp100: 2300.0,
        armor100: 37.5,
        hp150: 4600.0,
        armor150: 37.5,
    },
    BlockTier {
        rarity: Rarity::Epic,
        tier: 2,
        unlock_wave: 30,
        hp: 3450.0,
        armor: 41.25,
        xp: 3.0,
        fragments: 0.02,
        hp100: 6900.0,
        armor100: 61.88,
        hp150: 13800.0,
        armor150: 61.88,
    },
    BlockTier {
        rarity: Rarity::Epic,
        tier: 3,
        unlock_wave: 42,
        hp: 10350.0,
        armor: 68.06,
        xp: 9.0,
        fragments: 0.04,
        hp100: 20700.0,
        armor100: 102.09,
        hp150: 41400.0,
        armor150: 102.09,
    },
    BlockTier {
        rarity: Rarity::Epic,
        tier: 4,
        unlock_wave: 126,
        hp: 31050.0,
        armor: 112.3,
        xp: 27.0,
        fragments: 0.08,
        hp100: 62100.0,
        armor100: 168.45,
        hp150: 124200.0,
        armor150: 168.45,
    },
    BlockTier {
        rarity: Rarity::Legendary,
        tier: 1,
        unlock_wave: 12,
        hp: 1950.0,
        armor: 50.0,
        xp: 3.5,
        fragments: 0.01,
        hp100: 3900.0,
        armor100: 75.0,
        hp150: 7800.0,
        armor150: 75.0,
    },
    BlockTier {
        rarity: Rarity::Legendary,
        tier: 2,
        unlock_wave: 32,
        hp: 5850.0,
        armor: 82.5,
        xp: 10.5,
        fragments: 0.02,
        hp100: 11700.0,
        armor100: 123.75,
        hp150: 23400.0,
        armor150: 123.75,
    },
    BlockTier {
        rarity: Rarity::Legendary,
        tier: 3,
        unlock_wave: 45,
        hp: 17550.0,
        armor: 136.12,
        xp: 31.5,
        fragments: 0.04,
        hp100: 35100.0,
        armor100: 204.19,
        hp150: 70200.0,
        armor150: 204.19,
    },
    BlockTier {
        rarity: Rarity::Legendary,
        tier: 4,
        unlock_wave: 136,
        hp: 52650.0,
        armor: 224.61,
        xp: 94.5,
        fragments: 0.08,
        hp100: 105300.0,
        armor100: 336.91,
        hp150: 210600.0,
        armor150: 336.91,
    },
    BlockTier {
        rarity: Rarity::Mythic,
        tier: 1,
        unlock_wave: 20,
        hp: 3500.0,
        armor: 150.0,
        xp: 7.5,
        fragments: 0.01,
        hp100: 7000.0,
        armor100: 225.0,
        hp150: 14000.0,
        armor150: 225.0,
    },
    BlockTier {
        rarity: Rarity::Mythic,
        tier: 2,
        unlock_wave: 35,
        hp: 10500.0,
        armor: 247.5,
        xp: 22.5,
        fragments: 0.02,
        hp100: 21000.0,
        armor100: 371.25,
        hp150: 42000.0,
        armor150: 371.25,
    },
    BlockTier {
        rarity: Rarity::Mythic,
        tier: 3,
        unlock_wave: 50,
        hp: 31500.0,
        armor: 408.37,
        xp: 67.5,
        fragments: 0.04,
        hp100: 63000.0,
        armor100: 612.56,
        hp150: 126000.0,
        armor150: 612.56,
    },
    BlockTier {
        rarity: Rarity::Mythic,
        tier: 4,
        unlock_wave: 141,
        hp: 94500.0,
        armor: 673.82,
        xp: 202.5,
        fragments: 0.08,
        hp100: 189000.0,
        armor100: 1010.73,
        hp150: 378000.0,
        armor150: 1010.73,
    },
    BlockTier {
        rarity: Rarity::Divine,
        tier: 1,
        unlock_wave: 50,
        hp: 25000.0,
        armor: 300.0,
        xp: 20.0,
        fragments: 0.01,
        hp100: 50000.0,
        armor100: 450.0,
        hp150: 100000.0,
        armor150: 450.0,
    },
    BlockTier {
        rarity: Rarity::Divine,
        tier: 2,
        unlock_wave: 75,
        hp: 75000.0,
        armor: 495.0,
        xp: 60.0,
        fragments: 0.02,
        hp100: 150000.0,
        armor100: 742.5,
        hp150: 300000.0,
        armor150: 742.5,
    },
    BlockTier {
        rarity: Rarity::Divine,
        tier: 3,
        unlock_wave: 100,
        hp: 225000.0,
        armor: 816.75,
        xp: 180.0,
        fragments: 0.04,
        hp100: 450000.0,
        armor100: 1225.13,
        hp150: 900000.0,
        armor150: 1225.13,
    },
    BlockTier {
        rarity: Rarity::Divine,
        tier: 4,
        unlock_wave: 150,
        hp: 675000.0,
        armor: 1347.64,
        xp: 540.0,
        fragments: 0.08,
        hp100: 1350000.0,
        armor100: 2021.46,
        hp150: 2700000.0,
        armor150: 2021.46,
    },
];

pub(crate) const SPAWN_TABLE: &[(u32, u32, &[(Rarity, f64)])] = &[
    (1, 2, &[(Rarity::Dirt, 28.57), (Rarity::Common, 14.29)]),
    (
        3,
        4,
        &[
            (Rarity::Dirt, 25.40),
            (Rarity::Common, 12.70),
            (Rarity::Rare, 11.11),
        ],
    ),
    (
        5,
        5,
        &[
            (Rarity::Dirt, 25.52),
            (Rarity::Common, 10.94),
            (Rarity::Rare, 12.50),
        ],
    ),
    (
        6,
        9,
        &[
            (Rarity::Dirt, 22.97),
            (Rarity::Common, 9.84),
            (Rarity::Rare, 11.25),
            (Rarity::Epic, 10.00),
        ],
    ),
    (
        10,
        11,
        &[
            (Rarity::Dirt, 23.41),
            (Rarity::Common, 8.78),
            (Rarity::Rare, 9.88),
            (Rarity::Epic, 11.11),
        ],
    ),
    (
        12,
        14,
        &[
            (Rarity::Dirt, 21.74),
            (Rarity::Common, 8.15),
            (Rarity::Rare, 9.17),
            (Rarity::Epic, 10.32),
            (Rarity::Legendary, 7.14),
        ],
    ),
    (
        15,
        19,
        &[
            (Rarity::Dirt, 21.27),
            (Rarity::Common, 7.98),
            (Rarity::Rare, 8.97),
            (Rarity::Epic, 11.54),
            (Rarity::Legendary, 7.69),
        ],
    ),
    (
        20,
        24,
        &[
            (Rarity::Dirt, 19.50),
            (Rarity::Common, 7.31),
            (Rarity::Rare, 8.23),
            (Rarity::Epic, 12.34),
            (Rarity::Legendary, 8.64),
            (Rarity::Mythic, 5.00),
        ],
    ),
    (
        25,
        29,
        &[
            (Rarity::Dirt, 18.47),
            (Rarity::Common, 7.92),
            (Rarity::Rare, 9.05),
            (Rarity::Epic, 12.06),
            (Rarity::Legendary, 10.56),
            (Rarity::Mythic, 5.00),
        ],
    ),
    (
        30,
        49,
        &[
            (Rarity::Dirt, 18.10),
            (Rarity::Common, 9.05),
            (Rarity::Rare, 7.92),
            (Rarity::Epic, 11.88),
            (Rarity::Legendary, 11.88),
            (Rarity::Mythic, 5.00),
        ],
    ),
    (
        50,
        75,
        &[
            (Rarity::Dirt, 16.87),
            (Rarity::Common, 8.43),
            (Rarity::Rare, 9.84),
            (Rarity::Epic, 13.77),
            (Rarity::Legendary, 11.81),
            (Rarity::Mythic, 5.56),
            (Rarity::Divine, 2.78),
        ],
    ),
    (
        76,
        149,
        &[
            (Rarity::Dirt, 16.81),
            (Rarity::Common, 10.08),
            (Rarity::Rare, 10.08),
            (Rarity::Epic, 11.76),
            (Rarity::Legendary, 11.76),
            (Rarity::Mythic, 5.88),
            (Rarity::Divine, 2.94),
        ],
    ),
    (
        150,
        u32::MAX,
        &[
            (Rarity::Dirt, 16.81),
            (Rarity::Common, 10.08),
            (Rarity::Rare, 10.08),
            (Rarity::Epic, 11.76),
            (Rarity::Legendary, 11.76),
            (Rarity::Mythic, 5.88),
            (Rarity::Divine, 3.13),
        ],
    ),
];

pub(crate) const BOSS_FLOORS: &[(u32, &[(Rarity, u32)])] = &[
    (11, &[(Rarity::Dirt, 24)]),
    (17, &[(Rarity::Common, 24)]),
    (23, &[(Rarity::Dirt, 24)]),
    (25, &[(Rarity::Rare, 24)]),
    (29, &[(Rarity::Epic, 24)]),
    (31, &[(Rarity::Legendary, 24)]),
    (34, &[(Rarity::Common, 20), (Rarity::Legendary, 4)]),
    (35, &[(Rarity::Rare, 24)]),
    (41, &[(Rarity::Epic, 24)]),
    (44, &[(Rarity::Legendary, 24)]),
    (
        49,
        &[
            (Rarity::Dirt, 6),
            (Rarity::Common, 6),
            (Rarity::Rare, 6),
            (Rarity::Mythic, 6),
        ],
    ),
    (98, &[(Rarity::Mythic, 24)]),
    (149, &[(Rarity::Divine, 24)]),
];

#[derive(Clone, Copy)]
pub(crate) struct UpgradeSpec {
    pub(crate) id: &'static str,
    pub(crate) caption: &'static str,
    pub(crate) supported: bool,
}

pub(crate) const UPGRADE_SPECS: &[UpgradeSpec] = &[
    UpgradeSpec {
        id: "max_stamina_gems",
        caption: "Max Stamina +2, Stamina Mod Chance +0.05%",
        supported: true,
    },
    UpgradeSpec {
        id: "arch_exp_gems",
        caption: "Arch Exp gain +5% Exp Mod Chance +0.05%",
        supported: true,
    },
    UpgradeSpec {
        id: "fragment_gain_gems",
        caption: "Fragment Gain +2% Loot Mod Chance +0.05%",
        supported: true,
    },
    UpgradeSpec {
        id: "unlock_ability",
        caption: "Unlock New Ability",
        supported: true,
    },
    UpgradeSpec {
        id: "flat_damage_common",
        caption: "Flat Damage +1",
        supported: true,
    },
    UpgradeSpec {
        id: "armor_pen_common",
        caption: "Armor Penetration +1",
        supported: true,
    },
    UpgradeSpec {
        id: "arch_exp_common",
        caption: "Archaeology Exp Gain +2%",
        supported: true,
    },
    UpgradeSpec {
        id: "crit_upgrade",
        caption: "Crit Chance +0.25% / Crit Damage +1%",
        supported: true,
    },
    UpgradeSpec {
        id: "max_stamina_rare",
        caption: "Max Stamina +2 / Stamina Mod Chance +0.05%",
        supported: true,
    },
    UpgradeSpec {
        id: "flat_damage_rare",
        caption: "Flat Damage +2",
        supported: true,
    },
    UpgradeSpec {
        id: "loot_mod_gain",
        caption: "Loot Mod Gain +0.30x",
        supported: true,
    },
    UpgradeSpec {
        id: "enrage_upgrade",
        caption: "Enrage Damage / Crit Damage +2% / Enrage Cooldown -1s",
        supported: true,
    },
    UpgradeSpec {
        id: "flat_damage_super_crit",
        caption: "Flat Damage +2 / Super Crit Chance +0.35%",
        supported: true,
    },
    UpgradeSpec {
        id: "exp_frag_epic",
        caption: "Archaeology Exp Gain +3% / Fragment Gain +2%",
        supported: true,
    },
    UpgradeSpec {
        id: "flurry_upgrade",
        caption: "Flurry Stamina Gain +1 / Flurry Cooldown -1s",
        supported: true,
    },
    UpgradeSpec {
        id: "max_stamina_epic",
        caption: "Max Stamina +4 / Stamina Mod Gain +1",
        supported: true,
    },
    UpgradeSpec {
        id: "strength_skill_buff",
        caption: "Strength Skill Buff: Flat Damage +0.2, Damage +0.1%",
        supported: true,
    },
    UpgradeSpec {
        id: "agility_skill_buff",
        caption: "Agility Skill Buff: Max Stamina +1, Mod Ch. +0.02%",
        supported: true,
    },
    UpgradeSpec {
        id: "exp_stamina_legendary",
        caption: "Archaeology Exp Gain / +5% Maximum Stamina +1%",
        supported: true,
    },
    UpgradeSpec {
        id: "armor_pen_pct_cdr",
        caption: "Armor Penetration +2% / Ability Cooldowns -1s",
        supported: true,
    },
    UpgradeSpec {
        id: "crit_super_damage",
        caption: "Crit Damage +2% / Super Crit Damage +2%",
        supported: true,
    },
    UpgradeSpec {
        id: "quake_upgrade",
        caption: "Quake Attacks +1 / Cooldown -2s",
        supported: true,
    },
    UpgradeSpec {
        id: "perception_skill_buff",
        caption: "Perception Skill Buff: Mod Ch. +0.01%, Armor Pen +1",
        supported: true,
    },
    UpgradeSpec {
        id: "intellect_skill_buff",
        caption: "Intellect Skill Buff: Exp Gain +1%, Mod Ch. +0.01%",
        supported: true,
    },
    UpgradeSpec {
        id: "damage_armor_pen_mythic",
        caption: "Damage +2% / Armor Penetration +3",
        supported: true,
    },
    UpgradeSpec {
        id: "super_ultra_crit",
        caption: "Super Crit Chance +0.35% / Ultra Crit Chance +1%",
        supported: true,
    },
    UpgradeSpec {
        id: "exp_mod_gain",
        caption: "Exp Mod Gain +0.10x / Exp Mod Chance +0.10%",
        supported: true,
    },
    UpgradeSpec {
        id: "instacharge_stamina",
        caption: "Ability Instacharge Ch. +0.30% / Maximum Stamina +4",
        supported: true,
    },
    UpgradeSpec {
        id: "poly_archaeology_card_bonus",
        caption: "Polychrome Archaeology Card Bonus +15%",
        supported: true,
    },
    UpgradeSpec {
        id: "fragment_gain_multiplier",
        caption: "Fragment Gain 1.25x",
        supported: true,
    },
    UpgradeSpec {
        id: "stamina_mod_gain_once",
        caption: "Stamina Mod Gain +2",
        supported: true,
    },
    UpgradeSpec {
        id: "all_mod_chances",
        caption: "All Mod Chances +1.50%",
        supported: true,
    },
    UpgradeSpec {
        id: "exp_gain_double",
        caption: "Exp Gain 2.00x / All Stat Point Caps +5",
        supported: true,
    },
    UpgradeSpec {
        id: "asc1_stat_points",
        caption: "Stat Points +1 (Asc 1)",
        supported: false,
    },
    UpgradeSpec {
        id: "asc1_all_mod",
        caption: "All Mod Chances +0.02% (Asc 1)",
        supported: true,
    },
    UpgradeSpec {
        id: "asc1_crosshair_armor_pen",
        caption: "[Unlock Crosshair Fairies] / Armor Penetration +3 (Asc 1)",
        supported: true,
    },
    UpgradeSpec {
        id: "asc1_flat_damage_enrage",
        caption: "Flat Damage +3 / Enrage Cooldown -1s (Asc 1)",
        supported: true,
    },
    UpgradeSpec {
        id: "asc1_ultra_damage",
        caption: "Ultra Crit Damage +2% / Stamina Mod Chance +0.03% (Asc 1)",
        supported: true,
    },
    UpgradeSpec {
        id: "asc1_strength_skill_buff",
        caption: "Strength Skill Buff: / Damage +1%, Crit Damage +1% (Asc 1)",
        supported: true,
    },
    UpgradeSpec {
        id: "asc1_gold_crosshair",
        caption: "Gold Crosshair Chance +1% / Crosshair Auto-Tap Chance +1% (Asc 1)",
        supported: true,
    },
    UpgradeSpec {
        id: "asc1_flat_damage_ultra",
        caption: "Flat Damage +3 / Ultra Crit Chance +0.50% (Asc 1)",
        supported: true,
    },
    UpgradeSpec {
        id: "asc1_instacharge_stamina",
        caption: "Ability Instacharge +0.10% / Stamina Mod Chance + 0.10% (Asc 1)",
        supported: true,
    },
    UpgradeSpec {
        id: "asc1_damage_exp",
        caption: "Damage +10% / Archaeology Exp Gain +10% (Asc 1)",
        supported: true,
    },
    UpgradeSpec {
        id: "asc1_supercrit_expmod",
        caption: "Super Crit Damage +0.50% / Exp Mod Gain +2% (Asc 1)",
        supported: true,
    },
    UpgradeSpec {
        id: "asc1_max_stam_autotap",
        caption: "Maximum Stamina +0.50% / Crosshair Auto-Tap Ch. +0.20% (Asc 1)",
        supported: true,
    },
];

pub(crate) const ALL_CURRENCIES: [Currency; 7] = [
    Currency::Gems,
    Currency::Common,
    Currency::Rare,
    Currency::Epic,
    Currency::Legendary,
    Currency::Mythic,
    Currency::Divine,
];

pub(crate) const CARD_RARITIES: [Rarity; 7] = [
    Rarity::Dirt,
    Rarity::Common,
    Rarity::Rare,
    Rarity::Epic,
    Rarity::Legendary,
    Rarity::Mythic,
    Rarity::Divine,
];

pub(crate) const CARD_QUALITIES: [CardQuality; 5] = [
    CardQuality::None,
    CardQuality::Standard,
    CardQuality::Gilded,
    CardQuality::Polychrome,
    CardQuality::Infernal,
];

pub(crate) fn base_stat_cap(skill: &str) -> u32 {
    match skill {
        "strength" | "agility" => 50,
        "perception" | "intellect" | "luck" => 25,
        "divinity" => 10,
        "corruption" => 15,
        _ => 0,
    }
}
