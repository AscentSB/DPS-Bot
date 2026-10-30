//! The planner context (`makeContext` in src/calc/upgrades.js) and everything derived from it once per run: profile
//! stat totals, the part of every stat bucket that does not depend on gear, class multipliers, the tuning split.

use crate::loose::{Bucket, Loose};
use dps_core::config::accessories::{
    default_power, default_tuning_split, tuning_per_point, tuning_stats, POWERS,
};
use dps_core::config::constants::*;
use dps_core::config::item_stats::misc_buffs;
use dps_core::config::profile::{harp_song, BASE_CRIT_DAMAGE, SB_STRENGTH_EVERY, SKILL_BONUSES, SLAYER_BONUSES};
use dps_core::jsmath;
use dps_core::kinds::{Class, FloorMode, Mode, Target};
use dps_core::model::{
    Blessings, Collections, DpsJob, Map, PetProfile, ProfileStats, Resource, Tuning, Weights,
};
use dps_core::stat::Stat;
use std::sync::Arc;

/// JS `upgradeMode`: `all` lists every enchant level, `max` jumps straight to the max level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UpgradeMode {
    All,
    Max,
}

impl UpgradeMode {
    pub fn from_js(s: &str) -> UpgradeMode {
        if s == "max" {
            UpgradeMode::Max
        } else {
            UpgradeMode::All
        }
    }
    pub fn js_name(self) -> &'static str {
        match self {
            UpgradeMode::All => "all",
            UpgradeMode::Max => "max",
        }
    }
}

/// The arguments of JS `makeContext`.
#[derive(Clone, Debug)]
pub struct CtxInit {
    pub cls: Class,
    pub cata_level: f64,
    pub secrets: f64,
    pub power: Option<String>,
    pub tuning: Option<Tuning>,
    pub profile_speed: f64,
    pub pet_profile: PetProfile,
    pub archer_level: f64,
    pub berserk_level: f64,
    pub mage_level: f64,
    pub combat_level: f64,
    /// Skill levels (`None` = API off, every skill counts as maxed).
    pub skills: Option<Map<i32>>,
    pub slayers: Option<Map<i32>>,
    pub profile_stats: Option<ProfileStats>,
    pub upgrade_mode: UpgradeMode,
    pub mode: Mode,
    pub floor_mode: FloorMode,
    pub blessings: Option<Blessings>,
    pub collections: Option<Collections>,
    pub any_chest: bool,
    pub no_clear_boots: bool,
    pub resource: Arc<Resource>,
}

impl CtxInit {
    /// Defaults of the JS parameters for a class and Catacombs level.
    pub fn new(cls: Class, cata_level: f64, resource: Arc<Resource>) -> CtxInit {
        CtxInit {
            cls,
            cata_level,
            secrets: 0.0,
            power: None,
            tuning: None,
            profile_speed: 0.0,
            pet_profile: PetProfile::default(),
            archer_level: 0.0,
            berserk_level: 0.0,
            mage_level: 0.0,
            combat_level: 0.0,
            skills: None,
            slayers: None,
            profile_stats: None,
            upgrade_mode: UpgradeMode::All,
            mode: Mode::Lcm,
            floor_mode: FloorMode::Master,
            blessings: None,
            collections: None,
            any_chest: false,
            no_clear_boots: false,
            resource,
        }
    }

    /// `makeContext` arguments the way `runDps` passes them from a read member and a job.
    pub fn from_member(member: &dps_core::model::Member, job: &DpsJob, resource: Arc<Resource>) -> Option<CtxInit> {
        let cls = Class::from_js(&job.cls)?;
        let mut init = CtxInit::new(cls, member.cata_level as f64, resource);
        init.secrets = member.secrets;
        init.power = member.power.clone();
        init.tuning = member.tuning.clone();
        init.profile_speed = member.profile_speed as f64;
        init.pet_profile = member.pet_profile.clone();
        init.archer_level = member.archer_level as f64;
        init.berserk_level = member.berserk_level as f64;
        init.mage_level = member.mage_level as f64;
        init.combat_level = member.combat_level as f64;
        init.skills = member.skills.clone();
        init.slayers = member.slayers.clone();
        init.collections = member.collections.clone();
        init.profile_stats = Some(member.profile_stats.clone());
        init.upgrade_mode = UpgradeMode::from_js(&job.upgrade_mode);
        init.mode = Mode::from_js(&job.mode)?;
        init.floor_mode = FloorMode::from_js(&job.floor_mode)?;
        init.blessings = Some(job.blessings);
        init.any_chest = job.any_chest;
        init.no_clear_boots = job.no_clear_boots;
        Some(init)
    }
}

