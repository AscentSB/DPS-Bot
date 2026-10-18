//! Per-item stats (src/calc/stats.js: `baseItemStats`, `itemRawStats`, `itemContribution`, accessory stats, shard
//! stats). Every number is accumulated in the JS order of operations.

use crate::arena::*;
use crate::ctx::Ctx;
use crate::ids::*;
use crate::loose::Loose;
use dps_core::config::accessories::{enrichment, POWERS};
use dps_core::config::constants::*;
use dps_core::config::enchants::enchant;
use dps_core::config::gemstones::gem_stat;
use dps_core::config::item_stats::item_stats;
use dps_core::config::shards::{Shard, ShardFamily, SHARDS};
use dps_core::data::get_reforge;
use dps_core::drops::{drop_stat_multiplier, floor_tier_index};
use dps_core::formulas::{cata_boost, dungeon_crit_multiplier, dungeon_multiplier};
use dps_core::jsmath;
use dps_core::kinds::{Class, FloorMode, Mode, StatTarget};
use dps_core::model::Resource;
use dps_core::stat::{stat_alias, Stat};
use std::sync::Arc;

/// JS `itemKey`-independent helper: `scaleBucket(stats, item, ctx, dungeon)` with the multipliers already worked out
/// (`None` = not a dungeon item: unchanged).
pub fn scale_stats(stats: &Loose, scale: Option<(f64, f64)>) -> Loose {
    let Some((flat, crit)) = scale else {
        return *stats;
    };
    let mut out = *stats;
    let mut m = stats.mask;
    while m != 0 {
        let i = m.trailing_zeros() as usize;
        m &= m - 1;
        let s = Stat::ALL[i];
        if s.is_star_only() {
            out.v[i] = stats.v[i] * crit;
        } else if s.is_flat() || s == Stat::Speed {
            out.v[i] = stats.v[i] * flat;
        }
    }
    out
}

/// One value through `scaleBucket`.
#[inline]
pub fn scale_value(stat: Stat, value: f64, scale: Option<(f64, f64)>) -> f64 {
    match scale {
        None => value,
        Some((flat, crit)) => {
            if stat.is_star_only() {
                value * crit
            } else if stat.is_flat() || stat == Stat::Speed {
                value * flat
            } else {
                value
            }
        }
    }
}

/// JS `baseItemStats(id, resource, cls, cataLevel, floorMode)` (uncached).
pub fn compute_base_stats(id: &str, resource: &Resource, cls: Option<Class>, cata_level: f64, floor_mode: FloorMode) -> Triple {
    let info = resource.get(id);
    let ov = item_stats(id);
    let class_ok = match ov.and_then(|o| o.classes) {
        None => true,
        Some(classes) => cls.is_none_or(|c| classes.contains(&c)),
    };
    let mut stats = Loose::new();
    if !ov.is_some_and(|o| o.replace_stats) {
        if let Some(info) = info {
            for (api_key, &value) in &info.stats {
                if let Some(key) = stat_alias(api_key) {
                    stats.add_one(key, value, 1.0);
                }
            }
            if let Some(tiered) = &info.tiered_stats {
                for (api_key, values) in tiered {
                    let Some(key) = stat_alias(api_key) else { continue };
                    let Some(arr) = values.as_array() else { continue };
                    if arr.is_empty() {
                        continue;
                    }
                    let v = arr[floor_tier_index(cata_level, arr.len())].as_f64().unwrap_or(0.0);
                    stats.add_one(key, v, 1.0);
                }
            }
            // Per-Catacombs-level base stats (Hyperion +1 damage and +2 intelligence per level).
            if let Some(o) = ov {
                if !o.per_cata_level.is_empty() && cata_level.is_finite() {
                    stats.add_loose(o.per_cata_level, cata_level);
                }
            }
            let boost = drop_stat_multiplier(id, resource);
            if boost != 1.0 {
                let mut m = stats.mask;
                while m != 0 {
                    let i = m.trailing_zeros() as usize;
                    m &= m - 1;
                    stats.v[i] *= boost;
                }
            }
        }
    }
    // Master Mode only bonuses (Master Skulls) do nothing in Normal Mode.
    let mode_ok = !(ov.is_some_and(|o| o.master_mode_only) && floor_mode == FloorMode::Normal);
    if class_ok && mode_ok {
        if let Some(o) = ov {
            stats.add_loose(o.stats, 1.0);
        }
    }
    Triple {
        global: stats,
        wither: if class_ok { ov.map(|o| Loose::from_slice(o.wither)).unwrap_or_default() } else { Loose::new() },
        dragon: if class_ok { ov.map(|o| Loose::from_slice(o.dragon)).unwrap_or_default() } else { Loose::new() },
    }
}

