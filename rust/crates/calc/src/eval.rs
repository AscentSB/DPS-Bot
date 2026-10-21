//! `evaluate` (src/calc/stats.js): the damage of a loadout against both bosses. No allocation inside; callable from
//! many threads at once on a shared `&Model`.

use crate::arena::{ItemFlags, Model};
use crate::ctx::Ctx;
use crate::ids::*;
use crate::loadout::{GearView, Loadout};
use crate::loose::{Bucket, Loose};
use dps_core::config::accessories::{TUNING_PER_POINT_ATTACK_SPEED, TUNING_PER_POINT_SPEED};
use dps_core::config::constants::*;
use dps_core::config::enchants::enchant;
use dps_core::config::essence::ESSENCE_PERKS;
use dps_core::config::item_stats::ITEM_STATS;
use dps_core::config::profile::{BOOK_DAMAGE_OPTION, BOOK_DAMAGE_PER_LEVEL, BOOK_DAMAGE_UNLOCK_LEVEL, BOOK_MAX_LEVEL};
use dps_core::config::upgrades::{ExtraUpgrade, EXTRA_UPGRADES};
use dps_core::formulas::*;
use dps_core::jsmath;
use dps_core::kinds::{Class, FloorMode, Target};
use dps_core::model::Weights;
use dps_core::pets::PetStats;
use dps_core::stat::Stat;

pub const MAX_ACTIVE_EXTRAS: usize = 8;
const _: () = assert!(EXTRA_UPGRADES.len() <= MAX_ACTIVE_EXTRAS);

/// The extras that count right now (JS `activeExtras` result), as `EXTRA_UPGRADES` indexes in the JS order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ActiveExtras {
    idx: [u8; MAX_ACTIVE_EXTRAS],
    len: u8,
}

impl ActiveExtras {
    pub fn push(&mut self, i: usize) {
        self.idx[self.len as usize] = i as u8;
        self.len += 1;
    }
    pub fn len(&self) -> usize {
        self.len as usize
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Indexes into `EXTRA_UPGRADES`, in order.
    pub fn indexes(&self) -> impl Iterator<Item = usize> + '_ {
        self.idx[..self.len as usize].iter().map(|&i| i as usize)
    }
    pub fn iter(&self) -> impl Iterator<Item = &'static ExtraUpgrade> + '_ {
        self.indexes().map(|i| &EXTRA_UPGRADES[i])
    }
    pub fn keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.iter().map(|e| e.key)
    }
    pub fn contains_key(&self, key: &str) -> bool {
        self.iter().any(|e| e.key == key)
    }
}

/// Tuning points spent (JS `result.tuning`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TuningPoints {
    pub attack_speed: f64,
    pub speed: f64,
    pub dps: f64,
}

/// JS `evaluate` result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvalResult {
    /// Final stats per target (`[wither, dragon]`).
    pub stats: [FinalStats; 2],
    pub ap: f64,
    pub speed: f64,
    /// `attackSpeeds.wither`.
    pub attack_speed: f64,
    pub attack_speeds: [f64; 2],
    /// `volleys.wither`.
    pub volleys_per_second: f64,
    pub volleys: [f64; 2],
    pub arrows_per_volley: f64,
    pub tuning: TuningPoints,
    /// Damage per target (`result.wither`, `result.dragon`).
    pub damage: [f64; 2],
    pub extras: ActiveExtras,
}

impl EvalResult {
    #[inline]
    pub fn wither(&self) -> f64 {
        self.damage[0]
    }
    #[inline]
    pub fn dragon(&self) -> f64 {
        self.damage[1]
    }
    /// The damage shown with a plan (`runDps`'s `damage(d)`): per-target damage, Accessory Power, tuning points and the
    /// final stats with the attack speed and speed of each target.
    pub fn to_damage(&self) -> dps_core::model::Damage {
        use dps_core::model::{Damage, StatLine, TargetStats};
        let line = |t: usize| StatLine {
            strength: self.stats[t].strength,
            crit_damage: self.stats[t].crit_damage,
            crit_chance: self.stats[t].crit_chance,
            intelligence: self.stats[t].intelligence,
            ability_damage: self.stats[t].ability_damage,
            attack_speed: self.attack_speeds[t],
            speed: dungeon_speed(self.speed),
        };
        Damage {
            wither: self.damage[0],
            dragon: self.damage[1],
            ap: self.ap,
            tuning: dps_core::model::TuningPoints { attack_speed: self.tuning.attack_speed, speed: self.tuning.speed, dps: self.tuning.dps },
            stats: TargetStats { wither: line(0), dragon: line(1) },
        }
    }