/// What item stats (and the model's caches) depend on. A [`crate::Model`] only works with contexts of one key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemCtxKey {
    pub cls: Class,
    pub cata_bits: u64,
    pub floor_mode: FloorMode,
    pub mode: Mode,
    pub secrets_bits: u64,
    pub maxed_bits: u64,
    pub resource_ptr: usize,
    pub pet_gold_bits: u64,
}

/// Everything computed once per context.
#[derive(Debug)]
pub struct CtxDerived {
    pub rcm: bool,
    pub maxed_collections: f64,
    /// JS `classAdditiveDamage(ctx)`.
    pub class_additive_damage: f64,
    /// JS `archerMultiplier(ctx)`.
    pub archer_multiplier: f64,
    /// JS `berserkMeleeMultiplier(ctx)`.
    pub berserk_melee_multiplier: f64,
    /// JS `profileStatTotals(ctx)`.
    pub profile_totals: Loose,
    /// The bucket before any gear, per `[target][weapon is a bow]` (misc buffs, class stats, profile stats, arrow
    /// damage, combat crit chance, mage ability damage).
    pub prefix: [[Bucket; 2]; 2],
    /// `secretDigits(ctx.secrets)`: the digit count used when a General's Medallion is held.
    pub secret_digit_medallion: f64,
    pub default_power: usize,
    pub ctx_power: Option<usize>,
    pub item_key: ItemCtxKey,
}

/// One tuning point sink: `points * share * perPoint` goes to `stat`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TuneSink {
    pub stat: Stat,
    pub share: f64,
    pub per_point: f64,
}

/// The leftover-tuning distribution (JS `distribute`): which DPS stats get the points, resolved once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TuningPlan {
    pub len: u8,
    pub sinks: [TuneSink; 6],
}

/// The planner context. Cheap to clone (heavy parts are shared).
#[derive(Clone, Debug)]
pub struct Ctx {
    pub cls: Class,
    pub cata_level: f64,
    pub secrets: f64,
    pub power: Option<String>,
    pub tuning: Tuning,
    pub profile_speed: f64,
    pub pet_profile: PetProfile,
    pub archer_level: f64,
    pub berserk_level: f64,
    pub mage_level: f64,
    pub combat_level: f64,
    pub skills: Option<Map<i32>>,
    pub slayers: Option<Map<i32>>,
    pub profile_stats: Option<ProfileStats>,
    pub upgrade_mode: UpgradeMode,
    pub mode: Mode,
    /// Already forced to `Master` for Archer and Berserker.
    pub floor_mode: FloorMode,
    pub blessings: Blessings,
    pub collections: Option<Collections>,
    pub any_chest: bool,
    pub no_clear_boots: bool,
    pub resource: Arc<Resource>,
    pub d: Arc<CtxDerived>,
    pub tuning_plan: TuningPlan,
}

/// JS `weightsFor(cls, target, mode, floorMode)`.
pub fn weights_for(cls: Class, target: Option<Target>, mode: Mode, floor_mode: FloorMode) -> Weights {
    if cls == Class::Mage && mode == Mode::Rcm {
        return Weights { wither: 100.0, dragon: 0.0 };
    }
    if cls == Class::Mage && floor_mode == FloorMode::Normal {
        return Weights { wither: 100.0, dragon: 0.0 };
    }
    match target {
        Some(Target::Wither) => Weights { wither: 100.0, dragon: 0.0 },
        Some(Target::Dragon) => Weights { wither: 0.0, dragon: 100.0 },
        None => class_info(cls).weights,
    }
}

/// JS `skeletonMasterChestRule(cls, cataLevel)`.
pub fn skeleton_master_chest_rule(cls: Class, cata_level: f64) -> bool {
    SKELETON_MASTER_CHEST_CLASSES.contains(&cls) && cata_level >= SKELETON_MASTER_CHEST_CATA
}

/// The index of a power key in `POWERS` (`POWERS[key]` truthiness).
pub fn power_index(key: &str) -> Option<usize> {
    POWERS.iter().position(|p| p.key == key)
}