/// Result of JS `shardStats`.
#[derive(Clone, Copy, Debug)]
pub struct ShardStats {
    pub global: Loose,
    pub wither: Loose,
    pub dragon: Loose,
    pub tuning_points: f64,
}

/// JS `computeShardStats(loadout, ctx)` for shard levels given by `level(index in SHARDS)`.
pub fn compute_shard_stats(levels: &dyn Fn(usize) -> f64, cls: Class) -> ShardStats {
    let mut out = ShardStats { global: Loose::new(), wither: Loose::new(), dragon: Loose::new(), tuning_points: 0.0 };
    let level = |i: usize| jsmath::max(0.0, jsmath::min(10.0, levels(i)));
    let idx = |id: &str| SHARDS.iter().position(|s| s.id == id);
    let tiamat_level = idx("TIAMAT").map(level).unwrap_or(0.0);
    let tiamat_echo = SHARDS.iter().find(|s| s.id == "TIAMAT").map(|s| s.echo_per_level()).unwrap_or(0.0);
    let tiamat = 1.0 + tiamat_echo * tiamat_level;
    // echo[family] = 1 + perLevel.echo * level * tiamat (later shards overwrite earlier ones of a family)
    let mut echo: [Option<f64>; 5] = [None; 5];
    for (i, s) in SHARDS.iter().enumerate() {
        if let Some(fam) = s.echo {
            if fam != ShardFamily::Echo {
                echo[family_slot(fam)] = Some(1.0 + s.echo_per_level() * level(i) * tiamat);
            }
        }
    }
    for (i, s) in SHARDS.iter().enumerate() {
        if s.echo.is_some() || level(i) == 0.0 || level(i).is_nan() {
            continue;
        }
        if let Some(classes) = s.classes {
            if !classes.contains(&cls) {
                continue;
            }
        }
        let mult = shard_mult(s, &echo);
        let lv = level(i);
        let target = match s.target {
            None | Some(StatTarget::Global) => &mut out.global,
            Some(StatTarget::Wither) => &mut out.wither,
            Some(StatTarget::Dragon) => &mut out.dragon,
        };
        for &(k, v) in s.per_level {
            if k == Stat::TuningPoints {
                if v != 0.0 {
                    out.tuning_points += v * lv * mult;
                }
                continue;
            }
            target.add_one(k, v, lv * mult);
        }
    }
    out.tuning_points = out.tuning_points.floor();
    out
}

fn family_slot(f: ShardFamily) -> usize {
    match f {
        ShardFamily::Elemental => 0,
        ShardFamily::Unlimited => 1,
        ShardFamily::Ruler => 2,
        ShardFamily::Box => 3,
        ShardFamily::Echo => 4,
    }
}

fn shard_mult(s: &Shard, echo: &[Option<f64>; 5]) -> f64 {
    // (s.family && echo[s.family]) || 1
    match s.family.and_then(|f| echo[family_slot(f)]) {
        Some(m) if m != 0.0 && !m.is_nan() => m,
        _ => 1.0,
    }
}

impl Model {
    #[inline]
    pub(crate) fn secret_digit(&self, variant: usize) -> f64 {
        if variant == 1 {
            self.secret_digit_medallion
        } else {
            0.0
        }
    }

