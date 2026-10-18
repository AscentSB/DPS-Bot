//! Session data and fixed texts of the /dps command (port of the pure parts of src/commands/dps.js).

use dps_core::config::constants::{SKELETON_MASTER_CHEST_CATA, SKELETON_MASTER_CHEST_CLASSES};
use dps_core::model::{Blessings, DpsResult, ProfileMenu};
use dps_core::{Class, FloorMode, Mode, Target};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A /dps message goes inactive after this long without a button press.
pub const INACTIVITY: Duration = Duration::from_secs(30);
/// Interaction tokens last 15 minutes; past this the fallback edit through the token is not tried.
pub const TOKEN_EDIT: Duration = Duration::from_secs(14 * 60);
/// Button sessions are kept this long after the last use.
pub const SESSION: Duration = Duration::from_secs(30 * 60);
/// Per-user /dps cooldown.
pub const COOLDOWN: Duration = Duration::from_secs(10);

/// Coin plans are computed this deep and shown PAGE_SIZE at a time.
pub const PLAN_STEPS: usize = 40;
pub const PAGE_SIZE: usize = 8;
/// Steps under this many coins are free (claimed items).
pub const FREE_STEP_COINS: f64 = 1000.0;

pub const INACTIVE_NOTE: &str = "This session is inactive. Run /dps again to start a new one.";
pub const EXPIRED_REPLY: &str = "This /dps session expired. Run /dps again to start a new one.";
pub const PRICES_LOADING: &str = "Prices are still loading after a restart. Try again in a moment.";
pub const MOJANG_DOWN: &str = "Could not reach Mojang to look up that username. Try again in a minute.";
pub const PLAN_TOO_LONG: &str = "This calculation took too long and was stopped. Try again in a minute.";
pub const NOT_OWNER: &str = "Only the person who ran this /dps can use these buttons.";
pub const INVALID_IGN: &str = "That is not a valid Minecraft username (2 to 16 letters, digits or underscores).";
pub const NO_ACCOUNT: &str = "No Minecraft account has that username.";
pub const GENERIC_FAILURE: &str = "Something went wrong while loading this profile. Try again in a minute.";
pub const HYPIXEL_KEY_BAD: &str = "The bot's Hypixel API key is missing or invalid. Tell the bot owner.";
pub const HYPIXEL_RATE_LIMIT: &str = "The Hypixel API rate limit was reached. Try again in a minute.";
pub const HYPIXEL_DOWN: &str = "Could not reach the Hypixel API. Try again in a minute.";

/// JS `hypixelError(err)`, from the error message.
pub fn hypixel_error(msg: &str) -> &'static str {
    if msg.contains("HYPIXEL_API_KEY") || msg.contains("HTTP 403") {
        HYPIXEL_KEY_BAD
    } else if msg.contains("HTTP 429") {
        HYPIXEL_RATE_LIMIT
    } else {
        HYPIXEL_DOWN
    }
}

/// `Please wait {n}s before using /dps again.`
pub fn cooldown_reply(wait_secs: u64) -> String {
    format!("Please wait {wait_secs}s before using /dps again.")
}

