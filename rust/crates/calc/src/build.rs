//! `buildLoadout` (src/calc/upgrades.js): the profile's start loadout. Picks the best owned item for each slot in two
//! passes, the Mage dragon set, the clear boots, the consumed Rift Prism, the RCM blade, spare blades, powers and the
//! leftover tuning split.

use crate::arena::Model;
use crate::ctx::Ctx;
use crate::helpers::{has_wither_impact, new_item};
use crate::ids::*;
use crate::loadout::{LevelKind, Levels, Loadout, Owned, PetList};
use crate::parts::rcm_blade_score;
use crate::pick::Evaluator;
use dps_core::config::accessories::{default_power, FREE_POWERS, POWERS, RIFT_PRISM_ID, ap_override};
use dps_core::config::constants::*;
use dps_core::config::upgrades::EXTRA_UPGRADES;
use dps_core::kinds::Mode;
use dps_core::model::{ItemState, Member, Weights};
use indexmap::IndexMap;
use std::sync::Arc;

/// Mutable pieces of a start loadout under construction, with the index (in `member.items`) of every worn item object:
/// the JS identifies items by identity, so a byte-identical copy of the worn piece is a different item (it may sit in
/// the dragon set, and a spare blade is a copy of the worn one).
#[derive(Clone)]
struct Work {
    l: Loadout,
    /// JS `loadout.bank` is undefined until the first `cloneLoadout` (which makes it `bank ?? 0`).
    bank_defined: bool,
    /// Per `by_slot` position: the item object in the main set / the dragon set.
    main_idx: [Option<usize>; 10],
    dragon_idx: [Option<usize>; 10],
}

impl Work {
    /// JS `cloneLoadout(loadout)`.
    fn cloned(&self, zero_bank: BankId) -> Work {
        let mut w = self.clone();
        if !w.bank_defined {
            w.l.bank = zero_bank;
            w.bank_defined = true;
        }
        w
    }

    /// JS `setSlot(l, slot, item)`: the dragon set never repeats the main set's piece (same object).
    fn set_slot(&mut self, slot: Slot, (item, idx): Opt) {
        let pos = by_slot_index(slot);
        self.l.set_slot_keep_dragon(slot, item);
        self.main_idx[pos] = Some(idx);
        if let Slot::Gear(g) = slot {
            if self.dragon_idx[pos] == Some(idx) {
                self.l.store_dragon_slot(g, None);
                self.dragon_idx[pos] = None;
            }
        }
    }

    /// JS `setDragonSlot(l, slot, item)`: an object the main set already uses is not stored.
    fn set_dragon_slot(&mut self, slot: GearSlot, item: Option<Opt>) {
        let pos = by_slot_index(Slot::Gear(slot));
        let value = item.filter(|&(_, idx)| self.main_idx[pos] != Some(idx));
        self.l.store_dragon_slot(slot, value.map(|v| v.0));
        self.dragon_idx[pos] = value.map(|v| v.1);
    }
}

/// An option for a slot: the interned item and the position of the item object in `member.items`.
type Opt = (ItemId, usize);

/// Archer / Berserker with Maxor's boots owned: the boss boots are Maxor's or Necron's only.
fn boss_boots_allowed(owned: &Owned, id: &str, cls: dps_core::Class) -> bool {
    if !CLEAR_BOOTS_CLASSES.contains(&cls) {
        return true;
    }
    !owned.contains(MAXOR_BOOTS_ID) || id == MAXOR_BOOTS_ID || id == NECRON_BOOTS_ID
}

fn accessory_rank(item: &ItemState) -> i32 {
    (if item.recomb { 2 } else { 0 }) + item.enrichment.is_some() as i32
}