    #[inline]
    fn is_rcm(&self) -> bool {
        self.cls == Class::Mage && self.mode == Mode::Rcm
    }

    /// JS `baseItemStats(id, resource, cls, cataLevel, floorMode)` for this model's context, cached by id.
    pub fn base_item_stats(&self, id: &str) -> Arc<Triple> {
        if let Some(t) = self.base_cache.read().expect("base cache").get(id) {
            return t.clone();
        }
        let t = Arc::new(compute_base_stats(id, &self.resource, Some(self.cls), self.cata_level, self.floor_mode));
        self.base_cache.write().expect("base cache").insert(id.to_string(), t.clone());
        t
    }

    /// JS `itemRawStats(item, ctx)` (unscaled stats of one item by target).
    pub fn raw_stats(&self, id: ItemId, variant: usize) -> &Triple {
        self.items[id.index()].variants[variant].raw.get_or_init(|| Box::new(self.compute_raw(id, variant, 0.0)))
    }

    /// JS `itemRawStats(item, ctx, { chimeraPetStrength })` (not cached).
    pub fn raw_stats_chimera(&self, id: ItemId, variant: usize, chimera_pet_strength: f64) -> Triple {
        if chimera_pet_strength == 0.0 || chimera_pet_strength.is_nan() {
            return *self.raw_stats(id, variant);
        }
        self.compute_raw(id, variant, chimera_pet_strength)
    }

    /// `itemRawStats(item, ctx, { chimeraPetStrength }).global.strength ?? 0`, remembered for the few pet strengths a
    /// run sees (lock free).
    pub fn rag_strength_with(&self, id: ItemId, variant: usize, chimera_pet_strength: f64) -> f64 {
        if chimera_pet_strength == 0.0 || chimera_pet_strength.is_nan() {
            return self.raw_stats(id, variant).global.get0(Stat::Strength);
        }
        let bits = chimera_pet_strength.to_bits();
        for slot in &self.items[id.index()].variants[variant].rag {
            match slot.get() {
                Some(&(b, v)) if b == bits => return v,
                Some(_) => {}
                None => {
                    let v = self.compute_raw(id, variant, chimera_pet_strength).global.get0(Stat::Strength);
                    let _ = slot.set((bits, v));
                    return v;
                }
            }
        }
        self.compute_raw(id, variant, chimera_pet_strength).global.get0(Stat::Strength)
    }

    /// JS `itemContribution(item, ctx)`: stats after dungeon scaling.
    pub fn contribution(&self, id: ItemId, variant: usize) -> &Triple {
        self.items[id.index()].variants[variant].contrib.get_or_init(|| {
            let raw = self.raw_stats(id, variant);
            let scale = self.scale(id, variant);
            Box::new(Triple {
                global: scale_stats(&raw.global, scale),
                wither: scale_stats(&raw.wither, scale),
                dragon: scale_stats(&raw.dragon, scale),
            })
        })
    }

    /// The `(flat, crit)` multipliers `scaleBucket` uses for this item, `None` when it is not a dungeon item.
    pub fn scale(&self, id: ItemId, variant: usize) -> Option<(f64, f64)> {
        *self.items[id.index()].variants[variant].scale.get_or_init(|| {
            let e = &self.items[id.index()];
            if !e.flags.is_dungeon {
                return None;
            }
            let sd = self.secret_digit(variant);
            let master_stars = if self.floor_mode == FloorMode::Normal { 0.0 } else { e.state.master_stars as f64 };
            let flat = dungeon_multiplier(self.cata_level, MAX_STARS, master_stars, sd);
            let crit = dungeon_crit_multiplier(MAX_STARS, master_stars, sd);
            Some((flat, crit))
        })
    }