/// `**{name}** has no SkyBlock profiles.` (name escaped by the caller with `embed::escape_markdown`).
pub fn no_profiles_reply(escaped_name: &str) -> String {
    format!("**{escaped_name}** has no SkyBlock profiles.")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UpgradeMode {
    All,
    Max,
}

impl UpgradeMode {
    pub fn from_js(s: &str) -> Option<Self> {
        match s {
            "all" => Some(Self::All),
            "max" => Some(Self::Max),
            _ => None,
        }
    }
    pub const fn js_name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Max => "max",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum View {
    Coins,
    Profile,
}

impl View {
    pub fn from_js(s: &str) -> Option<Self> {
        match s {
            "coins" => Some(Self::Coins),
            "profile" => Some(Self::Profile),
            _ => None,
        }
    }
}

/// One /dps message's state (JS `sessions` value).
#[derive(Clone, Debug)]
pub struct Session {
    pub cls: Class,
    pub target: Option<Target>,
    pub mode: Mode,
    pub floor_mode: FloorMode,
    pub blessings: Option<Blessings>,
    pub any_chest: bool,
    pub upgrade_mode: UpgradeMode,
    /// The account name as Mojang spells it (unescaped).
    pub name: String,
    /// `member.cataLevel` (None when there is no member, like the JS `s.member?.cataLevel`).
    pub cata_level: Option<f64>,
    /// Coin plans by [`plan_key`].
    pub coins: HashMap<String, Arc<DpsResult>>,
    pub profile: Option<ProfileMenu>,
    pub view: View,
    pub page: usize,
    pub owner: u64,
    pub used_at: Instant,
    pub interaction_at: Instant,
    /// Message to edit when the session goes inactive.
    pub message_id: Option<u64>,
    pub channel_id: Option<u64>,
    /// Latest interaction token (for the fallback edit within [`TOKEN_EDIT`]).
    pub interaction_token: Option<String>,
}

impl Session {
    pub fn new(cls: Class, name: String, owner: u64) -> Self {
        let now = Instant::now();
        Session {
            cls,
            target: None,
            mode: Mode::Lcm,
            floor_mode: FloorMode::Master,
            blessings: Some(Blessings::default()),
            any_chest: false,
            upgrade_mode: UpgradeMode::All,
            name,
            cata_level: None,
            coins: HashMap::new(),
            profile: None,
            view: View::Coins,
            page: 0,
            owner,
            used_at: now,
            interaction_at: now,
            message_id: None,
            channel_id: None,
            interaction_token: None,
        }
    }

    /// The plan currently shown (`s.coins[planKey(s)]`).
    pub fn current(&self) -> Option<&Arc<DpsResult>> {
        self.coins.get(plan_key(self.upgrade_mode, self.any_chest))
    }
}

/// JS `planKey(s)`: `all`, `max`, `all:anyChest`, `max:anyChest`.
pub fn plan_key(upgrade_mode: UpgradeMode, any_chest: bool) -> &'static str {
    match (upgrade_mode, any_chest) {
        (UpgradeMode::All, false) => "all",
        (UpgradeMode::Max, false) => "max",
        (UpgradeMode::All, true) => "all:anyChest",
        (UpgradeMode::Max, true) => "max:anyChest",
    }
}

/// JS `skeletonMasterChestRule(cls, cataLevel)`.
pub fn skeleton_master_chest_rule(cls: Class, cata_level: Option<f64>) -> bool {
    SKELETON_MASTER_CHEST_CLASSES.contains(&cls) && cata_level.unwrap_or(0.0) >= SKELETON_MASTER_CHEST_CATA
}

/// JS `chestToggle(s)`: the Skeleton Master chest button shows only where the rule applies.
pub fn chest_toggle(s: &Session) -> bool {
    skeleton_master_chest_rule(s.cls, s.cata_level)
}

/// JS `pageCount(s)`.
pub fn page_count(s: &Session) -> usize {
    let rows = match s.view {
        View::Profile => s.profile.as_ref().map_or(0, |p| p.rows.len()),
        View::Coins => s.current().and_then(|r| r.plan.as_ref()).map_or(0, |p| p.steps.len()),
    };
    rows.div_ceil(PAGE_SIZE).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chest_rule() {
        assert!(skeleton_master_chest_rule(Class::Arch, Some(40.0)));
        assert!(skeleton_master_chest_rule(Class::Bers, Some(36.0)));
        assert!(!skeleton_master_chest_rule(Class::Mage, Some(40.0)));
        assert!(!skeleton_master_chest_rule(Class::Arch, Some(35.0)));
        assert!(!skeleton_master_chest_rule(Class::Arch, None));
    }

    #[test]
    fn errors() {
        assert_eq!(hypixel_error("HTTP 403 x"), HYPIXEL_KEY_BAD);
        assert_eq!(hypixel_error("HTTP 429"), HYPIXEL_RATE_LIMIT);
        assert_eq!(hypixel_error("boom"), HYPIXEL_DOWN);
    }
}