    /// `r.wither * weights.wither + r.dragon * weights.dragon`.
    #[inline]
    pub fn score(&self, w: &Weights) -> f64 {
        self.damage[0] * w.wither + self.damage[1] * w.dragon
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Small helpers (stats.js)
// ---------------------------------------------------------------------------------------------------------------

/// JS `attackSpeedOf(bucket)`.
#[inline]
pub fn attack_speed_of(b: &Bucket) -> f64 {
    b.get(Stat::AttackSpeed) * (1.0 + b.get(Stat::AdditiveAttackSpeed))
}

/// JS `volleysPerSecond(weapon, attackSpeed, ctx)`.
pub fn volleys_per_second(model: &Model, weapon: Option<ItemId>, attack_speed: f64, ctx: &Ctx) -> f64 {
    let a = jsmath::min(jsmath::max(attack_speed, 0.0), ATTACK_SPEED_CAP);
    if ctx.cls == Class::Mage {
        return TICKS_PER_SECOND / dps_core::jsfmt::math_round(MAGE_BEAM_TICKS / (1.0 + a / 100.0));
    }
    if let Some(ticks) = weapon.and_then(|w| model.flags(w).shot_ticks).filter(|&t| t != 0.0 && !t.is_nan()) {
        return TICKS_PER_SECOND / (ticks / (1.0 + a / 100.0) - 1e-9).ceil();
    }
    1.0
}

/// JS `arrowsPerVolley(weapon, extraArrows, ctx)`.
pub fn arrows_per_volley(model: &Model, weapon: Option<ItemId>, extra_arrows: f64, ctx: &Ctx) -> f64 {
    let Some(w) = weapon else { return 1.0 };
    if model.item(w).category.as_deref() != Some("BOW") {
        return 1.0;
    }
    let base = model.flags(w).arrows_base;
    let doubleshot = if ctx.cls == Class::Arch { DOUBLESHOT_BASE + DOUBLESHOT_PER_LEVEL * ctx.archer_level } else { 0.0 };
    base + extra_arrows + doubleshot
}

/// JS `finalStats(bucket, ctx)`.
#[inline]
pub fn final_stats(b: &Bucket, ctx: &Ctx) -> FinalStats {
    let effect = blessing_effect(b.get(Stat::Blessing), b.get(Stat::BlessingShard));
    let bl = &ctx.blessings;
    FinalStats {
        damage: calc_damage(b.get(Stat::Damage)),
        strength: calc_strength(
            b.get(Stat::Strength),
            b.get(Stat::AdditiveStrength),
            b.get(Stat::AdditiveStrength2),
            b.get(Stat::MultStrength),
            effect,
            bl,
        ),
        crit_damage: calc_crit_damage(
            b.get(Stat::CritDamage),
            b.get(Stat::AdditiveCritDamage),
            b.get(Stat::AdditiveCritDamage2),
            b.get(Stat::MultCritDamage),
            effect,
            bl,
        ),
        intelligence: calc_intelligence(
            b.get(Stat::Intelligence),
            b.get(Stat::AdditiveIntelligence),
            b.get(Stat::AdditiveIntelligence2),
            b.get(Stat::MultIntelligence),
            effect,
            bl,
        ),
        crit_chance: calc_crit_chance(b.get(Stat::CritChance), b.get(Stat::AdditiveCritChance), b.get(Stat::MultCritChance)),
        additive_damage: b.get(Stat::AdditiveDamage) + ctx.d.class_additive_damage,
        ability_damage: b.get(Stat::AbilityDamage),
    }
}

/// JS `bookMultiplier(loadout, ctx)`.
pub fn book_multiplier(model: &Model, l: &Loadout, ctx: &Ctx) -> f64 {
    let Some(p) = &ctx.profile_stats else { return 1.0 };
    if !model.bag(l.accessories).has_book {
        return 1.0;
    }
    if l.bop_bonus.as_deref() != Some(BOOK_DAMAGE_OPTION) || (p.sb_level as f64) < BOOK_DAMAGE_UNLOCK_LEVEL {
        return 1.0;
    }
    1.0 + BOOK_DAMAGE_PER_LEVEL * jsmath::min(p.sb_level as f64, BOOK_MAX_LEVEL)
}

/// JS `berserkMeleeMultiplier(ctx)`.
pub fn berserk_melee_multiplier(ctx: &Ctx) -> f64 {
    ctx.d.berserk_melee_multiplier
}

/// JS `archerMultiplier(ctx)`.
pub fn archer_multiplier(ctx: &Ctx) -> f64 {
    ctx.d.archer_multiplier
}

/// JS `weightedGain(before, after, weights)`: weighted percent DPS gain.
pub fn weighted_gain(before: &EvalResult, after: &EvalResult, weights: &Weights) -> f64 {
    let mut total = 0.0;
    for (i, w) in [weights.wither, weights.dragon].into_iter().enumerate() {
        if w == 0.0 || w.is_nan() {
            continue;
        }
        let b = before.damage[i];
        let a = after.damage[i];
        // Going from no damage (e.g. a blade without Wither Impact) to some counts as +100%.
        if b <= 0.0 {
            if a > 0.0 {
                total += w;
            }
            continue;
        }
        let pct = ((a - b) / b) * 100.0;
        total += (pct * w) / 100.0;
    }
    total
}

// ---------------------------------------------------------------------------------------------------------------
// Tuning
// ---------------------------------------------------------------------------------------------------------------

/// JS `tuningAllocation` result: stats to add and points spent.
#[derive(Clone, Copy, Debug)]
pub struct TuningAllocation {
    /// `stats.attackSpeed`.
    pub attack_speed: f64,
    /// `stats.speed`.
    pub speed: f64,
    /// The leftover points' stats, in the split's order.
    pub dps: [(Stat, f64); 6],
    pub dps_len: usize,
    pub attack_speed_points: f64,
    pub speed_points: f64,
    pub dps_points: f64,
}

/// JS `tuningAllocation(points, speed, ctx, attackSpeed, asMult)`; `speed_cap_bonus` is `ctx.speedCapBonus ?? 0`.
pub fn tuning_allocation(
    points: f64,
    speed: f64,
    ctx: &Ctx,
    speed_cap_bonus: f64,
    attack_speed: f64,
    as_mult: f64,
) -> TuningAllocation {
    let as_points = jsmath::min(
        points,
        jsmath::max(0.0, ((ATTACK_SPEED_CAP - attack_speed) / (TUNING_PER_POINT_ATTACK_SPEED * as_mult) - 1e-9).ceil()),
    );
    let left = points - as_points;
    // The cap is dungeon speed (over 100 counts for 2/3), so points fill the stat sheet up to sheetSpeedFor(cap).
    let target = sheet_speed_for(ctx.speed_cap() + speed_cap_bonus);
    let speed_points = jsmath::min(left, jsmath::max(0.0, ((target - speed) / TUNING_PER_POINT_SPEED - 1e-9).ceil()));
    let mut out = TuningAllocation {
        attack_speed: as_points * TUNING_PER_POINT_ATTACK_SPEED,
        speed: speed_points * TUNING_PER_POINT_SPEED,
        dps: [(Stat::Strength, 0.0); 6],
        dps_len: 0,
        attack_speed_points: as_points,
        speed_points,
        dps_points: left - speed_points,
    };
    // distribute(stats, left - speedPoints, ctx)
    let pts = left - speed_points;
    if pts > 0.0 {
        let plan = &ctx.tuning_plan;
        for s in &plan.sinks[..plan.len as usize] {
            out.dps[out.dps_len] = (s.stat, pts * s.share * s.per_point);
            out.dps_len += 1;
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Buckets before tuning
// ---------------------------------------------------------------------------------------------------------------

struct Pre<'a> {
    buckets: [Bucket; 2],
    ap: f64,
    tuning_points: f64,
    pets: [Option<&'a PetStats>; 2],
    speeds: [f64; 2],
}

#[inline]
fn truthy(x: Option<f64>) -> Option<f64> {
    x.filter(|&v| v != 0.0 && !v.is_nan())
}

fn set_bonuses(model: &Model, armor: &SlotMap, cls: Class) -> Loose {
    use dps_core::config::armor_sets::ARMOR_SETS;
    let mut out = Loose::new();
    for (i, s) in ARMOR_SETS.iter().enumerate() {
        if let Some(classes) = s.classes {
            if !classes.contains(&cls) {
                continue;
            }
        }
        let mut n = 0u32;
        for it in armor.values() {
            if model.flags(it).set_mask & (1 << i) != 0 {
                n += 1;
            }
        }
        if n == 0 {
            continue;
        }
        out.add_loose(s.per_piece, n as f64);
        if n >= 4 {
            out.add_loose(s.full, 1.0);
        }
    }
    out
}

/// Everything in `buckets_before_tuning` that does not depend on the active pets: the per-target buckets as they are
/// just before the pet is added, plus the per-loadout values the steps after the pet need.
pub struct PetBase<'l> {
    pre: [Bucket; 2],
    potion: f64,
    shards: &'l crate::itemstats::ShardStats,
    acc: crate::itemstats::AccStats,
    rag_strength: Option<f64>,
    clear_swap: f64,
    skip1: bool,
}

impl<'l> PetBase<'l> {
    /// `l` may carry any active pets; `active` must be the extras `evaluate` would use for every pet variant.
    pub fn new(model: &Model, ctx: &Ctx, l: &'l Loadout, active: &ActiveExtras, skip1: bool) -> PetBase<'l> {
        let variant = (l.has_medallion(model) && model.secret_digit_medallion != 0.0) as usize;
        let gears = [l.gear_view(false), l.gear_view(true)];
        pre_pet(model, ctx, l, active, &gears, variant, skip1)
    }
}

fn pre_pet<'l>(
    model: &Model,
    ctx: &Ctx,
    l: &'l Loadout,
    active: &ActiveExtras,
    gears: &[GearView; 2],
    variant: usize,
    skip1: bool,
) -> PetBase<'l> {
    let rcm = ctx.d.rcm;
    let cls = ctx.cls;

    // Extras: one summed stat object per scope, then added like JS `extraStats` + `add`.
    let mut ex_global = Loose::new();
    let mut ex_wither = Loose::new();
    let mut ex_dragon = Loose::new();
    for e in active.iter() {
        if let Some(w) = e.active_with {
            if !active.contains_key(w) {
                continue;
            }
        }
        if let Some(s) = e.stats {
            ex_global.add_loose(s, 1.0);
        }
        if let Some(s) = e.wither {
            ex_wither.add_loose(s, 1.0);
        }
        if let Some(s) = e.dragon {
            ex_dragon.add_loose(s, 1.0);
        }
    }

    let potion = l.pets.potion_boost(model);
    let shards = l.shards.shard_stats(cls);
    let acc = model.accessory_stats(ctx, l.accessories, l.power, shards.tuning_points, variant);

    // Ragnarock: strength from the ability item, with Chimera copying the strongest owned pet.
    let rag_strength = l.rag.map(|r| {
        let ps = l.pets.rag_strength(model);
        model.rag_strength_with(r, variant, ps)
    });

    // Speed per target only needs the clear boots swap, shared.
    let boots_speed = |item: Option<ItemId>| -> f64 {
        match item {
            Some(i) => model.contribution(i, variant).global.get0(Stat::Speed),
            None => 0.0,
        }
    };
    let clear_swap = if CLEAR_BOOTS_CLASSES.contains(&cls) && !rcm {
        jsmath::max(0.0, boots_speed(l.clear_boots) - boots_speed(l.armor.get(BOOTS)))
    } else {
        0.0
    };

    let mut pre = [Bucket::new(), Bucket::new()];
    for t in 0..2 {
        if skip1 && t == 1 {
            continue;
        }
        let g = &gears[t];
        let weapon_flags: Option<&ItemFlags> = g.weapon.map(|w| model.flags(w));
        let weapon_is_bow = weapon_flags.is_some_and(|f| f.is_bow);
        let mut b = ctx.d.prefix[t][weapon_is_bow as usize];

        // Gear.
        for item in g.items() {
            let c = model.contribution(item, variant);
            b.add_loose(&c.global, 1.0);
            if !rcm {
                b.add_loose(if t == 0 { &c.wither } else { &c.dragon }, 1.0);
            }
        }
        // Dual wield: Prosecute copy early, Execute copy late.
        if weapon_flags.is_some_and(|f| f.dual_wield) && !rcm {
            b.add_one(Stat::AdditiveDamage, DUAL_WIELD_ADDITIVE, 1.0);
        }
        // Legion.
        let mut legion = 0.0;
        for it in g.armor.values() {
            legion += model.flags(it).legion_level * LEGION_PER_LEVEL;
        }
        if legion != 0.0 && !legion.is_nan() {
            let grouped = !rcm && (t == 0 || cls == Class::Mage);
            let lg = legion * if grouped { LEGION_GROUPED_PLAYERS } else { LEGION_PLAYERS };
            b.add_one(Stat::AdditiveStrength, lg, 1.0);
            b.add_one(Stat::AdditiveCritDamage, lg, 1.0);
            b.add_one(Stat::AdditiveIntelligence, lg, 1.0);
            b.add_one(Stat::AdditiveAttackSpeed, lg, 1.0);
            b.add_one(Stat::AdditiveCritChance, lg, 1.0);
        }
        b.add_loose(&set_bonuses(model, &g.armor, cls), 1.0);

        // Extras.
        b.add_loose(&ex_global, 1.0);
        if !rcm {
            b.add_loose(if t == 0 { &ex_wither } else { &ex_dragon }, 1.0);
        }

        pre[t] = b;
    }
    PetBase { pre, potion, shards, acc, rag_strength, clear_swap, skip1 }
}

fn post_pet<'a>(model: &'a Model, ctx: &Ctx, l: &Loadout, gears: &[GearView; 2], variant: usize, base: &PetBase<'_>) -> Pre<'a> {
    let rcm = ctx.d.rcm;
    let (potion, shards, acc, rag_strength, clear_swap) = (base.potion, base.shards, &base.acc, base.rag_strength, base.clear_swap);
    let mut buckets = [Bucket::new(), Bucket::new()];
    let mut pets: [Option<&PetStats>; 2] = [None, None];
    let mut speeds = [0.0; 2];
    for t in 0..2 {
        if base.skip1 && t == 1 {
            buckets[1] = buckets[0];
            pets[1] = pets[0];
            speeds[1] = speeds[0];
            continue;
        }
        let g = &gears[t];
        let weapon_flags: Option<&ItemFlags> = g.weapon.map(|w| model.flags(w));
        let weapon_is_bow = weapon_flags.is_some_and(|f| f.is_bow);
        let mut b = base.pre[t];

        // Active pet for each boss.
        let pet = l.active_pets[t].map(|p| model.pet_stats(p, l.bank));
        pets[t] = pet;
        let weapon = g.weapon;
        if let Some(p) = pet {
            b.add(p.stats.as_slice(), 1.0);
            if !rcm {
                b.add(if t == 0 { p.wither.as_slice() } else { p.dragon.as_slice() }, 1.0);
            }
            // Pet damage perks (Golden Dragon's Legendary Treasure) also raise ability damage.
            b.add_one(Stat::MagicAdditive, p.stats.get(Stat::AdditiveDamage).unwrap_or(0.0), 1.0);
            let sp = &p.special;
            if let Some(v) = truthy(sp.combo_strength) {
                if weapon_is_bow {
                    b.add_one(Stat::Strength, v, 1.0);
                }
            }
            if let Some(v) = truthy(sp.perk_strength) {
                b.add_one(Stat::Strength, v, 1.0);
            }
            if let Some(v) = truthy(sp.perk_speed) {
                b.add_one(Stat::Speed, v, 1.0);
            }
            if let (Some(ws), Some(w)) = (sp.weapon_stats, weapon) {
                let sc = model.scale(w, variant);
                b.add_one(Stat::Damage, crate::itemstats::scale_value(Stat::Damage, ws.damage, sc), 1.0);
                b.add_one(Stat::Strength, crate::itemstats::scale_value(Stat::Strength, ws.strength, sc), 1.0);
            }
            if let (Some(fsb), Some(w)) = (truthy(sp.first_strike_boost), weapon) {
                if !rcm {
                    let st = model.item(w);
                    for key in ["first_strike", "triple_strike"] {
                        let ench = enchant(key).expect("enchant");
                        let level = st.enchants.get(key).copied().unwrap_or(0);
                        if level == 0 || ench.floor_mode != Some(ctx.floor_mode) {
                            continue;
                        }
                        let table = ench.stat_values(Stat::AdditiveDamage).expect("additiveDamage table");
                        let idx = level.min(5) as i64 - 1;
                        let v = if idx >= 0 { table.get(idx as usize).copied().unwrap_or(f64::NAN) } else { f64::NAN };
                        b.add_one(Stat::AdditiveDamage, v * fsb, 1.0);
                    }
                }
            }
            // Blaze: potato books on the weapon count twice.
            let potato_boost = sp.potato_multiplier.unwrap_or(1.0) - 1.0;
            if potato_boost > 0.0 {
                if let Some(w) = weapon {
                    let books = model.flags(w).potatoes * potato_boost;
                    let sc = model.scale(w, variant);
                    b.add_one(Stat::Damage, crate::itemstats::scale_value(Stat::Damage, books * HOT_POTATO.damage, sc), 1.0);
                    b.add_one(Stat::Strength, crate::itemstats::scale_value(Stat::Strength, books * HOT_POTATO.strength, sc), 1.0);
                }
            }
        }
        // Chimera on the weapon (not a Ragnarock): copies the active pet's stats, boosted like the weapon's own.
        if let (Some(w), Some(p)) = (weapon, pet) {
            let f = model.flags(w);
            if f.chimera_level != 0.0 && !f.is_rag_id {
                let sc = model.scale(w, variant);
                for (k, v) in p.stats.iter() {
                    if k.is_flat() || k == Stat::Speed || k == Stat::AttackSpeed || k == Stat::AbilityDamage {
                        let copy = v * CHIMERA_PER_LEVEL * f.chimera_level;
                        b.add_one(k, crate::itemstats::scale_value(k, copy, sc), 1.0);
                    }
                }
            }
        }
        b.add(DUNGEON_POTION, 1.0 + potion);

        // Ragnarock.
        if let Some(s) = rag_strength {
            b.add_one(Stat::Strength, RAGNAROCK_MULTIPLIER * s, 1.0);
        }

        // Essence shop perks.
        for (i, perk) in ESSENCE_PERKS.iter().enumerate() {
            let level = l.perks.at(i);
            if level == 0.0 || level.is_nan() {
                continue;
            }
            let mut pet_crit = 0.0;
            for &(k, v) in perk.per_level {
                if k == Stat::PetCritDamage {
                    pet_crit = v;
                } else {
                    b.add_one(k, v * level, 1.0);
                }
            }
            if pet_crit != 0.0 && !pet_crit.is_nan() {
                if let (Some(pid), Some(ps)) = (l.active_pets[t], pet) {
                    if Some(model.pet(pid).kind.as_str()) == perk.pet {
                        b.add_one(Stat::CritDamage, ps.stats.get(Stat::CritDamage).unwrap_or(0.0) * pet_crit * level, 1.0);
                    }
                }
            }
        }

        // Shards.
        b.add_loose(&shards.global, 1.0);
        if !rcm {
            b.add_loose(if t == 0 { &shards.wither } else { &shards.dragon }, 1.0);
        }

        // Accessory bag.
        b.add_loose(&acc.stats, 1.0);
        if weapon_is_bow {
            let bow = b.get(Stat::BowDamage);
            b.0[Stat::AdditiveDamage.index()] += bow;
        }

        // Speed per target (a pet's speed perk only counts while it is out).
        speeds[t] = BASE_SPEED + EXTRA_SPEED + ctx.profile_speed + b.get(Stat::Speed) + clear_swap;
        buckets[t] = b;
    }
    Pre { buckets, ap: acc.ap, tuning_points: acc.tuning_points, pets, speeds }
}

fn buckets_before_tuning<'a>(
    model: &'a Model,
    ctx: &Ctx,
    l: &Loadout,
    active: &ActiveExtras,
    gears: &[GearView; 2],
    variant: usize,
    skip1: bool,
) -> Pre<'a> {
    let base = pre_pet(model, ctx, l, active, gears, variant, skip1);
    post_pet(model, ctx, l, gears, variant, &base)
}

/// Spend one set's tuning points and add them to its buckets. Returns `(speed after tuning, allocation)`.
fn tuned(
    pre: &mut Pre,
    model: &Model,
    ctx: &Ctx,
    view_weapon: Option<ItemId>,
    targets: &[usize],
) -> (f64, TuningAllocation) {
    let speed_target = if targets.len() == 1 { targets[0] } else { 0 };
    let speed_before = pre.speeds[speed_target];
    let as_need = |b: &Bucket| {
        (ATTACK_SPEED_CAP - attack_speed_of(b)) / (TUNING_PER_POINT_ATTACK_SPEED * (1.0 + b.get(Stat::AdditiveAttackSpeed)))
    };
    let mut binding = targets[0];
    if targets.len() > 1 {
        let mut best = as_need(&pre.buckets[binding]);
        for &t in targets {
            let v = as_need(&pre.buckets[t]);
            if v > best {
                binding = t;
                best = v;
            }
        }
    }
    // Attack speed only matters for shortbows (shots per second) and the mage beam.
    let needs_attack_speed = (ctx.cls == Class::Mage && !ctx.d.rcm)
        || view_weapon.and_then(|w| model.flags(w).shot_ticks).is_some_and(|t| t != 0.0 && !t.is_nan());
    let speed_cap_bonus = pre.pets[speed_target].and_then(|p| p.special.speed_cap_bonus).unwrap_or(0.0);
    let alloc = tuning_allocation(
        pre.tuning_points,
        speed_before,
        ctx,
        speed_cap_bonus,
        if needs_attack_speed { attack_speed_of(&pre.buckets[binding]) } else { ATTACK_SPEED_CAP },
        1.0 + pre.buckets[binding].get(Stat::AdditiveAttackSpeed),
    );
    for &t in targets {
        let b = &mut pre.buckets[t];
        b.add_one(Stat::AttackSpeed, alloc.attack_speed, 1.0);
        b.add_one(Stat::Speed, alloc.speed, 1.0);
        for &(k, v) in &alloc.dps[..alloc.dps_len] {
            b.add_one(k, v, 1.0);
        }
    }
    (speed_before + alloc.speed, alloc)
}

// ---------------------------------------------------------------------------------------------------------------
// evaluateWith / activeExtras / evaluate
// ---------------------------------------------------------------------------------------------------------------

/// JS `evaluateWith(loadout, ctx, activeExtras)`; the result's `extras` is `active`.
pub fn evaluate_with(model: &Model, ctx: &Ctx, l: &Loadout, active: &ActiveExtras) -> EvalResult {
    evaluate_with_skip(model, ctx, l, active, false)
}

pub fn evaluate_with_skip(model: &Model, ctx: &Ctx, l: &Loadout, active: &ActiveExtras, skip1: bool) -> EvalResult {
    evaluate_with_base(model, ctx, l, active, skip1, None)
}

/// `evaluate_with_skip`, optionally starting from a [`PetBase`] built for the same loadout apart from `active_pets`
/// (and for the same `active` and `skip1`).
pub fn evaluate_with_base(
    model: &Model,
    ctx: &Ctx,
    l: &Loadout,
    active: &ActiveExtras,
    skip1: bool,
    base: Option<&PetBase<'_>>,
) -> EvalResult {
    debug_assert!(model.accepts(ctx), "Model and Ctx disagree on the item context");
    let rcm = ctx.d.rcm;
    let variant = (l.has_medallion(model) && model.secret_digit_medallion != 0.0) as usize;
    let gears = [l.gear_view(false), l.gear_view(true)];
    let mut pre = match base {
        Some(b) => post_pet(model, ctx, l, &gears, variant, b),
        None => buckets_before_tuning(model, ctx, l, active, &gears, variant, skip1),
    };

    // rawStats: with a dragon set each target is tuned on its own; the result carries the withers' numbers.
    let (speed, alloc) = if l.dragon_gear.is_some() {
        let w = tuned(&mut pre, model, ctx, gears[0].weapon, &[0]);
        tuned(&mut pre, model, ctx, gears[1].weapon, &[1]);
        w
    } else {
        tuned(&mut pre, model, ctx, gears[0].weapon, &[0, 1])
    };
    let ap = pre.ap;
    let buckets = pre.buckets;

    let arrows = arrows_per_volley(model, l.weapon, buckets[0].get(Stat::Arrows), ctx);
    let attack_speeds = [attack_speed_of(&buckets[0]), attack_speed_of(&buckets[1])];
    let volleys = [
        volleys_per_second(model, gears[0].weapon, attack_speeds[0], ctx),
        volleys_per_second(model, gears[1].weapon, attack_speeds[1], ctx),
    ];
    let mut result = EvalResult {
        stats: [FinalStats::default(); 2],
        ap,
        speed,
        attack_speed: attack_speeds[0],
        attack_speeds,
        volleys_per_second: volleys[0],
        volleys,
        arrows_per_volley: arrows,
        tuning: TuningPoints {
            attack_speed: alloc.attack_speed_points,
            speed: alloc.speed_points,
            dps: alloc.dps_points,
        },
        damage: [0.0; 2],
        extras: *active,
    };

    let book = book_multiplier(model, l, ctx);
    let cls = ctx.cls;
    for t in 0..2 {
        if skip1 && t == 1 {
            continue;
        }
        let g = &gears[t];
        let target = Target::ALL[t];
        let shots = volleys[t];
        let base = final_stats(&buckets[t], ctx);
        let defense = target_defense(target);
        let weapon = g.weapon.map(|w| model.flags(w));
        let cloak = g.equipment.get(CLOAK).map(|c| model.flags(c));
        let sa_cloak = if cloak.is_some_and(|c| c.sa_cloak) && weapon.is_some_and(|w| w.has_wi) {
            ctx.sa_cloak_multiplier(shots)
        } else {
            1.0
        };
        let blade = if t == 0 && weapon.is_some_and(|w| w.wi_weapon_id) { NECRON_BLADE_WITHER_MULTIPLIER } else { 1.0 };
        let fabled = (if weapon.is_some_and(|w| w.fabled) { FABLED_MULTIPLIER } else { 1.0 }) * sa_cloak * blade * book;
        if rcm {
            // One Wither Impact cast against a clear mob.
            let has_blade = weapon.is_some_and(|w| w.has_wi);
            let loving = if g.armor.get(CHESTPLATE).is_some_and(|c| model.flags(c).loving) { LOVING_ABILITY_MULTIPLIER } else { 1.0 };
            let implosion = if g.equipment.get(BELT).is_some_and(|c| model.flags(c).implosion_belt) { IMPLOSION_BELT_MULTIPLIER } else { 1.0 };
            result.damage[t] = if has_blade {
                rcm_damage(
                    &base,
                    0.0,
                    cata_boost(ctx.cata_level),
                    buckets[t].get(Stat::MagicAdditive) + ctx.d.class_additive_damage,
                    loving * implosion * book,
                )
            } else {
                0.0
            };
            result.stats[t] = base;
            continue;
        }
        let overload = weapon.is_some_and(|w| w.overload);
        let pet_deaths_touch = l.active_pets[t]
            .map(|p| model.pet_stats(p, l.bank).special.deaths_touch.unwrap_or(0.0))
            .unwrap_or(0.0);
        let boss_dps = |s: &FinalStats| -> f64 {
            let hit_with = |m: f64| -> f64 {
                match cls {
                    Class::Arch => archer_damage(s, defense, ctx.d.archer_multiplier * m, overload),
                    Class::Bers => berserker_damage(s, defense, 0.0, m, overload),
                    Class::Mage => mage_damage(s, defense, m),
                }
            };
            let mut dmg = hit_with(fabled);
            // Ferocity (Berserker vs withers only).
            if cls == Class::Bers && t == 0 {
                let ferocity = buckets[t].get(Stat::Ferocity) * (1.0 + buckets[t].get(Stat::AdditiveFerocity));
                if ferocity > 0.0 {
                    dmg += FEROCITY_EFFECTIVENESS * (ferocity / 100.0) * hit_with(fabled * ctx.d.berserk_melee_multiplier);
                }
            }
            // Thunderlord: every 3rd hit adds lightning for a % of the hit.
            let thunder = weapon.map_or(0.0, |w| w.thunder_pct);
            if thunder != 0.0 && !thunder.is_nan() {
                dmg += hit_with(fabled * thunder / 100.0) / THUNDERLORD_EVERY;
            }
            let mut dps = dmg * arrows * shots;
            // Wither Skeleton Death's Touch: one extra hit of 67% every second.
            if pet_deaths_touch != 0.0 && !pet_deaths_touch.is_nan() {
                dps += hit_with(fabled * pet_deaths_touch);
            }
            // Fire Aspect (withers only).
            let fire = if t == 0 { weapon.map_or(0.0, |w| w.fire_pct) } else { 0.0 };
            if fire != 0.0 && !fire.is_nan() {
                dps += hit_with(fabled * fire / 100.0);
            }
            dps
        };
        let mut s = base;
        result.damage[t] = boss_dps(&s);
        // Mage at 1B+ DPS kills things in so few hits that First Strike counts in Master Mode too.
        let first_strike =
            if cls == Class::Mage && ctx.floor_mode != FloorMode::Normal { weapon.map_or(0.0, |w| w.first_strike_add) } else { 0.0 };
        if first_strike != 0.0 && !first_strike.is_nan() && result.damage[t] >= MAGE_FIRST_STRIKE_DPS {
            s = FinalStats { additive_damage: base.additive_damage + first_strike, ..base };
            result.damage[t] = boss_dps(&s);
        }
        result.stats[t] = s;
    }
    result
}

fn dominates(a: &ExtraUpgrade, b: &ExtraUpgrade) -> bool {
    let scopes = |e: &ExtraUpgrade| [e.stats, e.wither, e.dragon];
    let get = |s: Option<&'static [(Stat, f64)]>, k: Stat| s.and_then(|s| dps_core::stat::get(s, k)).unwrap_or(0.0);
    let (sa, sb) = (scopes(a), scopes(b));
    let every = (0..3).all(|i| sb[i].unwrap_or(&[]).iter().all(|&(k, v)| get(sa[i], k) >= v));
    let some = (0..3).any(|i| sa[i].unwrap_or(&[]).iter().any(|&(k, v)| v > get(sb[i], k)));
    every && some
}

/// JS `activeExtras(loadout, ctx, weights)`: owned extras that count right now. In each group, the member that gives
/// the most damage wins.
pub fn active_extras(model: &Model, ctx: &Ctx, l: &Loadout, weights: Option<&Weights>) -> ActiveExtras {
    active_extras_scored(model, ctx, l, weights).0
}

/// `active_extras`, plus whether any group had to be decided by scoring (that score depends on the active pets).
pub fn active_extras_scored(model: &Model, ctx: &Ctx, l: &Loadout, weights: Option<&Weights>) -> (ActiveExtras, bool) {
    let mut scored = false;
    // owned = EXTRA_UPGRADES.filter(owned && allowed)
    let mut fixed = ActiveExtras::default();
    let mut groups: [(&str, [u8; MAX_ACTIVE_EXTRAS], usize); MAX_ACTIVE_EXTRAS] = [("", [0; MAX_ACTIVE_EXTRAS], 0); MAX_ACTIVE_EXTRAS];
    let mut ngroups = 0;
    for (i, e) in EXTRA_UPGRADES.iter().enumerate() {
        if !l.extras.has_index(i) {
            continue;
        }
        if e.classes.is_some_and(|c| !c.contains(&ctx.cls)) {
            continue;
        }
        if e.requires.is_some_and(|r| !l.extras.has(r)) {
            continue;
        }
        match e.group {
            None => fixed.push(i),
            Some(g) => {
                let gi = match groups[..ngroups].iter().position(|x| x.0 == g) {
                    Some(p) => p,
                    None => {
                        groups[ngroups].0 = g;
                        ngroups += 1;
                        ngroups - 1
                    }
                };
                let n = groups[gi].2;
                groups[gi].1[n] = i as u8;
                groups[gi].2 += 1;
            }
        }
    }
    let mut active = fixed;
    for g in &groups[..ngroups] {
        let all = &g.1[..g.2];
        // A member at least as strong as another on every stat wins without scoring both.
        let mut members = [0u8; MAX_ACTIVE_EXTRAS];
        let mut nm = 0;
        for &m in all {
            let dominated = all.iter().any(|&o| o != m && dominates(&EXTRA_UPGRADES[o as usize], &EXTRA_UPGRADES[m as usize]));
            if !dominated {
                members[nm] = m;
                nm += 1;
            }
        }
        let mut best = members[0];
        if nm > 1 {
            scored = true;
            let mut best_score = f64::NEG_INFINITY;
            let (ww, wd) = (weights.map_or(50.0, |w| w.wither), weights.map_or(50.0, |w| w.dragon));
            for &m in &members[..nm] {
                let mut trial = active;
                trial.push(m as usize);
                let r = evaluate_with(model, ctx, l, &trial);
                let score = r.damage[0] * ww + r.damage[1] * wd;
                if score > best_score {
                    best_score = score;
                    best = m;
                }
            }
        }
        active.push(best as usize);
    }
    // Extras tied to a group member (pet items) only count with that member.
    let mut out = ActiveExtras::default();
    for i in active.indexes() {
        match EXTRA_UPGRADES[i].active_with {
            Some(w) if !active.contains_key(w) => {}
            _ => out.push(i),
        }
    }
    (out, scored)
}

/// JS `evaluate(loadout, ctx, weights)`.
pub fn evaluate(model: &Model, ctx: &Ctx, l: &Loadout, weights: &Weights) -> EvalResult {
    let extras = active_extras(model, ctx, l, Some(weights));
    evaluate_with(model, ctx, l, &extras)
}

pub fn evaluate_scoring(model: &Model, ctx: &Ctx, l: &Loadout, weights: &Weights) -> EvalResult {
    let extras = active_extras(model, ctx, l, Some(weights));
    let skip1 = ctx.d.rcm && weights.dragon == 0.0 && l.dragon_gear.is_none();
    evaluate_with_skip(model, ctx, l, &extras, skip1)
}

/// `ITEM_STATS[id]?.shotTicks` is truthy.
pub fn has_shot_ticks(id: &str) -> bool {
    ITEM_STATS.iter().find(|d| d.id == id).and_then(|d| d.shot_ticks).is_some_and(|t| t != 0.0)
}