    /// JS `accessoryItemStats(item, ctx)`: one accessory's stats with its enrichment and dungeon boost.
    pub fn accessory_item_stats(&self, id: ItemId, variant: usize) -> &Loose {
        self.items[id.index()].variants[variant].acc.get_or_init(|| {
            let e = &self.items[id.index()];
            let mut stats = self.raw_stats(id, variant).global;
            if let Some(en) = e.state.enrichment.as_deref().and_then(enrichment) {
                stats.add_loose(en.stats, 1.0);
            }
            if e.flags.info_dungeon_item {
                let boost = 1.0 + cata_boost(self.cata_level);
                let mut m = stats.mask;
                while m != 0 {
                    let i = m.trailing_zeros() as usize;
                    m &= m - 1;
                    let s = Stat::ALL[i];
                    if (s.is_flat() || s == Stat::Speed) && !s.is_star_only() {
                        stats.v[i] *= boost;
                    }
                }
            }
            Box::new(stats)
        })
    }

    /// The bag's counted accessories, Accessory Power and summed stats (JS `computeBagStats` without the power).
    pub fn bag_base(&self, bag: BagId, variant: usize) -> &BagBase {
        self.bags[bag.index()].base[variant].get_or_init(|| {
            let counted = self.counted_accessories(bag);
            let mut ap = 0.0;
            let mut loose = Loose::new();
            for &item in &counted {
                ap += self.items[item.index()].flags.ap;
                loose.add_loose_from(self.accessory_item_stats(item, variant), 1.0);
            }
            Box::new(BagBase { power_mult: dps_core::formulas::power_multiplier(ap), counted, ap, loose })
        })
    }

    /// JS `countedAccessories(loadout)`: ignored ones dropped, one (the best) per family, non-family items first.
    pub fn counted_accessories(&self, bag: BagId) -> Vec<ItemId> {
        let mut out: Vec<ItemId> = Vec::new();
        let mut best: indexmap::IndexMap<u32, ItemId, rustc_hash::FxBuildHasher> = indexmap::IndexMap::default();
        for &item in self.bags[bag.index()].items.iter() {
            let f = &self.items[item.index()].flags;
            if f.acc_ignored {
                continue;
            }
            match f.acc_family {
                None => out.push(item),
                Some(fam) => {
                    let better = match best.get(&fam) {
                        None => true,
                        Some(&prev) => f.acc_rank > self.items[prev.index()].flags.acc_rank,
                    };
                    if better {
                        best.insert(fam, item);
                    }
                }
            }
        }
        out.extend(best.values().copied());
        out
    }

    /// JS `totalAccessoryPower(loadout, resource)`.
    pub fn total_accessory_power(&self, bag: BagId) -> f64 {
        self.bag_base(bag, 0).ap
    }

    /// JS `accessoryStats(loadout, ctx, extraTuningPoints)`: the bag's stats including the power's, AP, tuning points.
    pub fn accessory_stats(&self, ctx: &Ctx, bag: BagId, power: PowerField, extra_tuning_points: f64, variant: usize) -> AccStats {
        let base = self.bag_base(bag, variant);
        let mut stats = base.loose;
        let p = &POWERS[match power {
            PowerField::Unset => ctx.d.ctx_power.unwrap_or(ctx.d.default_power),
            PowerField::Known(i) => i as usize,
            PowerField::Unknown => ctx.d.default_power,
        }];
        stats.add_loose(p.stats, base.power_mult);
        stats.add_loose(p.bonus, 1.0);
        let points = (base.ap / dps_core::config::accessories::AP_PER_TUNING_POINT).floor() + extra_tuning_points;
        AccStats { stats, ap: base.ap, tuning_points: points }
    }