/// JS `buildLoadout(profile, ctx, weights)`. Like the JS it stores the chosen leftover tuning split on the context.
pub fn build_loadout(model: &mut Model, ctx: &mut Ctx, weights: Weights, member: &Member) -> Loadout {
    let cls = ctx.cls;
    let weapon_cats = class_weapon_categories(cls);
    let items = &member.items;
    let mut l = Loadout::empty(model);

    // Slot order of the JS object: weapon, rag, armor slots, equipment slots.
    let mut by_slot: Vec<(Slot, Vec<Opt>)> = vec![(Slot::Gear(GearSlot::Weapon), vec![]), (Slot::Rag, vec![])];
    for i in 0..4 {
        by_slot.push((Slot::Gear(GearSlot::Armor(i)), vec![]));
    }
    for i in 0..4 {
        by_slot.push((Slot::Gear(GearSlot::Equipment(i)), vec![]));
    }
    let slot_pos = |s: Slot| by_slot_index(s);

    let mut owned_ids: Vec<String> = Vec::with_capacity(items.len() + 1);
    let mut acc: IndexMap<String, ItemId> = IndexMap::new();
    let mut interned: Vec<ItemId> = Vec::with_capacity(items.len());
    for it in items {
        interned.push(model.intern_item(it.clone()));
    }
    for (idx, item) in items.iter().enumerate() {
        owned_ids.push(item.id.clone());
        if ctx.excluded_item(&item.id) {
            continue;
        }
        let id = interned[idx];
        if item.id == RAGNAROCK_ID {
            by_slot[slot_pos(Slot::Rag)].1.push((id, idx));
        }
        let cat = item.category.as_deref();
        if cat.is_some_and(|c| weapon_cats.contains(&c)) {
            by_slot[0].1.push((id, idx));
        } else if let Some(pos) = cat.and_then(|c| {
            let key = equipment_slot(c).unwrap_or(c);
            ARMOR_SLOTS.iter().position(|&s| s == key).map(|i| 2 + i).or_else(|| EQUIPMENT_SLOTS.iter().position(|&s| s == key).map(|i| 6 + i))
        }) {
            by_slot[pos].1.push((id, idx));
        } else if cat == Some(ACCESSORY_CATEGORY) {
            let replace = match acc.get(&item.id) {
                None => true,
                Some(&prev) => accessory_rank(item) > accessory_rank(model.item(prev)),
            };
            if replace {
                acc.insert(item.id.clone(), id);
            }
        }
    }
    l.owned = Owned::from_ids(owned_ids.iter().cloned());
    let bag_ids: Vec<ItemId> = acc.values().copied().collect();
    l.accessories = model.intern_bag(&bag_ids);

    // Consumed Rift Prism. The profile's highest Accessory Power is the truth: the prism's 11 is only added when the
    // bag comes up exactly 11 short of it (or when the profile gives no Accessory Power at all).
    let gap = member.highest_ap.map(|h| h - model.total_accessory_power(l.accessories));
    let prism_ap = ap_override(RIFT_PRISM_ID);
    let add_prism = (gap.is_some() && gap == prism_ap) || (gap.is_none() && member.rift_prism == Some(true));
    if add_prism && !acc.contains_key(RIFT_PRISM_ID) {
        let p = model.intern_item(new_item(RIFT_PRISM_ID, &ctx.resource));
        acc.insert(RIFT_PRISM_ID.to_string(), p);
        let ids: Vec<ItemId> = acc.values().copied().collect();
        l.accessories = model.intern_bag(&ids);
    }
    if add_prism || member.rift_prism == Some(true) {
        l.owned.insert(RIFT_PRISM_ID);
    }

    for (i, e) in EXTRA_UPGRADES.iter().enumerate() {
        if e.owned_items.is_some_and(|o| o.iter().any(|x| l.owned.contains(x))) {
            l.extras.insert_index(i);
        }
    }
    // Necron's Blade variants you own, and whether any has Wither Impact (for the Shadow Assassin Cloak in LCM).
    let blade_idx: Vec<usize> = (0..items.len()).filter(|&i| WITHER_IMPACT_WEAPONS.contains(&items[i].id.as_str())).collect();
    l.blades = blade_idx.iter().map(|&i| interned[i]).collect::<Vec<_>>().into();
    // Mage LCM with two or more Necron's Blades: the one built for the clear stays the RCM blade.
    let mut rcm_blade: Option<usize> = None;
    if cls == dps_core::Class::Mage && ctx.mode != Mode::Rcm && blade_idx.len() >= 2 {
        let mut best = blade_idx[0];
        for &b in &blade_idx {
            if rcm_blade_score(&items[b]) > rcm_blade_score(&items[best]) {
                best = b;
            }
        }
        if rcm_blade_score(&items[best]) > 0.0 {
            rcm_blade = Some(best);
            by_slot[0].1.retain(|&(_, idx)| idx != best);
        }
    }
    l.wither_impact = blade_idx.iter().any(|&i| has_wither_impact(&items[i]));
    {
        let pos = slot_pos(Slot::Gear(GearSlot::Armor(BOOTS as u8)));
        let owned = l.owned.clone();
        by_slot[pos].1.retain(|&(id, _)| boss_boots_allowed(&owned, &model.item(id).id, cls));
    }
    // Clear boots: your fastest boots, worn during the clear for speed. Mage clears in RCM armor, so it has none.
    l.clear_boots = if CLEAR_BOOTS_CLASSES.contains(&cls) && ctx.mode != Mode::Rcm {
        let pos = slot_pos(Slot::Gear(GearSlot::Armor(BOOTS as u8)));
        let speed = |m: &Model, x: Option<ItemId>| x.map(|i| m.contribution(i, 0).global.get0(dps_core::Stat::Speed)).unwrap_or(0.0);
        let mut best: Option<ItemId> = None;
        for &(b, _) in &by_slot[pos].1 {
            if speed(model, Some(b)) > speed(model, best) {
                best = Some(b);
            }
        }
        best
    } else {
        None
    };
    // Pets are assumed level 100 (Golden Dragon 200); a held item that is missing counts as none.
    l.shards = Levels::new(LevelKind::Shards, member.shards.iter().map(|(k, &v)| (k.clone(), v as f64)).collect());
    l.perks = Levels::new(LevelKind::Perks, member.perks.iter().map(|(k, &v)| (k.clone(), v)).collect());
    l.pets = {
        let ids = member.pets.iter().map(|p| model.pet_id(p)).collect();
        PetList::new(ids)
    };
    // The bank (JS `loadout.bank ?? ctx.petProfile.bank ?? 0`) counts from the start; power, bopBonus and the
    // learned powers are only set after the slot passes.
    l.bank = model.bank_id(ctx.pet_profile.bank.unwrap_or(0.0));

    let zero_bank = model.bank_id(0.0);
    let bank_final = model.bank_id(ctx.pet_profile.bank.filter(|b| *b != 0.0 && !b.is_nan()).unwrap_or(0.0));
    let ev = Evaluator::new(model, ctx, weights);
    let l = ev.select_pet(&l);

    let dragon_set = ctx.has_dragon_set();
    let mut w = Work { l, bank_defined: false, main_idx: [None; 10], dragon_idx: [None; 10] };
    for _pass in 0..2 {
        for (slot, options) in &by_slot {
            let mut best: Option<Work> = None;
            let mut best_score = f64::NEG_INFINITY;
            for &opt in options {
                let mut trial = w.cloned(zero_bank);
                trial.set_slot(*slot, opt);
                let s = ev.score(&trial.l);
                if s > best_score {
                    best_score = s;
                    best = Some(trial);
                }
                // Mage: the new piece for withers only, keeping the current one for dragons.
                let pos = by_slot_index(*slot);
                let current = if *slot == Slot::Rag { None } else { gear_slot(*slot).and_then(|g| w.l.main_item(g)).zip(w.main_idx[pos]) };
                if let (true, Some(cur), Some(g)) = (dragon_set, current, gear_slot(*slot)) {
                    if cur.1 != opt.1 && w.l.dragon_item(g).is_none() {
                        let mut swap = w.cloned(zero_bank);
                        swap.set_slot(*slot, opt);
                        swap.set_dragon_slot(g, Some(cur));
                        let s2 = ev.score(&swap.l);
                        if s2 > best_score {
                            best_score = s2;
                            best = Some(swap);
                        }
                    }
                }
            }
            if let Some(b) = best {
                w = b;
            }
        }
        // Mage: the best dragon piece for each slot (none = the main set's).
        if !dragon_set {
            continue;
        }
        for (slot, options) in &by_slot {
            if *slot == Slot::Rag {
                continue;
            }
            let g = gear_slot(*slot).expect("gear slot");
            let mut best: Option<Work> = None;
            let mut best_score = ev.score(&w.l);
            for &opt in options {
                let mut trial = w.cloned(zero_bank);
                trial.set_dragon_slot(g, Some(opt));
                let s = ev.score(&trial.l);
                if s > best_score {
                    best_score = s;
                    best = Some(trial);
                }
            }
            if let Some(b) = best {
                w = b;
            }
        }
    }

    let mut l = ev.select_pet(&w.l);
    // Blades left over (not the RCM one, not worn): they can become a dual-wield copy or another set's weapon.
    l.rcm_blade = rcm_blade.map(|i| interned[i]);
    l.spare_blades = (0..items.len())
        .filter(|&i| {
            (WITHER_IMPACT_WEAPONS.contains(&items[i].id.as_str()) || DUAL_WIELD_WEAPONS.contains(&items[i].id.as_str()))
                && Some(i) != rcm_blade
                && Some(i) != w.main_idx[0]
                && Some(i) != w.dragon_idx[0]
        })
        .map(|i| interned[i])
        .collect::<Vec<_>>()
        .into();
    // Powers you have learned are free to switch: use the best one. With no power on the profile, the class default is
    // assumed learned.
    let current = ctx.power.clone().unwrap_or_else(|| default_power(cls).to_string());
    let mut unlocked: Vec<u8> = Vec::new();
    for p in FREE_POWERS.iter().map(|s| s.to_string()).chain(member.unlocked_powers.iter().cloned()).chain(std::iter::once(current.clone())) {
        if let Some(i) = POWERS.iter().position(|x| x.key == p) {
            if !unlocked.contains(&(i as u8)) {
                unlocked.push(i as u8);
            }
        }
    }
    l.unlocked_powers = unlocked.into();
    l.set_power(Some(&current));
    l.bop_bonus = ctx.profile_stats.as_ref().and_then(|p| p.bop_bonus.as_deref()).map(Arc::from);
    // Coins in the bank, from the bank API (0 when it is off or there is no bank): Golden Dragon's Legendary Treasure.
    l.bank = bank_final;
    let l = ev.select_power(&l);
    // Tuning is free to change, so leftover points use the best split for this loadout, stored on ctx for the rest
    // of the run.
    let split = ev.best_tuning_split(&l);
    ctx.set_tuning_split(split);
    let ev = Evaluator::new(model, ctx, weights);
    ev.select_pet(&l)
}

fn by_slot_index(s: Slot) -> usize {
    match s {
        Slot::Gear(GearSlot::Weapon) => 0,
        Slot::Rag => 1,
        Slot::Gear(GearSlot::Armor(i)) => 2 + i as usize,
        Slot::Gear(GearSlot::Equipment(i)) => 6 + i as usize,
    }
}

fn gear_slot(s: Slot) -> Option<GearSlot> {
    match s {
        Slot::Gear(g) => Some(g),
        Slot::Rag => None,
    }
}
