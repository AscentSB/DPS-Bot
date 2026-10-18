//! Small enums for the string values the JS code compares all the time (`ctx.cls === 'mage'`, `target === 'wither'`,
//! rarities). Each has `from_js` / `js_name` to convert at the edges (job JSON, item JSON, labels).

/// Dungeon class: JS `'arch' | 'bers' | 'mage'`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Class {
    Arch,
    Bers,
    Mage,
}

impl Class {
    pub const ALL: [Class; 3] = [Class::Arch, Class::Bers, Class::Mage];
    pub fn from_js(s: &str) -> Option<Class> {
        match s {
            "arch" => Some(Class::Arch),
            "bers" => Some(Class::Bers),
            "mage" => Some(Class::Mage),
            _ => None,
        }
    }
    pub const fn js_name(self) -> &'static str {
        match self {
            Class::Arch => "arch",
            Class::Bers => "bers",
            Class::Mage => "mage",
        }
    }
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// A boss phase the damage is scored against (JS `TARGETS = ['wither', 'dragon']`, in that order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Target {
    Wither,
    Dragon,
}

impl Target {
    /// JS `TARGETS`, in iteration order.
    pub const ALL: [Target; 2] = [Target::Wither, Target::Dragon];
    pub fn from_js(s: &str) -> Option<Target> {
        match s {
            "wither" => Some(Target::Wither),
            "dragon" => Some(Target::Dragon),
            _ => None,
        }
    }
    pub const fn js_name(self) -> &'static str {
        match self {
            Target::Wither => "wither",
            Target::Dragon => "dragon",
        }
    }
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Which bucket a config stat block goes to: JS `target: 'global' | 'wither' | 'dragon'` (`global` = both).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StatTarget {
    Global,
    Wither,
    Dragon,
}

impl StatTarget {
    pub fn from_js(s: &str) -> Option<StatTarget> {
        match s {
            "global" => Some(StatTarget::Global),
            "wither" => Some(StatTarget::Wither),
            "dragon" => Some(StatTarget::Dragon),
            _ => None,
        }
    }
    pub const fn js_name(self) -> &'static str {
        match self {
            StatTarget::Global => "global",
            StatTarget::Wither => "wither",
            StatTarget::Dragon => "dragon",
        }
    }
    /// The single target, or `None` for `Global`.
    pub const fn target(self) -> Option<Target> {
        match self {
            StatTarget::Global => None,
            StatTarget::Wither => Some(Target::Wither),
            StatTarget::Dragon => Some(Target::Dragon),
        }
    }
}

/// JS `floorMode`: `'master'` (Master Mode, the default) or `'normal'` (F7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FloorMode {
    Master,
    Normal,
}

impl FloorMode {
    pub fn from_js(s: &str) -> Option<FloorMode> {
        match s {
            "master" => Some(FloorMode::Master),
            "normal" => Some(FloorMode::Normal),
            _ => None,
        }
    }
    pub const fn js_name(self) -> &'static str {
        match self {
            FloorMode::Master => "master",
            FloorMode::Normal => "normal",
        }
    }
}

/// JS `mode`: `'lcm'` (boss set, the default) or `'rcm'` (Mage clear set).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Lcm,
    Rcm,
}

impl Mode {
    pub fn from_js(s: &str) -> Option<Mode> {
        match s {
            "lcm" => Some(Mode::Lcm),
            "rcm" => Some(Mode::Rcm),
            _ => None,
        }
    }
    pub const fn js_name(self) -> &'static str {
        match self {
            Mode::Lcm => "lcm",
            Mode::Rcm => "rcm",
        }
    }
}

/// The rarities the bot knows: `RARITIES` (COMMON..DIVINE, indices 0-6) then `SPECIAL_RARITIES` (7, 8). Item JSON
/// can hold other tiers (e.g. `UNOBTAINABLE`); keep those as strings and use `Rarity::from_js` returning `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Mythic,
    Divine,
    Special,
    VerySpecial,
}

impl Rarity {
    pub const ALL: [Rarity; 9] = [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::Epic,
        Rarity::Legendary,
        Rarity::Mythic,
        Rarity::Divine,
        Rarity::Special,
        Rarity::VerySpecial,
    ];
    pub fn from_js(s: &str) -> Option<Rarity> {
        Some(match s {
            "COMMON" => Rarity::Common,
            "UNCOMMON" => Rarity::Uncommon,
            "RARE" => Rarity::Rare,
            "EPIC" => Rarity::Epic,
            "LEGENDARY" => Rarity::Legendary,
            "MYTHIC" => Rarity::Mythic,
            "DIVINE" => Rarity::Divine,
            "SPECIAL" => Rarity::Special,
            "VERY_SPECIAL" => Rarity::VerySpecial,
            _ => return None,
        })
    }
    pub const fn js_name(self) -> &'static str {
        match self {
            Rarity::Common => "COMMON",
            Rarity::Uncommon => "UNCOMMON",
            Rarity::Rare => "RARE",
            Rarity::Epic => "EPIC",
            Rarity::Legendary => "LEGENDARY",
            Rarity::Mythic => "MYTHIC",
            Rarity::Divine => "DIVINE",
            Rarity::Special => "SPECIAL",
            Rarity::VerySpecial => "VERY_SPECIAL",
        }
    }
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}