impl Ctx {
    /// JS `makeContext(...)`.
    pub fn new(init: CtxInit) -> Ctx {
        let floor_mode = if init.cls == Class::Mage { init.floor_mode } else { FloorMode::Master };
        let blessings = match init.blessings {
            Some(b) => blessings_validated(b),
            None => Blessings::default(),
        };
        let tuning = init.tuning.unwrap_or_default();
        let mut ctx = Ctx {
            cls: init.cls,
            cata_level: init.cata_level,
            secrets: init.secrets,
            power: init.power,
            tuning,
            profile_speed: init.profile_speed,
            pet_profile: init.pet_profile,
            archer_level: init.archer_level,
            berserk_level: init.berserk_level,
            mage_level: init.mage_level,
            combat_level: init.combat_level,
            skills: init.skills,
            slayers: init.slayers,
            profile_stats: init.profile_stats,
            upgrade_mode: init.upgrade_mode,
            mode: init.mode,
            floor_mode,
            blessings,
            collections: init.collections,
            any_chest: init.any_chest,
            no_clear_boots: init.no_clear_boots,
            resource: init.resource,
            d: Arc::new(placeholder_derived()),
            tuning_plan: TuningPlan { len: 0, sinks: [TuneSink { stat: Stat::Strength, share: 0.0, per_point: 0.0 }; 6] },
        };
        ctx.d = Arc::new(derive(&ctx));
        ctx.tuning_plan = tuning_plan(&ctx);
        ctx
    }

    /// Recompute everything derived from the fields after changing some of them (the JS `{ ...ctx, ...changes }`
    /// followed by a fresh set of caches).
    pub fn rederive(&mut self) {
        self.d = Arc::new(derive(self));
        self.tuning_plan = tuning_plan(self);
    }

    /// `ctx.tuning = { ...ctx.tuning, split }` (JS keys are the stat names).
    pub fn set_tuning_split(&mut self, split: Option<Vec<(Stat, f64)>>) {
        self.tuning.split = split.map(|s| s.into_iter().map(|(k, v)| (k.js_name().to_string(), v)).collect());
        self.tuning_plan = tuning_plan(self);
    }

    /// A copy with another tuning split (cheap).
    pub fn with_tuning_split(&self, split: Option<Vec<(Stat, f64)>>) -> Ctx {
        let mut c = self.clone();
        c.set_tuning_split(split);
        c
    }

    /// The tuning split as stats, in key order (`ctx.tuning.split`).
    pub fn tuning_split(&self) -> Option<Vec<(Stat, f64)>> {
        self.tuning
            .split
            .as_ref()
            .map(|m| m.iter().filter_map(|(k, v)| Stat::from_js(k).map(|s| (s, *v))).collect())
    }

    #[inline]
    pub fn is_rcm(&self) -> bool {
        self.d.rcm
    }

    /// JS `hasDragonSet(ctx)`: Mage LCM in Master Mode can swap gear for the dragons.
    #[inline]
    pub fn has_dragon_set(&self) -> bool {
        self.cls == Class::Mage && self.mode != Mode::Rcm && self.floor_mode != FloorMode::Normal
    }

    /// JS `maxedCollections(ctx)`.
    #[inline]
    pub fn maxed_collections(&self) -> f64 {
        self.d.maxed_collections
    }

    /// JS `skeletonMasterChestForced(ctx)`.
    #[inline]
    pub fn skeleton_master_chest_forced(&self) -> bool {
        !self.any_chest && skeleton_master_chest_rule(self.cls, self.cata_level)
    }

    /// JS `clearBootsUsed(ctx)`: Archer / Berserker swap to separate clear boots unless the button turns them off.
    #[inline]
    pub fn clear_boots_used(&self) -> bool {
        CLEAR_BOOTS_CLASSES.contains(&self.cls) && self.mode != Mode::Rcm && !self.no_clear_boots
    }

    /// JS `excludedItem(id, ctx)`: items a class never uses.
    pub fn excluded_item(&self, id: &str) -> bool {
        if class_excluded_item(self.cls, id) {
            return true;
        }
        if self.skeleton_master_chest_forced()
            && self.resource.get(id).and_then(|i| i.category.as_deref()) == Some("CHESTPLATE")
            && !SKELETON_MASTER_CHESTS.contains(&id)
        {
            return true;
        }
        // Balloon Snake's Jump Boost gets in the way in the clear.
        if self.mode == Mode::Rcm && id == "BALLOON_SNAKE" {
            return true;
        }
        self.cls == Class::Mage && self.mode != Mode::Rcm && (id == "SPEED_WITHER_BOOTS" || id == "STARRED_SPEED_WITHER_BOOTS")
    }

    /// JS `classAdditiveDamage(ctx)`.
    #[inline]
    pub fn class_additive_damage(&self) -> f64 {
        self.d.class_additive_damage
    }

    /// JS `saCloakMultiplier(hitsPerSecond, ctx)`.
    pub fn sa_cloak_multiplier(&self, hits_per_second: f64) -> f64 {
        sa_cloak_multiplier(hits_per_second, Some(self))
    }

    /// JS `speedCap(cls, mode)`.
    #[inline]
    pub fn speed_cap(&self) -> f64 {
        speed_cap(self.cls, self.mode)
    }
}

