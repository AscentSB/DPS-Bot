//! The [`Evaluator`] and the choices built on `evaluate`: `score`, `selectPet`, `dpsPets`, `selectPower` and
//! `bestTuningSplit` (src/calc/stats.js and src/calc/upgrades.js).

use crate::arena::Model;
use crate::ctx::Ctx;
use crate::eval::{evaluate, EvalResult};
use crate::ids::*;
use crate::loadout::{Loadout, PetList};
use crate::memo::{EvalKey, EvalMemo};
use dps_core::config::accessories::{tuning_stats, POWERS};
use dps_core::config::constants::ATTACK_SPEED_CAP;
use dps_core::config::pets::{phase_pet, EXCLUDED_PETS, TIER_BOOST_ID};
use dps_core::kinds::{Class, Target};
use dps_core::model::Weights;
use dps_core::pets::pet_stats;
use dps_core::stat::Stat;

/// Model + context + weights (+ optional memo): what scoring a loadout needs. `Sync`: share one between threads.
#[derive(Clone, Copy)]
pub struct Evaluator<'a> {
    pub model: &'a Model,
    pub ctx: &'a Ctx,
    pub weights: Weights,
    pub memo: Option<&'a EvalMemo>,
}

impl<'a> Evaluator<'a> {
    pub fn new(model: &'a Model, ctx: &'a Ctx, weights: Weights) -> Evaluator<'a> {
        debug_assert!(model.accepts(ctx), "Model and Ctx disagree on the item context");
        Evaluator { model, ctx, weights, memo: None }
    }

    pub fn with_memo(mut self, memo: &'a EvalMemo) -> Evaluator<'a> {
        self.memo = if memo.matches(self.ctx) && memo.bind_weights(&self.weights) { Some(memo) } else { None };
        self
    }

    /// JS `evaluate(loadout, ctx, weights)`, through the memo when there is one.
    pub fn evaluate(&self, l: &Loadout) -> EvalResult {
        match self.memo {
            None => crate::eval::evaluate_scoring(self.model, self.ctx, l, &self.weights),
            Some(memo) => {
                let key = EvalKey::new(self.model, l);
                if let Some(r) = memo.get(&key) {
                    return r;
                }
                let r = crate::eval::evaluate_scoring(self.model, self.ctx, l, &self.weights);
                memo.insert(key, r);
                r
            }
        }
    }

    /// JS `score(loadout, ctx, weights)`.
    pub fn score(&self, l: &Loadout) -> f64 {
        let r = self.evaluate(l);
        r.damage[0] * self.weights.wither + r.damage[1] * self.weights.dragon
    }

    /// JS `dpsPets(pets, ctx)`: owned pets that can matter for damage, one per type (highest rarity, keeping a held
    /// item). Cached on the pet list.
    pub fn dps_pets<'p>(&self, pets: &'p PetList) -> &'p [PetId] {
        pets.dps.get_or_init(|| {
            const ORDER: [&str; 6] = ["COMMON", "UNCOMMON", "RARE", "EPIC", "LEGENDARY", "MYTHIC"];
            const USEFUL: [Stat; 8] = [
                Stat::Strength,
                Stat::CritDamage,
                Stat::CritChance,
                Stat::Intelligence,
                Stat::AdditiveDamage,
                Stat::AdditiveStrength,
                Stat::AttackSpeed,
                Stat::AbilityDamage,
            ];
            let tier_rank = |t: &str| ORDER.iter().position(|&x| x == t).map(|i| i as i32).unwrap_or(-1);
            let mut by_type: Vec<(&str, &str, PetId)> = Vec::new();
            for &id in &pets.ids {
                let entry = &self.model.pets[id.index()];
                let pet = &entry.pet;
                if EXCLUDED_PETS.contains(&pet.kind.as_str()) {
                    continue;
                }
                let compute = || {
                    let s = pet_stats(pet, &self.ctx.pet_profile);
                    USEFUL.iter().any(|&k| s.stats.get(k).is_some_and(|v| v != 0.0 && !v.is_nan()))
                        || !s.dragon.is_empty()
                        || !s.wither.is_empty()
                };
                let useful = if entry.per_bank { compute() } else { *entry.useful.get_or_init(compute) };
                if !useful {
                    continue;
                }
                let held = pet.held_item.as_deref().unwrap_or("");
                match by_type.iter_mut().find(|(k, h, _)| *k == pet.kind.as_str() && *h == held) {
                    Some(slot) => {
                        if tier_rank(&pet.tier) > tier_rank(&self.model.pet(slot.2).tier) {
                            slot.2 = id;
                        }
                    }
                    None => by_type.push((pet.kind.as_str(), held, id)),
                }
            }
            by_type.into_iter().map(|t| t.2).collect()
        })
    }

    /// The `dps_pets` list of `parent.ids + [new]`, derived from the parent's cached list instead of rescanning every
    /// pet: the new pet replaces the same-type entry when it has a strictly higher tier, or is appended.
    pub fn dps_after_buy(&self, parent: &PetList, new: PetId) -> Vec<PetId> {
        const ORDER: [&str; 6] = ["COMMON", "UNCOMMON", "RARE", "EPIC", "LEGENDARY", "MYTHIC"];
        const USEFUL: [Stat; 8] = [
            Stat::Strength,
            Stat::CritDamage,
            Stat::CritChance,
            Stat::Intelligence,
            Stat::AdditiveDamage,
            Stat::AdditiveStrength,
            Stat::AttackSpeed,
            Stat::AbilityDamage,
        ];
        let tier_rank = |t: &str| ORDER.iter().position(|&x| x == t).map(|i| i as i32).unwrap_or(-1);
        let mut v: Vec<PetId> = self.dps_pets(parent).to_vec();
        let entry = &self.model.pets[new.index()];
        let pet = &entry.pet;
        if EXCLUDED_PETS.contains(&pet.kind.as_str()) {
            return v;
        }
        let compute = || {
            let s = pet_stats(pet, &self.ctx.pet_profile);
            USEFUL.iter().any(|&k| s.stats.get(k).is_some_and(|x| x != 0.0 && !x.is_nan()))
                || !s.dragon.is_empty()
                || !s.wither.is_empty()
        };
        let useful = if entry.per_bank { compute() } else { *entry.useful.get_or_init(compute) };
        if !useful {
            return v;
        }
        let held = pet.held_item.as_deref().unwrap_or("");
        let pos = v.iter().position(|&o| {
            let op = self.model.pet(o);
            op.kind == pet.kind && op.held_item.as_deref().unwrap_or("") == held
        });
        match pos {
            Some(i) => {
                if tier_rank(&pet.tier) > tier_rank(&self.model.pet(v[i]).tier) {
                    v[i] = new;
                }
            }
            None => v.push(new),
        }
        v
    }

    /// JS `selectPet(loadout, ctx, weights)`: the class's phase pet when owned, otherwise the owned pet that does
    /// the most damage against that boss.
    pub fn select_pet(&self, l: &Loadout) -> Loadout {
        let ctx = self.ctx;
        let model = self.model;
        let dps = self.dps_pets(&l.pets);
        let rcm = ctx.is_rcm();
        let mut active = l.active_pets;
        let mut work = l.clone();
        // Only the active pets change below, so the gear/set-bonus part of the buckets is built once. That needs
        // extras that do not depend on the pet (no group decided by scoring) and no memo to go through.
        let skip1 = rcm && self.weights.dragon == 0.0 && l.dragon_gear.is_none();
        let (extras, base) = if self.memo.is_none() {
            let (ex, scored) = crate::eval::active_extras_scored(model, ctx, l, Some(&self.weights));
            let b = if scored { None } else { Some(crate::eval::PetBase::new(model, ctx, l, &ex, skip1)) };
            (ex, b)
        } else {
            (Default::default(), None)
        };
        for target in Target::ALL {
            let t = target.index();
            // RCM scores the clear only, so the dragon pet changes nothing.
            if rcm && target == Target::Dragon {
                active[t] = None;
                continue;
            }
            let preferred = if rcm { Some("GOLDEN_DRAGON") } else { phase_pet(target, ctx.cls) };
            let pool: Vec<PetId> = dps.iter().copied().filter(|&p| Some(model.pet(p).kind.as_str()) == preferred).collect();
            let mut best: Option<PetId> = None;
            let mut best_score = f64::NEG_INFINITY;
            let candidates: Vec<Option<PetId>> = if pool.is_empty() {
                std::iter::once(None).chain(dps.iter().copied().map(Some)).collect()
            } else {
                pool.into_iter().map(Some).collect()
            };
            for pet in candidates {
                work.active_pets = active;
                work.active_pets[t] = pet;
                let r = match &base {
                    Some(b) => crate::eval::evaluate_with_base(model, ctx, &work, &extras, skip1, Some(b)),
                    None => self.evaluate(&work),
                };
                // Mage can only use a Tier Boosted Ender Dragon if attack speed still reaches the cap some other way.
                if ctx.cls == Class::Mage {
                    if let Some(p) = pet {
                        let pp = model.pet(p);
                        if pp.kind == "ENDER_DRAGON"
                            && pp.held_item.as_deref() == Some(TIER_BOOST_ID)
                            && r.attack_speeds[t] < ATTACK_SPEED_CAP
                        {
                            continue;
                        }
                    }
                }
                let score = r.damage[0] * self.weights.wither + r.damage[1] * self.weights.dragon;
                if score > best_score {
                    best_score = score;
                    best = pet;
                }
            }
            active[t] = best;
        }
        let mut out = l.clone();
        out.active_pets = active;
        out
    }

    /// JS `selectPower(loadout, ctx, weights)`: the best learned power (switching at Maxwell is free).
    pub fn select_power(&self, l: &Loadout) -> Loadout {
        let mut best = l.power;
        let mut best_score = f64::NEG_INFINITY;
        let mut work = l.clone();
        for &p in l.unlocked_powers.iter() {
            if (p as usize) >= POWERS.len() {
                continue;
            }
            work.power = PowerField::Known(p);
            let s = self.score(&work);
            if s > best_score {
                best_score = s;
                best = PowerField::Known(p);
            }
        }
        if best == l.power {
            l.clone()
        } else {
            let mut out = l.clone();
            out.power = best;
            out
        }
    }

    /// JS `bestTuningSplit(loadout, ctx, weights)`: every split of 10 tenths over the class's tuning stats; the
    /// first that beats the best by more than 1e-9 wins. `None` when the class has no tuning stats.
    pub fn best_tuning_split(&self, l: &Loadout) -> Option<Vec<(Stat, f64)>> {
        let stats = tuning_stats(self.ctx.cls, self.ctx.mode);
        if stats.is_empty() {
            return None;
        }
        let mut best: Option<Vec<(Stat, f64)>> = None;
        let mut best_score = f64::NEG_INFINITY;
        let mut split: Vec<(Stat, f64)> = Vec::with_capacity(stats.len());
        // One copy of the context for every split (a copy per split cloned the profile and collections each time).
        let mut ctx = self.ctx.clone();
        self.walk(l, &mut ctx, stats, 0, 10, &mut split, &mut best, &mut best_score);
        best
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        &self,
        l: &Loadout,
        ctx: &mut Ctx,
        stats: &[Stat],
        i: usize,
        left: i32,
        split: &mut Vec<(Stat, f64)>,
        best: &mut Option<Vec<(Stat, f64)>>,
        best_score: &mut f64,
    ) {
        if i == stats.len() - 1 {
            let mut full = split.clone();
            full.push((stats[i], left as f64 / 10.0));
            ctx.set_tuning_split(Some(full.clone()));
            let r = evaluate(self.model, ctx, l, &self.weights);
            let score = r.damage[0] * self.weights.wither + r.damage[1] * self.weights.dragon;
            if score > *best_score + 1e-9 {
                *best_score = score;
                *best = Some(full);
            }
            return;
        }
        for n in 0..=left {
            split.push((stats[i], n as f64 / 10.0));
            self.walk(l, ctx, stats, i + 1, left - n, split, best, best_score);
            split.pop();
        }
    }
}