    /// JS `computeItemRawStats(item, ctx, { chimeraPetStrength })`.
    fn compute_raw(&self, id: ItemId, variant: usize, chimera_pet_strength: f64) -> Triple {
        let _ = variant;
        let e = &self.items[id.index()];
        let st = &e.state;
        let mut out = *self.base_item_stats(&st.id);
        let rarity = stat_rarity(item_rarity(&st.base_rarity, st.recomb, &st.id));
        let weapon = e.flags.is_weapon_category;
        let rcm = self.is_rcm();

        if let Some(reforge) = st.reforge.as_deref().filter(|r| !r.is_empty()).and_then(get_reforge) {
            if reforge.applies_to(&st.id, st.category.as_deref()) {
                if let Some(s) = reforge.stats_for(rarity) {
                    out.global.add_loose(s, 1.0);
                }
                if let Some(cs) = reforge.cata_stat {
                    out.global.add_one(cs, self.cata_level, 1.0);
                }
            }
        }

        for g in st.gems.slots.values() {
            if let Some((stat, value)) = gem_stat(&g.gem, &g.quality, rarity) {
                out.global.add_one(stat, value, 1.0);
            }
        }

        if weapon {
            let potatoes = e.flags.potatoes;
            out.global.add_one(Stat::Damage, potatoes * HOT_POTATO.damage, 1.0);
            out.global.add_one(Stat::Strength, potatoes * HOT_POTATO.strength, 1.0);
            if st.art_of_war {
                out.global.add_one(Stat::Strength, ART_OF_WAR_STRENGTH, 1.0);
            }
        }

        let cls = self.cls;
        for (name, &level) in &st.enchants {
            let Some(ench) = enchant(name) else { continue };
            if level < 1 || !st.category.as_deref().is_some_and(|c| ench.applies_to_category(c)) {
                continue;
            }
            if let Some(items) = ench.items {
                if !items.contains(&st.id.as_str()) {
                    continue;
                }
            }
            if let Some(classes) = ench.classes {
                if !classes.contains(&cls) {
                    continue;
                }
            }
            if ench.chimera {
                out.global.add_one(Stat::Strength, CHIMERA_PER_LEVEL * level as f64 * chimera_pet_strength, 1.0);
                continue;
            }
            if ench.legion || ench.thunder.is_some() {
                continue;
            }
            if let Some(the_one) = ench.the_one {
                let idx = (level.min(the_one.len() as i32) - 1) as usize;
                out.global.add_one(Stat::Strength, the_one.get(idx).copied().unwrap_or(0.0) * self.maxed, 1.0);
                continue;
            }
            // Master Mode / Normal Mode enchants; the clear set has its own list that works on both floors.
            if let Some(fm) = ench.floor_mode {
                if !rcm && fm != self.floor_mode {
                    continue;
                }
            }
            let enemies = if ench.swarm {
                if rcm {
                    RCM_SWARM_ENEMIES
                } else {
                    BOSS_SWARM_ENEMIES
                }
            } else {
                1.0
            };
            for &(stat, values) in ench.stats {
                let value = values[(level.min(values.len() as i32) - 1) as usize] * enemies;
                match ench.target {
                    StatTarget::Global => out.global.add_one(stat, value, 1.0),
                    StatTarget::Wither => out.wither.add_one(stat, value, 1.0),
                    StatTarget::Dragon => out.dragon.add_one(stat, value, 1.0),
                }
                // Mage RCM: only Giant Killer, Prosecute, Swarm and Smite raise Wither Impact damage.
                if stat == Stat::AdditiveDamage && weapon && rcm && RCM_ENCHANTS.contains(&name.as_str()) {
                    out.global.add_one(Stat::MagicAdditive, value, 1.0);
                }
            }
        }
        // Items like Necron heads multiply everything on them (base, reforge, gems, enchants).
        if let Some(m) = item_stats(&st.id).and_then(|o| o.stat_multiplier) {
            if m != 0.0 {
                for bucket in [&mut out.global, &mut out.wither, &mut out.dragon] {
                    let mut mask = bucket.mask;
                    while mask != 0 {
                        let i = mask.trailing_zeros() as usize;
                        mask &= mask - 1;
                        let s = Stat::ALL[i];
                        if s.is_flat() || s == Stat::Speed {
                            bucket.v[i] *= m;
                        }
                    }
                }
            }
        }
        out
    }
}

/// Result of [`Model::accessory_stats`].
#[derive(Clone, Copy, Debug)]
pub struct AccStats {
    pub stats: Loose,
    pub ap: f64,
    pub tuning_points: f64,
}