/// JS `saCloakMultiplier(hitsPerSecond, ctx)`: Shadow Assassin Cloak averaged over every hit.
pub fn sa_cloak_multiplier(hits_per_second: f64, ctx: Option<&Ctx>) -> f64 {
    let reduction = match ctx {
        Some(c) if c.cls == Class::Mage => {
            SOLO_CLASS_MULTIPLIER * MAGE_COOLDOWN_REDUCTION_BASE
                + jsmath::min(MAGE_COOLDOWN_REDUCTION_MAX, MAGE_COOLDOWN_REDUCTION_PER_LEVEL * c.mage_level)
        }
        _ => 0.0,
    };
    let hits_per_cooldown = jsmath::max(1.0, SA_CLOAK_COOLDOWN_S * (1.0 - reduction / 100.0) * hits_per_second);
    1.0 + (SA_CLOAK_MULTIPLIER - 1.0) / hits_per_cooldown
}

fn placeholder_derived() -> CtxDerived {
    CtxDerived {
        rcm: false,
        maxed_collections: 0.0,
        class_additive_damage: 0.0,
        archer_multiplier: 1.0,
        berserk_melee_multiplier: 1.0,
        profile_totals: Loose::new(),
        prefix: [[Bucket::new(); 2]; 2],
        secret_digit_medallion: 0.0,
        default_power: 0,
        ctx_power: None,
        item_key: ItemCtxKey {
            cls: Class::Arch,
            cata_bits: 0,
            floor_mode: FloorMode::Master,
            mode: Mode::Lcm,
            secrets_bits: 0,
            maxed_bits: 0,
            resource_ptr: 0,
            pet_gold_bits: 0,
        },
    }
}

/// JS `computeProfileStats`: base crit damage, skills, slayers, SkyBlock level, Harp songs, cake buffs. Empty when
/// the profile was not read.
fn profile_totals(ctx: &Ctx) -> Loose {
    let mut out = Loose::new();
    let Some(p) = &ctx.profile_stats else {
        return out;
    };
    out.add_one(Stat::CritDamage, BASE_CRIT_DAMAGE, 1.0);
    for sb in SKILL_BONUSES {
        let have = ctx.skills.as_ref().and_then(|s| s.get(sb.skill)).map(|&l| l as f64);
        let level = jsmath::min(sb.max, have.unwrap_or(sb.max));
        let mut l = 1.0;
        while l <= level {
            if let Some(step) = sb.step_for(l) {
                out.add_loose(step, 1.0);
            }
            l += 1.0;
        }
    }
    for sl in SLAYER_BONUSES {
        let level = ctx.slayers.as_ref().and_then(|s| s.get(sl.boss)).map(|&l| l as f64).unwrap_or(0.0);
        let mut l = 1.0;
        while l <= level {
            if let Some(r) = sl.reward(l) {
                out.add_loose(r, 1.0);
            }
            l += 1.0;
        }
    }
    out.add_one(Stat::Strength, (p.sb_level as f64 / SB_STRENGTH_EVERY).floor(), 1.0);
    for song in &p.harp_songs {
        out.add_one(Stat::Intelligence, harp_song(song).map(|s| s.intelligence).unwrap_or(0.0), 1.0);
    }
    for cake in &p.cakes {
        if let Some(stat) = Stat::from_js(&cake.stat) {
            out.add_one(stat, cake.amount, 1.0);
        }
    }
    out
}

fn derive(ctx: &Ctx) -> CtxDerived {
    let rcm = ctx.cls == Class::Mage && ctx.mode == Mode::Rcm;
    let maxed = ctx.collections.as_ref().map(|c| c.maxed as f64).unwrap_or(DEFAULT_MAXED_COLLECTIONS);
    let combat = ctx.combat_level;
    let warrior = WARRIOR_PER_LEVEL * jsmath::min(combat, 50.0) + WARRIOR_PER_LEVEL_AFTER_50 * jsmath::max(0.0, combat - 50.0);
    let berserk = if ctx.cls == Class::Bers {
        SOLO_CLASS_MULTIPLIER * BERSERK_DAMAGE_BASE + (BERSERK_DAMAGE_PER_LEVEL + LUST_FOR_BLOOD_PER_LEVEL) * ctx.berserk_level
    } else {
        0.0
    };
    let totals = profile_totals(ctx);
    let misc = misc_buffs(ctx.cls);
    let mut prefix = [[Bucket::new(); 2]; 2];
    for (t, target) in Target::ALL.iter().enumerate() {
        for (bow, b) in prefix[t].iter_mut().enumerate() {
            b.add(misc.stats, 1.0);
            if ctx.cls == Class::Mage {
                b.add_one(Stat::Intelligence, SOLO_CLASS_MULTIPLIER * MAGE_INT_BASE + MAGE_INT_PER_LEVEL * ctx.mage_level, 1.0);
            }
            b.add_loose(&totals, 1.0);
            if bow == 1 {
                b.add_one(Stat::Damage, arrow_damage(ctx.cls), 1.0);
            }
            b.add_one(Stat::CritChance, combat * COMBAT_CRIT_CHANCE_PER_LEVEL, 1.0);
            if ctx.cls == Class::Mage {
                b.add_one(
                    Stat::AbilityDamage,
                    SOLO_CLASS_MULTIPLIER * MAGE_ABILITY_DAMAGE_BASE
                        + MAGE_ABILITY_DAMAGE_PER_5_LEVELS * (ctx.mage_level / 5.0).floor(),
                    1.0,
                );
                let ench = ctx.skills.as_ref().and_then(|s| s.get("ENCHANTING")).map(|&l| l as f64).unwrap_or(ENCHANTING_MAX_LEVEL as f64);
                b.add_one(Stat::AbilityDamage, ENCHANTING_ABILITY_DAMAGE_PER_LEVEL * ench, 1.0);
            }
            if !rcm {
                match target {
                    Target::Wither => b.add(misc.wither, 1.0),
                    Target::Dragon => b.add(misc.dragon, 1.0),
                }
            }
        }
    }
    let pet_gold = ctx.pet_profile.gold_collection.unwrap_or(f64::NAN);
    CtxDerived {
        rcm,
        maxed_collections: maxed,
        class_additive_damage: warrior + berserk,
        archer_multiplier: if ctx.cls == Class::Arch {
            1.0 + (SOLO_CLASS_MULTIPLIER * ARCHER_ARROW_DAMAGE_PER_LEVEL * ctx.archer_level) / 100.0
        } else {
            1.0
        },
        berserk_melee_multiplier: 1.0
            + (SOLO_CLASS_MULTIPLIER * BERSERK_DAMAGE_BASE + BERSERK_DAMAGE_PER_LEVEL * ctx.berserk_level) / 100.0,
        profile_totals: totals,
        prefix,
        secret_digit_medallion: dps_core::formulas::secret_digits(ctx.secrets),
        default_power: power_index(default_power(ctx.cls)).expect("default power exists"),
        ctx_power: ctx.power.as_deref().and_then(power_index),
        item_key: ItemCtxKey {
            cls: ctx.cls,
            cata_bits: ctx.cata_level.to_bits(),
            floor_mode: ctx.floor_mode,
            mode: ctx.mode,
            secrets_bits: ctx.secrets.to_bits(),
            maxed_bits: maxed.to_bits(),
            resource_ptr: Arc::as_ptr(&ctx.resource) as usize,
            pet_gold_bits: pet_gold.to_bits(),
        },
    }
}

/// JS `distribute` setup: which stats get leftover points and in what shares.
fn tuning_plan(ctx: &Ctx) -> TuningPlan {
    let useful = tuning_stats(ctx.cls, ctx.mode);
    let mut entries: Vec<(Stat, f64)> = Vec::new();
    if let Some(split) = &ctx.tuning.split {
        for (k, share) in split {
            if let Some(stat) = Stat::from_js(k) {
                if useful.contains(&stat) && tuning_per_point(stat).is_some_and(|p| p != 0.0 && !p.is_nan()) {
                    entries.push((stat, *share));
                }
            }
        }
    }
    let mut total = 0.0;
    for &(_, share) in &entries {
        total += share;
    }
    let mut plan = TuningPlan { len: 0, sinks: [TuneSink { stat: Stat::Strength, share: 0.0, per_point: 0.0 }; 6] };
    let mut push = |stat: Stat, share: f64| {
        plan.sinks[plan.len as usize] = TuneSink { stat, share, per_point: tuning_per_point(stat).unwrap_or(0.0) };
        plan.len += 1;
    };
    if total != 0.0 && !total.is_nan() {
        for &(stat, share) in &entries {
            push(stat, share / total);
        }
    } else {
        let fallback: Vec<(Stat, f64)> =
            if ctx.is_rcm_cfg() { vec![(Stat::Intelligence, 1.0)] } else { default_tuning_split(ctx.cls).to_vec() };
        for (stat, share) in fallback {
            push(stat, share);
        }
    }
    plan
}

impl Ctx {
    fn is_rcm_cfg(&self) -> bool {
        self.cls == Class::Mage && self.mode == Mode::Rcm
    }
}
