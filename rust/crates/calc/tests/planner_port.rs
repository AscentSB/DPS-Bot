//! Ports of the checks in test/planner.test.js that only use functions of this crate (stats.js and the loadout
//! helpers of upgrades.js). The JS tests build their start loadout with `buildLoadout` (part 2); `build` below is a
//! small stand-in that picks the best owned item per slot the same way.

use dps_calc::helpers::*;
use dps_calc::ids::*;
use dps_calc::*;
use dps_core::config::accessories::{FREE_POWERS, POWERS};
use dps_core::config::constants::*;
use dps_core::config::upgrades::EXTRA_UPGRADES;
use dps_core::kinds::{Class, FloorMode, Mode};
use dps_core::model::{Cake, ItemInfo, ItemState, Pet, PetProfile, ProfileStats, Resource, Tuning, Weights};
use dps_core::stat::Stat;
use serde_json::{json, Value};
use std::sync::Arc;

fn close(a: f64, b: f64) {
    close_eps(a, b, 1e-6);
}
fn close_eps(a: f64, b: f64, eps: f64) {
    assert!((a - b).abs() <= eps * b.abs().max(1.0), "{a} != {b}");
}

fn info(v: Value) -> (String, ItemInfo) {
    let i: ItemInfo = serde_json::from_value(v).unwrap();
    (i.id.clone(), i)
}

fn base_resource() -> Resource {
    let entries = vec![
        json!({"id":"HYPERION","name":"Hyperion","category":"SWORD","tier":"LEGENDARY","dungeon_item":true,"stats":{"DAMAGE":260,"STRENGTH":150,"INTELLIGENCE":350}}),
        json!({"id":"GIANTS_SWORD","name":"Giant's Sword","category":"SWORD","tier":"LEGENDARY","dungeon_item":true,"stats":{"DAMAGE":500}}),
        json!({"id":"TERMINATOR","name":"Terminator","category":"BOW","tier":"LEGENDARY","dungeon_item_conversion_cost":{"essence_type":"DRAGON","amount":300},"stats":{"DAMAGE":310,"STRENGTH":50,"CRITICAL_DAMAGE":250}}),
        json!({"id":"POWER_WITHER_LEGGINGS","name":"Necron's Leggings","category":"LEGGINGS","tier":"LEGENDARY","dungeon_item":true,"stats":{"STRENGTH":40,"CRITICAL_DAMAGE":30}}),
        json!({"id":"SOULWEAVER_GLOVES","name":"Soulweaver Gloves","category":"GLOVES","tier":"EPIC","dungeon_item":true,"stats":{"strength":10,"critical_damage":10}}),
        json!({"id":"RAGNAROCK_AXE","name":"Ragnarock","category":"SWORD","tier":"RARE","stats":{"DAMAGE":200,"STRENGTH":100}}),
        json!({"id":"SPEED_TALISMAN","name":"Speed Talisman","category":"ACCESSORY","tier":"COMMON"}),
        json!({"id":"SPEED_RING","name":"Speed Ring","category":"ACCESSORY","tier":"UNCOMMON"}),
        json!({"id":"WITHER_RELIC","name":"Wither Relic","category":"ACCESSORY","tier":"LEGENDARY"}),
        json!({"id":"MASTER_SKULL_TIER_7","name":"Master Skull - Tier 7","category":"ACCESSORY","tier":"LEGENDARY","dungeon_item":true}),
        json!({"id":"GENERAL_MEDALLION","name":"General's Medallion","category":"ACCESSORY","tier":"RARE","dungeon_item":true}),
    ];
    entries.into_iter().map(info).collect()
}

fn with_items(base: &Resource, extra: Vec<Value>) -> Arc<Resource> {
    let mut r = base.clone();
    for e in extra {
        let (id, i) = info(e);
        r.insert(id, i);
    }
    Arc::new(r)
}

fn res() -> Arc<Resource> {
    Arc::new(base_resource())
}

fn mk_ctx(cls: Class, cata: f64, res: &Arc<Resource>, f: impl FnOnce(&mut CtxInit)) -> Ctx {
    let mut init = CtxInit::new(cls, cata, res.clone());
    f(&mut init);
    Ctx::new(init)
}

fn weights(cls: Class) -> Weights {
    weights_for(cls, None, Mode::Lcm, FloorMode::Master)
}

fn pet(kind: &str, tier: &str, held: Option<&str>) -> Pet {
    Pet { kind: kind.into(), tier: tier.into(), held_item: held.map(String::from) }
}

fn new_it(res: &Resource, id: &str) -> ItemState {
    new_item(id, res)
}

fn with(mut it: ItemState, f: impl FnOnce(&mut ItemState)) -> ItemState {
    f(&mut it);
    it
}

fn enchants(it: &mut ItemState, list: &[(&str, i32)]) {
    it.enchants = list.iter().map(|&(k, v)| (k.to_string(), v)).collect();
}

fn scrolled(mut it: ItemState) -> ItemState {
    it.scrolls = WITHER_SCROLLS.iter().map(|s| s.to_string()).collect();
    it
}

/// Stand-in for `buildLoadout`: the best owned item per slot (two passes), pets, power, tuning split.
struct World {
    model: Model,
    ctx: Ctx,
    w: Weights,
}

impl World {
    fn new(ctx: Ctx, w: Weights) -> World {
        let model = Model::new(&ctx);
        World { model, ctx, w }
    }

    fn ev(&self) -> Evaluator<'_> {
        Evaluator::new(&self.model, &self.ctx, self.w)
    }

    fn eval(&self, l: &Loadout) -> EvalResult {
        self.ev().evaluate(l)
    }

    fn intern(&mut self, it: &ItemState) -> ItemId {
        self.model.intern_item(it.clone())
    }

    fn build(&mut self, items: &[ItemState], pets: &[Pet], shards: &[(&str, f64)], perks: &[(&str, f64)]) -> Loadout {
        let cls = self.ctx.cls;
        let mut l = Loadout::empty(&mut self.model);
        let weapon_cats = class_weapon_categories(cls);
        let mut by_slot: Vec<(Slot, Vec<ItemId>)> = vec![(Slot::Gear(GearSlot::Weapon), vec![]), (Slot::Rag, vec![])];
        for i in 0..4 {
            by_slot.push((Slot::Gear(GearSlot::Armor(i)), vec![]));
        }
        for i in 0..4 {
            by_slot.push((Slot::Gear(GearSlot::Equipment(i)), vec![]));
        }
        let mut owned: Vec<String> = Vec::new();
        let mut acc: indexmap::IndexMap<String, ItemId> = indexmap::IndexMap::new();
        for it in items {
            owned.push(it.id.clone());
            if self.ctx.excluded_item(&it.id) {
                continue;
            }
            let id = self.intern(it);
            if it.id == RAGNAROCK_ID {
                by_slot[1].1.push(id);
            }
            let cat = it.category.as_deref().unwrap_or("");
            if weapon_cats.contains(&cat) {
                by_slot[0].1.push(id);
            } else if let Some(slot) = ARMOR_SLOTS.iter().position(|&s| s == cat) {
                by_slot[2 + slot].1.push(id);
            } else if let Some(slot) = equipment_slot(cat).and_then(|s| EQUIPMENT_SLOTS.iter().position(|&x| x == s)) {
                by_slot[6 + slot].1.push(id);
            } else if cat == ACCESSORY_CATEGORY {
                let rank = |x: &ItemState| (if x.recomb { 2 } else { 0 }) + (x.enrichment.is_some() as i32);
                match acc.get(&it.id) {
                    Some(&prev) if rank(it) <= rank(self.model.item(prev)) => {}
                    _ => {
                        acc.insert(it.id.clone(), id);
                    }
                }
            }
        }
        let acc_ids: Vec<ItemId> = acc.values().copied().collect();
        l.accessories = self.model.intern_bag(&acc_ids);
        l.owned = Owned::from_ids(owned.iter().cloned());
        for e in EXTRA_UPGRADES {
            if e.owned_items.is_some_and(|o| o.iter().any(|x| owned.iter().any(|y| y == x))) {
                l.extras.insert(e.key);
            }
        }
        l.shards = Levels::new(LevelKind::Shards, shards.iter().map(|&(k, v)| (k.to_string(), v)).collect());
        l.perks = Levels::new(LevelKind::Perks, perks.iter().map(|&(k, v)| (k.to_string(), v)).collect());
        let pet_ids: Vec<PetId> = pets.iter().map(|p| self.model.pet_id(p)).collect();
        l.pets = PetList::new(pet_ids);
        l.bop_bonus = self.ctx.profile_stats.as_ref().and_then(|p| p.bop_bonus.as_deref()).map(Arc::from);
        l.bank = self.model.bank_id(self.ctx.pet_profile.bank.unwrap_or(0.0));
        // Powers.
        let current = self.ctx.power.clone().unwrap_or_else(|| dps_core::config::accessories::default_power(cls).to_string());
        let mut unlocked: Vec<u8> = Vec::new();
        for p in FREE_POWERS.iter().map(|s| s.to_string()).chain(std::iter::once(current.clone())) {
            if let Some(i) = POWERS.iter().position(|x| x.key == p) {
                if !unlocked.contains(&(i as u8)) {
                    unlocked.push(i as u8);
                }
            }
        }
        l.unlocked_powers = unlocked.into();
        l.power = match POWERS.iter().position(|x| x.key == current) {
            Some(i) => PowerField::Known(i as u8),
            None => PowerField::Unknown,
        };
        // Clear boots.
        if self.ctx.clear_boots_used() {
            let mut best: Option<ItemId> = None;
            for &b in &by_slot[5].1 {
                let speed = |x: ItemId| self.model.contribution(x, 0).global.get0(Stat::Speed);
                if best.is_none_or(|bb| speed(b) > speed(bb)) {
                    best = Some(b);
                }
            }
            l.clear_boots = best;
        }
        l = self.ev().select_pet(&l);
        let dragon_set = self.ctx.has_dragon_set();
        let _ = dragon_set;
        for _pass in 0..2 {
            for (slot, options) in &by_slot {
                let mut best: Option<Loadout> = None;
                let mut best_score = f64::NEG_INFINITY;
                for &opt in options {
                    let mut trial = l.clone();
                    trial.set_slot(*slot, opt);
                    let s = self.ev().score(&trial);
                    if s > best_score {
                        best_score = s;
                        best = Some(trial);
                    }
                }
                if let Some(b) = best {
                    l = b;
                }
            }
        }
        l = self.ev().select_pet(&l);
        l = self.ev().select_power(&l);
        let split = self.ev().best_tuning_split(&l);
        self.ctx.set_tuning_split(split);
        self.ev().select_pet(&l)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Item stats
// ---------------------------------------------------------------------------------------------------------------

fn contribution(w: &mut World, it: &ItemState, variant: usize) -> Triple {
    let id = w.intern(it);
    *w.model.contribution(id, variant)
}

#[test]
fn necron_leggings_with_ancient() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let legs = with(new_it(&res, "POWER_WITHER_LEGGINGS"), |i| i.reforge = Some("ancient".into()));
    let c = contribution(&mut w, &legs, 0);
    close(c.global.get0(Stat::Strength), 65.0 * 6.35);
    close(c.global.get0(Stat::CritDamage), 80.0 * 6.35);
    close(c.global.get0(Stat::CritChance), 12.0 * 1.5);
}

#[test]
fn lowercase_api_stat_keys() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let c = contribution(&mut w, &new_it(&res, "SOULWEAVER_GLOVES"), 0);
    close(c.global.get0(Stat::Strength), 10.0 * 6.35);
}

#[test]
fn diamond_necron_head_with_ancient() {
    let res = with_items(
        &base_resource(),
        vec![json!({"id":"DIAMOND_NECRON_HEAD","name":"Diamond Necron Head","category":"HELMET","tier":"SPECIAL","dungeon_item":true,"stats":{"STRENGTH":40}})],
    );
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let head = with(new_it(&res, "DIAMOND_NECRON_HEAD"), |i| i.reforge = Some("ancient".into()));
    let c = contribution(&mut w, &head, 0);
    close(c.global.get0(Stat::Strength), 150.0 * 6.35);
    close(c.global.get0(Stat::CritDamage), 100.0 * 6.35);
    close(c.global.get0(Stat::CritChance), 30.0 * 1.5);
}

#[test]
fn maxors_boots_speed_full_multiplier() {
    let res = with_items(
        &base_resource(),
        vec![json!({"id":"SPEED_WITHER_BOOTS","name":"Maxor's Boots","category":"BOOTS","tier":"LEGENDARY","dungeon_item":true,"stats":{"WALK_SPEED":30}})],
    );
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let c = contribution(&mut w, &new_it(&res, "SPEED_WITHER_BOOTS"), 0);
    close(c.global.get0(Stat::Speed), 30.0 * (4.85 + 1.5));
}

#[test]
fn floor_scaled_armor() {
    let res = with_items(
        &base_resource(),
        vec![json!({"id":"SKELETON_MASTER_CHESTPLATE","name":"Skeleton Master Chestplate","category":"CHESTPLATE","tier":"EPIC","dungeon_item":true,
            "tiered_stats":{"CRITICAL_CHANCE":[2,2,2,2,3,3,3,3,4,4],"CRITICAL_DAMAGE":[22,23,25,27,30,32,35,38,41,45]}})],
    );
    let chest = with(new_it(&res, "SKELETON_MASTER_CHESTPLATE"), |i| {
        i.recomb = true;
        i.reforge = Some("ancient".into());
        i.master_stars = 5;
    });
    assert_eq!(chest.base_rarity, "LEGENDARY");
    // secret digit 5 = a General's Medallion with 5-digit secrets.
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |i| i.secrets = 12345.0), weights(Class::Arch));
    close(contribution(&mut w, &chest, 1).global.get0(Stat::CritChance), 21.0 * 1.8);
    let mut low = World::new(mk_ctx(Class::Arch, 24.0, &res, |i| i.secrets = 12345.0), weights(Class::Arch));
    close(contribution(&mut low, &with(chest.clone(), |i| i.reforge = None), 1).global.get0(Stat::CritChance), 4.5 * 1.8);
}

#[test]
fn normal_mode_and_master_skull() {
    let res = res();
    let nm = mk_ctx(Class::Mage, 50.0, &res, |i| i.floor_mode = FloorMode::Normal);
    assert_eq!(nm.floor_mode, FloorMode::Normal);
    assert_eq!(mk_ctx(Class::Arch, 50.0, &res, |i| i.floor_mode = FloorMode::Normal).floor_mode, FloorMode::Master);
    assert_eq!(mk_ctx(Class::Mage, 50.0, &res, |i| {
        i.floor_mode = FloorMode::Normal;
        i.mode = Mode::Rcm;
    })
    .floor_mode, FloorMode::Normal);
    let wt = weights_for(Class::Mage, None, Mode::Lcm, FloorMode::Normal);
    assert_eq!((wt.wither, wt.dragon), (100.0, 0.0));
    let starred = with(new_it(&res, "HYPERION"), |i| i.master_stars = 5);
    let plain = new_it(&res, "HYPERION");
    let mut wn = World::new(nm, wt);
    let mut wm = World::new(mk_ctx(Class::Mage, 50.0, &res, |_| {}), weights(Class::Mage));
    close(contribution(&mut wn, &starred, 0).global.get0(Stat::Strength), contribution(&mut wn, &plain, 0).global.get0(Stat::Strength));
    assert!(contribution(&mut wm, &starred, 0).global.get0(Stat::Strength) > contribution(&mut wm, &plain, 0).global.get0(Stat::Strength));
    let skull = new_it(&res, "MASTER_SKULL_TIER_7");
    assert_eq!(contribution(&mut wn, &skull, 0).global.get(Stat::MultStrength), None);
    close(contribution(&mut wm, &skull, 0).global.get0(Stat::MultStrength), 1.1);
}

#[test]
fn hyperion_per_cata_level() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Mage, 50.0, &res, |_| {}), weights(Class::Mage));
    let hyp = contribution(&mut w, &new_it(&res, "HYPERION"), 0).global;
    close(hyp.get0(Stat::Intelligence), (350.0 + 100.0) * 6.35);
    close(hyp.get0(Stat::Damage), (260.0 + 50.0) * 6.35);
}

#[test]
fn dungeon_accessory_enrichments() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let acc = |w: &mut World, it: &ItemState| {
        let id = w.intern(it);
        *w.model.accessory_item_stats(id, 0)
    };
    let skull = with(new_it(&res, "MASTER_SKULL_TIER_7"), |i| i.enrichment = Some("critical_damage".into()));
    close(acc(&mut w, &skull).get0(Stat::CritDamage), 1.0 * (1.0 + 4.85));
    close(acc(&mut w, &with(skull.clone(), |i| i.enrichment = Some("critical_chance".into()))).get0(Stat::CritChance), 1.0);
    let relic = with(new_it(&res, "WITHER_RELIC"), |i| i.enrichment = Some("critical_damage".into()));
    close(acc(&mut w, &relic).get0(Stat::CritDamage), 1.0);
}

#[test]
fn class_bonuses() {
    let res = res();
    let arch = mk_ctx(Class::Arch, 50.0, &res, |i| {
        i.archer_level = 50.0;
        i.combat_level = 60.0;
    });
    close(archer_multiplier_of(&arch), 1.8);
    let bers = mk_ctx(Class::Bers, 50.0, &res, |i| {
        i.archer_level = 50.0;
        i.combat_level = 60.0;
        i.berserk_level = 50.0;
    });
    close(archer_multiplier_of(&bers), 1.0);
    close(arch.class_additive_damage(), 210.0);
    // Berserk class bonus (115) plus Lust for Blood (1200 at Berserk 50 solo).
    close(bers.class_additive_damage(), 210.0 + 115.0 + 1200.0);
    close(mk_ctx(Class::Mage, 50.0, &res, |i| i.combat_level = 60.0).class_additive_damage(), 210.0);
}

fn archer_multiplier_of(c: &Ctx) -> f64 {
    dps_calc::eval::archer_multiplier(c)
}

#[test]
fn ability_damage_star_part_only() {
    let res = with_items(
        &base_resource(),
        vec![json!({"id":"WITHER_GOGGLES","name":"Wither Goggles","category":"HELMET","tier":"EPIC","dungeon_item":true,"stats":{"ABILITY_DAMAGE_PERCENT":45}})],
    );
    let mut w = World::new(mk_ctx(Class::Mage, 50.0, &res, |i| i.secrets = 12345.0), weights(Class::Mage));
    let it = with(new_it(&res, "WITHER_GOGGLES"), |i| i.master_stars = 5);
    close(contribution(&mut w, &it, 1).global.get0(Stat::AbilityDamage), 45.0 * 1.8);
}

#[test]
fn terminator_attack_speed_star_boost() {
    let res = with_items(
        &base_resource(),
        vec![json!({"id":"TERMINATOR","name":"Terminator","category":"BOW","tier":"LEGENDARY","dungeon_item_conversion_cost":{"essence_type":"DRAGON","amount":300},"stats":{"DAMAGE":310,"STRENGTH":50,"CRITICAL_DAMAGE":250,"ATTACK_SPEED":40}})],
    );
    let mut w = World::new(mk_ctx(Class::Bers, 50.0, &res, |_| {}), weights(Class::Bers));
    close(contribution(&mut w, &new_it(&res, "TERMINATOR"), 0).global.get0(Stat::AttackSpeed), 60.0);
}

// ---------------------------------------------------------------------------------------------------------------
// Accessories, shards, tuning, small formulas
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn accessory_family_supersedes_talisman() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let items = vec![new_it(&res, "SPEED_TALISMAN"), new_it(&res, "SPEED_RING")];
    let l = w.build(&items, &[], &[], &[]);
    assert_eq!(w.model.total_accessory_power(l.accessories), 5.0);
}

#[test]
fn dungeon_accessory_power_and_master_skull() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let l = w.build(&[new_it(&res, "MASTER_SKULL_TIER_7")], &[], &[], &[]);
    assert_eq!(w.model.total_accessory_power(l.accessories), 32.0);
    let silky = POWERS.iter().position(|p| p.key == "silky").unwrap() as u8;
    let acc = w.model.accessory_stats(&w.ctx, l.accessories, PowerField::Known(silky), 0.0, 0);
    assert_eq!(acc.stats.get0(Stat::MultStrength), 1.1);
}

#[test]
fn hatcessories_and_families() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let hat = |id: &str, recomb: bool| ItemState {
        id: id.into(),
        name: id.into(),
        category: Some("ACCESSORY".into()),
        base_rarity: "SPECIAL".into(),
        recomb,
        ..Default::default()
    };
    let acc = |id: &str, rarity: &str| ItemState { id: id.into(), category: Some("ACCESSORY".into()), base_rarity: rarity.into(), ..Default::default() };
    let ids: Vec<ItemId> = [
        hat("PARTY_HAT_CRAB", false),
        hat("PARTY_HAT_SLOTH", false),
        hat("PARTY_HAT_CRAB_ANIMATED", true),
        acc("WITHER_RELIC", "LEGENDARY"),
    ]
    .iter()
    .map(|i| w.intern(i))
    .collect();
    let bag = w.model.intern_bag(&ids);
    let mut counted: Vec<String> = w.model.counted_accessories(bag).iter().map(|&i| w.model.item(i).id.clone()).collect();
    counted.sort();
    assert_eq!(counted, ["PARTY_HAT_CRAB_ANIMATED", "WITHER_RELIC"]);

    let ids: Vec<ItemId> = [
        acc("ABICASE_BLUE_RED", "RARE"),
        acc("ABICASE_BLUE_BLUE", "RARE"),
        acc("PESTHUNTER_RELIC", "LEGENDARY"),
        acc("PESTHUNTER_ARTIFACT", "EPIC"),
    ]
    .iter()
    .map(|i| w.intern(i))
    .collect();
    let bag = w.model.intern_bag(&ids);
    let counted: Vec<String> = w.model.counted_accessories(bag).iter().map(|&i| w.model.item(i).id.clone()).collect();
    assert_eq!(counted.iter().filter(|id| id.starts_with("ABICASE")).count(), 1);
    assert_eq!(counted.iter().filter(|id| id.starts_with("PESTHUNTER")).collect::<Vec<_>>(), ["PESTHUNTER_RELIC"]);

    assert!(accessory_superseded("PESTHUNTER_ARTIFACT", ["PESTHUNTER_RELIC"], None));
    assert!(accessory_superseded("PESTHUNTER_BADGE", ["PESTHUNTER_RELIC"], None));
    assert!(!accessory_superseded("PESTHUNTER_RELIC", ["PESTHUNTER_ARTIFACT"], None));
}

fn shard_global(w: &World, shards: &[(&str, f64)], cls: Class) -> Loose {
    let _ = w;
    Levels::new(LevelKind::Shards, shards.iter().map(|&(k, v)| (k.to_string(), v)).collect()).shard_stats(cls).global
}

#[test]
fn shard_echoes() {
    let res = res();
    let w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    close(shard_global(&w, &[("JORMUNG", 10.0)], Class::Arch).get0(Stat::AdditiveStrength), 0.01);
    close(shard_global(&w, &[("JORMUNG", 10.0), ("MOLTHORN", 10.0)], Class::Arch).get0(Stat::AdditiveStrength), 0.015);
    close(
        shard_global(&w, &[("JORMUNG", 10.0), ("MOLTHORN", 10.0), ("TIAMAT", 10.0)], Class::Arch).get0(Stat::AdditiveStrength),
        0.01 * (1.0 + 0.5 * 1.5),
    );
    assert_eq!(shard_global(&w, &[("SKELETOR", 10.0)], Class::Bers).get(Stat::AdditiveDamage), None);
}

fn mk_alloc(cls: Class, split: &[(Stat, f64)], speed_cap_bonus: f64, points: f64, speed: f64, attack_speed: f64, as_mult: f64) -> TuningAllocation {
    let res = res();
    let mut ctx = mk_ctx(cls, 50.0, &res, |_| {});
    ctx.set_tuning_split(Some(split.to_vec()));
    tuning_allocation(points, speed, &ctx, speed_cap_bonus, attack_speed, as_mult)
}

#[test]
fn tuning_points_fill_speed_then_crit() {
    let split = [(Stat::CritChance, 1.0)];
    let a = mk_alloc(Class::Arch, &split, 0.0, 200.0, 580.0, 100.0, 1.0);
    assert_eq!(a.speed_points, 80.0);
    assert_eq!(a.speed, 120.0);
    assert!((a.dps[..a.dps_len].iter().find(|(k, _)| *k == Stat::CritChance).unwrap().1 - 24.0).abs() < 1e-9);
    let b = mk_alloc(Class::Arch, &split, 0.0, 50.0, 300.0, 100.0, 1.0);
    assert_eq!(b.speed_points, 50.0);
    assert_eq!(b.dps_len, 0);
    assert_eq!(mk_alloc(Class::Arch, &split, 0.0, 100.0, 720.0, 100.0, 1.0).speed_points, 0.0);
    // attack speed first
    let c = mk_alloc(Class::Arch, &split, 0.0, 300.0, 640.0, 70.0, 1.0);
    assert_eq!((c.attack_speed_points, c.speed_points, c.dps_points), (100.0, 40.0, 160.0));
}

#[test]
fn dungeon_speed_counts_two_thirds_over_100() {
    use dps_core::config::constants::{dungeon_speed, sheet_speed_for};
    assert_eq!(dungeon_speed(80.0), 80.0);
    assert!((dungeon_speed(200.0) - 166.67).abs() < 0.01);
    assert!((dungeon_speed(300.0) - 233.33).abs() < 0.01);
    close(dungeon_speed(700.0), 500.0);
    close(sheet_speed_for(500.0), 700.0);
    close(sheet_speed_for(450.0), 625.0);
}

#[test]
fn ender_dragon_superior_and_black_cat_tuning() {
    let split = [(Stat::CritChance, 1.0)];
    assert_eq!(mk_alloc(Class::Arch, &split, 0.0, 300.0, 500.0, 73.0 * 1.1, 1.1).attack_speed_points, 60.0);
    assert_eq!(mk_alloc(Class::Arch, &split, 0.0, 300.0, 500.0, 73.0, 1.0).attack_speed_points, 90.0);
    // Black Cat lifts the speed target from 500 to 600 (850 on the sheet).
    assert_eq!(mk_alloc(Class::Arch, &[(Stat::CritDamage, 1.0)], 100.0, 200.0, 750.0, 100.0, 1.0).speed_points, 67.0);
}

#[test]
fn mage_rcm_tuning() {
    let res = res();
    let mut ctx = mk_ctx(Class::Mage, 50.0, &res, |i| {
        i.mage_level = 50.0;
        i.mode = Mode::Rcm;
    });
    assert!(ctx.is_rcm());
    ctx.set_tuning_split(None);
    let t = tuning_allocation(100.0, 620.0, &ctx, 0.0, 100.0, 1.0);
    assert_eq!(t.speed_points, 54.0);
    close(t.dps[..t.dps_len].iter().find(|(k, _)| *k == Stat::Intelligence).unwrap().1, 92.0);
    let wt = weights_for(Class::Mage, None, Mode::Rcm, FloorMode::Master);
    assert_eq!((wt.wither, wt.dragon), (100.0, 0.0));
}

#[test]
fn terminator_volleys_and_arrows() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |i| i.archer_level = 50.0), weights(Class::Arch));
    let term = w.intern(&ItemState { id: "TERMINATOR".into(), category: Some("BOW".into()), base_rarity: "LEGENDARY".into(), ..Default::default() });
    let ctx = w.ctx.clone();
    let v = |a: f64| dps_calc::eval::volleys_per_second(&w.model, Some(term), a, &ctx);
    assert_eq!(v(100.0), 4.0);
    assert!((v(99.0) - 20.0 / 6.0).abs() < 1e-9);
    assert_eq!(v(150.0), 4.0);
    assert!((dps_calc::eval::arrows_per_volley(&w.model, Some(term), 0.2, &ctx) - 4.7).abs() < 1e-9);
    let a20 = mk_ctx(Class::Arch, 50.0, &res, |i| i.archer_level = 20.0);
    assert!((dps_calc::eval::arrows_per_volley(&w.model, Some(term), 0.0, &a20) - 4.2).abs() < 1e-9);
    let bers = mk_ctx(Class::Bers, 50.0, &res, |_| {});
    assert_eq!(dps_calc::eval::arrows_per_volley(&w.model, Some(term), 0.0, &bers), 3.0);
    // Mage beam: 20 / round(10 / (1 + AS/100)).
    let mage = mk_ctx(Class::Mage, 50.0, &res, |_| {});
    close(dps_calc::eval::volleys_per_second(&w.model, None, 100.0, &mage), 4.0);
    close(dps_calc::eval::volleys_per_second(&w.model, None, 60.0, &mage), 20.0 / 6.0);
}

#[test]
fn sa_cloak_multiplier_cases() {
    let res = res();
    let mage50 = mk_ctx(Class::Mage, 50.0, &res, |i| i.mage_level = 50.0);
    let bers = mk_ctx(Class::Bers, 50.0, &res, |_| {});
    close(mage50.sa_cloak_multiplier(4.0), 1.0 + 0.1 / 6.0);
    close(bers.sa_cloak_multiplier(1.0), 1.0 + 0.1 / 3.0);
    close(bers.sa_cloak_multiplier(4.0), 1.0 + 0.1 / 12.0);
    close(bers.sa_cloak_multiplier(0.1), 1.1);
}

// ---------------------------------------------------------------------------------------------------------------
// evaluate on built loadouts
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn general_medallion_secret_digits() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |i| i.secrets = 12345.0), weights(Class::Arch));
    let term = new_it(&res, "TERMINATOR");
    let without = w.build(&[term.clone()], &[], &[], &[]);
    let with_m = w.build(&[term, new_it(&res, "GENERAL_MEDALLION")], &[], &[], &[]);
    let a = w.eval(&without).stats[1].damage;
    let b = w.eval(&with_m).stats[1].damage;
    close(b - a, 310.0 * 0.05);
}

#[test]
fn ragnarock_and_chimera() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let term = new_it(&res, "TERMINATOR");
    let rag0 = with(new_it(&res, "RAGNAROCK_AXE"), |i| i.stars = 0);
    let base = w.build(&[term.clone()], &[], &[], &[]);
    let rag = w.build(&[term.clone(), rag0.clone()], &[], &[], &[]);
    let strength = |w: &World, l: &Loadout| w.eval(l).stats[1].strength;
    let a = strength(&w, &base);
    let b = strength(&w, &rag);
    assert!(b > a);
    let chim_rag = with(rag0.clone(), |i| enchants(i, &[("ultimate_chimera", 5)]));
    let gd = pet("GOLDEN_DRAGON", "LEGENDARY", Some("ANTIQUE_REMEDIES"));
    let chim = w.build(&[term.clone(), chim_rag], &[gd.clone()], &[], &[]);
    assert_eq!(w.model.pet(chim.active_pets[0].unwrap()).kind, "GOLDEN_DRAGON");
    assert!(strength(&w, &chim) > b);

    // Cast with the highest-strength owned pet, whatever pet is active.
    let ed = pet("ENDER_DRAGON", "LEGENDARY", None);
    let l = w.build(&[term, with(rag0, |i| enchants(i, &[("ultimate_chimera", 5)]))], &[gd.clone(), ed.clone()], &[], &[]);
    let strength_with = |w: &mut World, pets: &[Pet]| {
        let mut l2 = l.clone();
        let ids: Vec<PetId> = pets.iter().map(|p| w.model.pet_id(p)).collect();
        l2.pets = PetList::new(ids);
        let e = w.model.pet_id(&ed);
        l2.active_pets = [Some(e), Some(e)];
        w.eval(&l2).stats[1].strength
    };
    let both = strength_with(&mut w, &[gd, ed.clone()]);
    let only_ed = strength_with(&mut w, std::slice::from_ref(&ed));
    assert!(both > only_ed);
}

#[test]
fn forbidden_blessing_and_jellyfish() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let term = new_it(&res, "TERMINATOR");
    let base = w.build(&[term.clone()], &[], &[], &[]);
    let blessed = w.build(&[term.clone()], &[], &[], &[("forbidden_blessing", 10.0)]);
    assert!(w.eval(&blessed).stats[1].strength / w.eval(&base).stats[1].strength > 1.0);

    let gd = pet("GOLDEN_DRAGON", "LEGENDARY", None);
    let jelly = pet("JELLYFISH", "LEGENDARY", None);
    let gd_only = w.build(&[term.clone()], &[gd.clone()], &[], &[]);
    let both = w.build(&[term], &[gd, jelly], &[], &[]);
    assert_eq!(w.model.pet(both.active_pets[0].unwrap()).kind, "GOLDEN_DRAGON");
    let a = w.eval(&gd_only).stats[1];
    let b = w.eval(&both).stats[1];
    assert!(b.strength > a.strength && b.crit_chance > a.crit_chance);
}

#[test]
fn skeleton_master_set_bonus() {
    let res = with_items(
        &base_resource(),
        ["HELMET", "CHESTPLATE", "LEGGINGS", "BOOTS"]
            .iter()
            .map(|s| json!({"id": format!("SKELETON_MASTER_{s}"), "name": s, "category": s, "tier": "EPIC", "dungeon_item": true}))
            .collect(),
    );
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let term = new_it(&res, "TERMINATOR");
    let pieces: Vec<ItemState> = ["HELMET", "CHESTPLATE", "LEGGINGS", "BOOTS"].iter().map(|s| new_it(&res, &format!("SKELETON_MASTER_{s}"))).collect();
    let none = w.build(&[term.clone()], &[], &[], &[]);
    let mut all = vec![term];
    all.extend(pieces);
    let full = w.build(&all, &[], &[], &[]);
    assert_eq!(w.eval(&full).stats[1].additive_damage - w.eval(&none).stats[1].additive_damage, 4.0 * 5.0 + 25.0);
}

#[test]
fn weirder_tuba_dragons_only() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let base = w.build(&[new_it(&res, "TERMINATOR")], &[], &[], &[]);
    let mut tuba = base.clone();
    tuba.extras = ExtraSet::default();
    tuba.extras.insert("WEIRDER_TUBA");
    let a = w.eval(&base);
    let b = w.eval(&tuba);
    close(b.wither(), a.wither());
    assert!(b.dragon() > a.dragon());
    close(b.attack_speeds[1] - a.attack_speeds[1], 5.0);
}

#[test]
fn legion_attack_speed_and_players() {
    let res = with_items(
        &base_resource(),
        vec![json!({"id":"TERMINATOR","name":"Terminator","category":"BOW","tier":"LEGENDARY","dungeon_item_conversion_cost":{"essence_type":"DRAGON","amount":300},"stats":{"DAMAGE":310,"STRENGTH":50,"CRITICAL_DAMAGE":250,"ATTACK_SPEED":40}})],
    );
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let legs = with(new_it(&res, "POWER_WITHER_LEGGINGS"), |i| enchants(i, &[("ultimate_legion", 5)]));
    let mut l = w.build(&[new_it(&res, "TERMINATOR"), legs], &[], &[], &[]);
    l.power = PowerField::Known(POWERS.iter().position(|p| p.key == "silky").unwrap() as u8);
    l.active_pets = [None, None];
    close(w.eval(&l).attack_speed, 65.0 * (1.0 + 5.0 * 0.0007 * 4.0));

    // Inferno Demonlord shard: +1 attack speed per level.
    let mut l2 = l.clone();
    l2.shards = Levels::new(LevelKind::Shards, vec![("BURNINGSOUL".into(), 10.0)]);
    let plain = {
        let mut p = l.clone();
        p.armor = SlotMap::new();
        p
    };
    let _ = plain;
    let mut no_legion = l.clone();
    no_legion.armor = SlotMap::new();
    let mut shard = no_legion.clone();
    shard.shards = l2.shards.clone();
    close(w.eval(&shard).attack_speed - w.eval(&no_legion).attack_speed, 10.0);
}

#[test]
fn legion_players_per_target() {
    let res = res();
    let gain = |cls: Class, mode: Mode| -> (f64, f64) {
        let mut w = World::new(mk_ctx(cls, 50.0, &res, |i| i.mode = mode), weights_for(cls, None, mode, FloorMode::Master));
        let weapon = new_it(&res, if cls == Class::Arch { "TERMINATOR" } else { "HYPERION" });
        let legs = new_it(&res, "POWER_WITHER_LEGGINGS");
        let mut plain = w.build(&[weapon, legs.clone()], &[], &[], &[]);
        plain.active_pets = [None, None];
        let mut legion = plain.clone();
        // JS: { ...plain.armor.LEGGINGS, enchants } (a bare item when the class wears no leggings).
        let piece = plain.armor.get(LEGGINGS).map(|i| w.model.item(i).clone()).unwrap_or_default();
        let l5 = w.intern(&with(piece, |i| enchants(i, &[("ultimate_legion", 5)])));
        legion.armor.set(LEGGINGS, Some(l5));
        let a = w.eval(&plain).stats;
        let b = w.eval(&legion).stats;
        (b[0].strength / a[0].strength - 1.0, b[1].strength / a[1].strength - 1.0)
    };
    let arch = gain(Class::Arch, Mode::Lcm);
    close(arch.0, 4.0 * arch.1);
    let mage = gain(Class::Mage, Mode::Lcm);
    close(mage.1, mage.0);
    close(mage.0, arch.0);
}

#[test]
fn duplex_and_combat_crit_chance() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |i| i.archer_level = 50.0), weights(Class::Arch));
    let plain = w.build(&[new_it(&res, "TERMINATOR")], &[], &[], &[]);
    let duplex = w.build(&[with(new_it(&res, "TERMINATOR"), |i| enchants(i, &[("ultimate_reiterate", 5)]))], &[], &[], &[]);
    close(w.eval(&duplex).arrows_per_volley - w.eval(&plain).arrows_per_volley, 0.2);
    close(w.eval(&duplex).arrows_per_volley, 4.7);

    let at = |combat: f64| {
        let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |i| i.combat_level = combat), weights(Class::Arch));
        let l = w.build(&[new_it(&res, "TERMINATOR")], &[], &[], &[]);
        w.eval(&l).stats[0].crit_chance
    };
    // Terminator divides crit chance by 4: +30 flat -> +7.5.
    close(at(60.0) - at(0.0), 30.0 * 0.25);
}

#[test]
fn fabled_and_rcm_enchants() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Mage, 50.0, &res, |_| {}), weights(Class::Mage));
    let plain = with(scrolled(new_it(&res, "HYPERION")), |i| i.reforge = None);
    let base = w.build(&[plain.clone()], &[], &[], &[]);
    let fabled = w.intern(&with(plain, |i| i.reforge = Some("fabled".into())));
    let mut with_fabled = base.clone();
    with_fabled.weapon = Some(fabled);
    assert!(w.eval(&with_fabled).wither() > w.eval(&base).wither());

    // RCM: Smite counts, Sharpness and Fabled do not.
    let rcm_w = weights_for(Class::Mage, None, Mode::Rcm, FloorMode::Master);
    let mut r = World::new(mk_ctx(Class::Mage, 50.0, &res, |i| i.mode = Mode::Rcm), rcm_w);
    let blade = |e: &[(&str, i32)], reforge: &str| with(scrolled(new_it(&res, "HYPERION")), |i| {
        i.reforge = Some(reforge.into());
        enchants(i, e);
    });
    let l = r.build(&[blade(&[], "heroic")], &[], &[], &[]);
    let cast = |r: &mut World, it: ItemState| {
        let id = r.intern(&it);
        let mut l2 = l.clone();
        l2.weapon = Some(id);
        r.eval(&l2).wither()
    };
    let none = cast(&mut r, blade(&[], "heroic"));
    assert!(cast(&mut r, blade(&[("smite", 7)], "heroic")) > none);
    close(cast(&mut r, blade(&[("sharpness", 7)], "heroic")), none);
}

#[test]
fn rcm_enchant_list_and_floor_toggle() {
    let res = with_items(
        &base_resource(),
        vec![json!({"id":"WEAK_SWORD","name":"Weak Sword","category":"SWORD","tier":"COMMON","stats":{"DAMAGE":1}})],
    );
    // floor toggle
    let dmg = |floor: FloorMode, e: &[(&str, i32)]| {
        let mut w = World::new(mk_ctx(Class::Mage, 0.0, &res, |i| i.floor_mode = floor), weights_for(Class::Mage, None, Mode::Lcm, floor));
        let it = with(new_it(&res, "WEAK_SWORD"), |i| enchants(i, e));
        let l = w.build(&[it], &[], &[], &[]);
        w.eval(&l).wither()
    };
    use FloorMode::*;
    assert!(dmg(Normal, &[("giant_killer", 7)]) > dmg(Normal, &[]));
    close(dmg(Master, &[("giant_killer", 7)]), dmg(Master, &[]));
    assert!(dmg(Master, &[("titan_killer", 7)]) > dmg(Master, &[]));
    close(dmg(Normal, &[("titan_killer", 7)]), dmg(Normal, &[]));
    assert!(dmg(Master, &[("triple_strike", 5)]) > dmg(Master, &[]));
    close(dmg(Master, &[("first_strike", 5)]), dmg(Master, &[]));
    assert!(dmg(Normal, &[("first_strike", 5)]) > dmg(Normal, &[]));
    close(dmg(Normal, &[("triple_strike", 5)]), dmg(Normal, &[]));
    close(dmg(Master, &[("thunderlord", 7)]) / dmg(Master, &[]), 1.2);
    assert!(dmg(Master, &[("prosecute", 6)]) > dmg(Master, &[]));

    // RCM
    let res = Arc::new(base_resource());
    let cast = |floor: FloorMode, e: &[(&str, i32)]| {
        let mut w = World::new(
            mk_ctx(Class::Mage, 0.0, &res, |i| {
                i.mode = Mode::Rcm;
                i.floor_mode = floor;
            }),
            weights_for(Class::Mage, None, Mode::Rcm, floor),
        );
        let it = with(scrolled(new_it(&res, "HYPERION")), |i| enchants(i, e));
        let l = w.build(&[it], &[], &[], &[]);
        w.eval(&l).wither()
    };
    for e in [[("giant_killer", 7)], [("prosecute", 6)], [("ultimate_swarm", 5)], [("smite", 7)]] {
        assert!(cast(Master, &e) > cast(Master, &[]), "{e:?}");
    }
    for e in [[("titan_killer", 7)], [("first_strike", 5)], [("thunderlord", 7)], [("sharpness", 7)], [("ender_slayer", 7)]] {
        close(cast(Master, &e), cast(Master, &[]));
    }
    assert!(cast(Normal, &[("giant_killer", 7)]) > cast(Normal, &[]));
    close(cast(Normal, &[("titan_killer", 7)]), cast(Normal, &[]));
}

#[test]
fn ferocity_berserker_wither_only() {
    let res_f = with_items(
        &base_resource(),
        vec![json!({"id":"TERMINATOR","name":"Terminator","category":"BOW","tier":"LEGENDARY","dungeon_item_conversion_cost":{"essence_type":"DRAGON","amount":300},"stats":{"DAMAGE":310,"STRENGTH":50,"CRITICAL_DAMAGE":250,"FEROCITY":20}})],
    );
    let res0 = res();
    let dmg = |cls: Class, r: &Arc<Resource>| {
        let mut w = World::new(
            mk_ctx(cls, 50.0, r, |i| {
                i.berserk_level = 50.0;
                i.archer_level = 50.0;
            }),
            weights(cls),
        );
        let mut l = w.build(&[new_it(r, "TERMINATOR")], &[], &[], &[]);
        l.active_pets = [None, None];
        w.eval(&l)
    };
    let with_f = dmg(Class::Bers, &res_f);
    let without = dmg(Class::Bers, &res0);
    assert!(with_f.wither() > without.wither());
    close(with_f.dragon(), without.dragon());
    let a1 = dmg(Class::Arch, &res_f);
    let a0 = dmg(Class::Arch, &res0);
    close(a1.wither(), a0.wither());
    // Ferocity on a dungeon item: star multiplier only.
    let mut w = World::new(mk_ctx(Class::Bers, 50.0, &res_f, |_| {}), weights(Class::Bers));
    close(contribution(&mut w, &new_it(&res_f, "TERMINATOR"), 0).global.get0(Stat::Ferocity), 20.0 * 1.5);
    let vicious = with(new_it(&res_f, "TERMINATOR"), |i| enchants(i, &[("vicious", 5)]));
    close(contribution(&mut w, &vicious, 0).global.get0(Stat::Ferocity), (20.0 + 5.0) * 1.5);
}

#[test]
fn book_of_progression_multiplier() {
    let res = with_items(
        &base_resource(),
        vec![json!({"id":"BOOK_OF_PROGRESSION","name":"Book of Progression","category":"ACCESSORY","tier":"LEGENDARY"})],
    );
    let mk = |sb: i32, bop: Option<&str>| {
        mk_ctx(Class::Arch, 50.0, &res, |i| {
            i.profile_stats = Some(ProfileStats { sb_level: sb, bop_bonus: bop.map(String::from), harp_songs: vec![], cakes: vec![] });
        })
    };
    let ctx = mk(278, Some("EXTRA_INFLICTION"));
    let mut w = World::new(ctx, weights(Class::Arch));
    let with_book = w.build(&[new_it(&res, "TERMINATOR"), new_it(&res, "BOOK_OF_PROGRESSION")], &[], &[], &[]);
    let without = w.build(&[new_it(&res, "TERMINATOR")], &[], &[], &[]);
    let book = |w: &World, l: &Loadout, c: &Ctx| dps_calc::eval::book_multiplier(&w.model, l, c);
    close(book(&w, &with_book, &mk(278, Some("EXTRA_INFLICTION"))), 1.0278);
    close(book(&w, &with_book, &mk(620, Some("EXTRA_INFLICTION"))), 1.05);
    close(book(&w, &without, &mk(278, Some("EXTRA_INFLICTION"))), 1.0);
    let mut off = with_book.clone();
    off.bop_bonus = Some(Arc::from("STACKED_HEALING"));
    close(book(&w, &off, &mk(278, Some("STACKED_HEALING"))), 1.0);
}

#[test]
fn chimera_on_mage_weapon() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Mage, 50.0, &res, |_| {}), weights(Class::Mage));
    let gd = pet("GOLDEN_DRAGON", "LEGENDARY", None);
    let base = w.build(&[new_it(&res, "HYPERION")], &[gd], &[], &[]);
    let chim_item = with(w.model.item(base.weapon.unwrap()).clone(), |i| enchants(i, &[("ultimate_chimera", 5)]));
    let id = w.intern(&chim_item);
    let mut chim = base.clone();
    chim.weapon = Some(id);
    let a = w.eval(&base);
    let b = w.eval(&chim);
    // Golden Dragon's 50 attack speed x 100% x 1.5 (5 stars, no master stars or medallion).
    close(b.attack_speeds[0] - a.attack_speeds[0], 50.0 * 1.5);
    assert!(b.wither() > a.wither());
}

#[test]
fn dragon_set_weapon_per_target() {
    let res = res();
    let mut w = World::new(
        mk_ctx(Class::Mage, 50.0, &res, |i| {
            i.mage_level = 50.0;
            i.combat_level = 60.0;
        }),
        weights_for(Class::Mage, None, Mode::Lcm, FloorMode::Master),
    );
    let smite = with(new_it(&res, "HYPERION"), |i| enchants(i, &[("smite", 7)]));
    let ender = with(new_it(&res, "HYPERION"), |i| enchants(i, &[("ender_slayer", 7)]));
    let (sm, en) = (w.intern(&smite), w.intern(&ender));
    let mut l = w.build(&[smite], &[], &[], &[]);
    assert_eq!(l.weapon, Some(sm));
    let mainonly = w.eval(&l);
    l.set_dragon_slot(GearSlot::Weapon, Some(en));
    assert_eq!(l.dragon_item(GearSlot::Weapon), Some(en));
    let both = w.eval(&l);
    close(both.wither(), mainonly.wither());
    assert!(both.dragon() > mainonly.dragon());
}

#[test]
fn tier_boosted_ender_dragon_for_mage() {
    let res = res();
    let mut w = World::new(
        mk_ctx(Class::Mage, 50.0, &res, |i| {
            i.mage_level = 50.0;
            i.combat_level = 60.0;
        }),
        weights(Class::Mage),
    );
    let boosted = pet("ENDER_DRAGON", "EPIC", Some("PET_ITEM_TIER_BOOST"));
    let l = w.build(&[new_it(&res, "HYPERION")], &[boosted.clone()], &[], &[]);
    let mut l1 = l.clone();
    let id = w.model.pet_id(&boosted);
    l1.active_pets[1] = Some(id);
    let r = w.eval(&l1);
    if r.attack_speeds[1] < 100.0 {
        assert_ne!(l.active_pets[1], Some(id));
    }
    let legendary = pet("ENDER_DRAGON", "LEGENDARY", None);
    let l2 = w.build(&[new_it(&res, "HYPERION")], &[legendary], &[], &[]);
    assert_eq!(w.model.pet(l2.active_pets[1].unwrap()).kind, "ENDER_DRAGON");
}

#[test]
fn gold_collection_row_counts_pet_item_chimera_and_ragnarock() {
    let res = res();
    let mut w = World::new(
        mk_ctx(Class::Mage, 50.0, &res, |i| {
            i.mage_level = 50.0;
            i.combat_level = 60.0;
            i.pet_profile = PetProfile { bank: Some(1e9), gold_collection: Some(54321.0) };
        }),
        weights(Class::Mage),
    );
    let hype = |list: &[(&str, i32)]| with(new_it(&res, "HYPERION"), |i| enchants(i, list));
    let rag = |list: &[(&str, i32)]| with(new_it(&res, "RAGNAROCK_AXE"), |i| enchants(i, list));
    let row = |w: &mut World, held: Option<&str>, items: &[ItemState]| -> String {
        let l = w.build(items, &[pet("GOLDEN_DRAGON", "LEGENDARY", held)], &[], &[]);
        let (rows, _) = profile_upgrades(&mut w.model, &w.ctx, w.w, &l, None);
        rows.into_iter().map(|r| r.0).find(|s| s.starts_with("Gold collection")).expect("gold row")
    };
    let label = |s: &str| format!("Gold collection to 100,000 (Golden Dragon: +11.1 strength{s})");
    let chim = [("ultimate_chimera", 5)];
    assert_eq!(row(&mut w, None, &[hype(&[])]), label(""));
    assert_eq!(row(&mut w, Some("ANTIQUE_REMEDIES"), &[hype(&[])]), label(", +20 with Antique Remedies"));
    assert_eq!(row(&mut w, None, &[hype(&chim)]), label(", +81.6 with Chimera"));
    assert_eq!(row(&mut w, None, &[hype(&[]), rag(&[])]), label(""));
    assert_eq!(row(&mut w, None, &[hype(&[]), rag(&chim)]), label(", +27.8 with Ragnarock"));
    assert_eq!(
        row(&mut w, Some("ANTIQUE_REMEDIES"), &[hype(&chim), rag(&chim)]),
        label(", +176.8 with Antique Remedies, Chimera and Ragnarock")
    );
}

#[test]
fn f7_dragon_slot_never_sets_attack_speed_tuning() {
    let accs: Vec<Value> = (0..80)
        .map(|i| json!({"id": format!("TEST_ACC_{i}"), "name": format!("Test {i}"), "category": "ACCESSORY", "tier": "LEGENDARY"}))
        .collect();
    let res = with_items(&base_resource(), accs);
    let ctx = mk_ctx(Class::Mage, 50.0, &res, |i| {
        i.mage_level = 50.0;
        i.combat_level = 60.0;
        i.floor_mode = FloorMode::Normal;
        i.pet_profile = PetProfile { bank: Some(50e6), gold_collection: Some(1e9) };
        i.profile_stats = Some(ProfileStats {
            sb_level: 0,
            bop_bonus: None,
            harp_songs: vec![],
            cakes: vec![Cake { stat: "speed".into(), amount: 700.0 }],
        });
    });
    let mut w = World::new(ctx, weights_for(Class::Mage, None, Mode::Lcm, FloorMode::Normal));
    let gd = pet("GOLDEN_DRAGON", "LEGENDARY", None);
    let ed = pet("ENDER_DRAGON", "LEGENDARY", None);
    let hype = with(new_it(&res, "HYPERION"), |i| enchants(i, &[("ultimate_chimera", 5)]));
    let mut items = vec![hype];
    items.extend((0..80).map(|i| new_it(&res, &format!("TEST_ACC_{i}"))));
    let l = w.build(&items, &[gd.clone(), ed], &[], &[]);
    let gd_id = w.model.pet_id(&gd);
    assert_eq!(l.active_pets[0], Some(gd_id));
    assert_eq!(w.eval(&l).tuning.attack_speed, 0.0);
    // An empty dragon slot (no attack speed) changes nothing on F7.
    let mut l2 = l.clone();
    l2.active_pets = [Some(gd_id), None];
    assert_eq!(w.eval(&l2).tuning.attack_speed, 0.0);
}

#[test]
fn fire_aspect_and_first_strike() {
    let res = with_items(
        &base_resource(),
        vec![json!({"id":"BIG_SWORD","name":"Big Sword","category":"SWORD","tier":"LEGENDARY","stats":{"DAMAGE":1e9}})],
    );
    let mut w = World::new(
        mk_ctx(Class::Mage, 50.0, &res, |i| {
            i.mage_level = 50.0;
            i.combat_level = 60.0;
        }),
        weights(Class::Mage),
    );
    let l = w.build(&[new_it(&res, "HYPERION")], &[], &[], &[]);
    let with_weapon = |w: &mut World, it: ItemState| {
        let id = w.intern(&it);
        let mut l2 = l.clone();
        l2.weapon = Some(id);
        w.eval(&l2)
    };
    let hype = |e: &[(&str, i32)]| with(new_it(&res, "HYPERION"), |i| enchants(i, e));
    let plain = with_weapon(&mut w, hype(&[]));
    let fire = with_weapon(&mut w, hype(&[("fire_aspect", 3)]));
    assert!(fire.wither() > plain.wither());
    close(fire.dragon(), plain.dragon());
    assert!(plain.wither() < 1e9);
    close(with_weapon(&mut w, hype(&[("first_strike", 5)])).wither(), plain.wither());
    let big = |e: &[(&str, i32)]| with(new_it(&res, "BIG_SWORD"), |i| enchants(i, e));
    let big_plain = with_weapon(&mut w, big(&[]));
    assert!(big_plain.wither() >= 1e9);
    assert!(with_weapon(&mut w, big(&[("first_strike", 5)])).wither() > big_plain.wither());
}

#[test]
fn best_pet_per_boss_and_unknown_pet() {
    let res = res();
    let pets = [pet("GOLDEN_DRAGON", "LEGENDARY", None), pet("ENDER_DRAGON", "LEGENDARY", None)];
    // Endgame-sized stat pools, where Ender Dragon's Superior (+10% most stats) is worth the most.
    let cake = |stat: &str, amount: f64| Cake { stat: stat.into(), amount };
    let pick = |cls: Class, bank: f64| -> [String; 2] {
        let ctx = mk_ctx(cls, 50.0, &res, |i| {
            i.combat_level = 60.0;
            i.berserk_level = 50.0;
            i.pet_profile = PetProfile { bank: Some(bank), gold_collection: Some(1e9) };
            i.profile_stats = Some(ProfileStats {
                sb_level: 0,
                bop_bonus: None,
                harp_songs: vec![],
                cakes: vec![cake("strength", 1200.0), cake("critDamage", 900.0), cake("attackSpeed", 100.0)],
            });
        });
        let mut w = World::new(ctx, weights(cls));
        let l = w.build(&[new_it(&res, "TERMINATOR")], &pets, &[], &[]);
        l.active_pets.map(|p| w.model.pet(p.unwrap()).kind.clone())
    };
    // Archer: the Golden Dragon's bank perk (+250% additive at 1B) wins rich, Ender Dragon wins broke.
    assert_eq!(pick(Class::Arch, 1e9), ["GOLDEN_DRAGON", "ENDER_DRAGON"]);
    assert_eq!(pick(Class::Arch, 0.0)[0], "ENDER_DRAGON");
    // Berserker: Lust for Blood's 1200 additive waters the bank perk down, so Ender Dragon wins even at 1B.
    assert_eq!(pick(Class::Bers, 1e9)[0], "ENDER_DRAGON");

    // A pet the bot has no data for is ignored instead of crashing.
    let mut w = World::new(mk_ctx(Class::Mage, 50.0, &res, |_| {}), weights_for(Class::Mage, None, Mode::Lcm, FloorMode::Master));
    let l = w.build(&[new_it(&res, "HYPERION")], &[pet("SOME_NEW_PET", "LEGENDARY", None)], &[], &[]);
    assert!(w.eval(&l).wither() > 0.0);
    // No horse pets.
    let horse = pet("SKELETON_HORSE", "LEGENDARY", None);
    let pl = PetList::new(vec![w.model.pet_id(&horse)]);
    assert_eq!(w.ev().dps_pets(&pl).len(), 0);
}

#[test]
fn best_tuning_split_is_chosen_by_the_bot() {
    let res = res();
    let mut w = World::new(
        mk_ctx(Class::Mage, 50.0, &res, |i| {
            i.tuning = Some(Tuning { allocation: Default::default(), split: Some([("critChance".to_string(), 1.0)].into_iter().collect()) });
        }),
        weights(Class::Mage),
    );
    w.build(&[new_it(&res, "HYPERION"), new_it(&res, "WITHER_RELIC")], &[], &[], &[]);
    let split = w.ctx.tuning_split().expect("split");
    assert!((split.iter().map(|(_, v)| v).sum::<f64>() - 1.0).abs() < 1e-9);
    assert!(split.iter().any(|(k, _)| *k == Stat::Intelligence));
}

#[test]
fn clear_boots_and_pets_perks() {
    let res = with_items(
        &base_resource(),
        vec![
            json!({"id":"SPEED_WITHER_BOOTS","name":"Maxor's Boots","category":"BOOTS","tier":"LEGENDARY","dungeon_item":true,"stats":{"WALK_SPEED":30,"CRITICAL_DAMAGE":45}}),
            json!({"id":"POWER_WITHER_BOOTS","name":"Necron's Boots","category":"BOOTS","tier":"LEGENDARY","dungeon_item":true,"stats":{"STRENGTH":40,"CRITICAL_DAMAGE":30}}),
        ],
    );
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let necron_only = w.build(&[new_it(&res, "TERMINATOR"), new_it(&res, "POWER_WITHER_BOOTS")], &[], &[], &[]);
    let both = w.build(&[new_it(&res, "TERMINATOR"), new_it(&res, "POWER_WITHER_BOOTS"), new_it(&res, "SPEED_WITHER_BOOTS")], &[], &[], &[]);
    assert_eq!(w.model.item(both.clear_boots.unwrap()).id, "SPEED_WITHER_BOOTS");
    assert!(w.eval(&both).speed - w.eval(&necron_only).speed >= 0.0);

    // Clear boots button off (noClearBoots): no clear boots, so the boss boots carry the speed.
    for cls in [Class::Arch, Class::Bers] {
        let on = mk_ctx(cls, 50.0, &res, |_| {});
        let off = mk_ctx(cls, 50.0, &res, |i| i.no_clear_boots = true);
        assert!(on.clear_boots_used() && !off.clear_boots_used());
        let mut w = World::new(off, weights(cls));
        let one = w.build(&[new_it(&res, "TERMINATOR"), new_it(&res, "POWER_WITHER_BOOTS"), new_it(&res, "SPEED_WITHER_BOOTS")], &[], &[], &[]);
        assert_eq!(one.clear_boots, None);
    }
    assert!(!mk_ctx(Class::Mage, 50.0, &res, |_| {}).clear_boots_used());

    // Wither Skeleton: a wither-only damage perk.
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let mut l = w.build(&[new_it(&res, "TERMINATOR")], &[], &[], &[]);
    let ws = pet("WITHER_SKELETON", "LEGENDARY", None);
    let id = w.model.pet_id(&ws);
    let none = {
        l.active_pets = [None, None];
        w.eval(&l)
    };
    l.active_pets = [Some(id), Some(id)];
    let with_ws = w.eval(&l);
    assert!(with_ws.wither() / none.wither() > with_ws.dragon() / none.dragon());
    // Blaze doubles potato books on a mage weapon.
    let mut mw = World::new(mk_ctx(Class::Mage, 50.0, &res, |_| {}), weights(Class::Mage));
    let mut ml = mw.build(&[with(new_it(&res, "HYPERION"), |i| i.hpb = 15)], &[], &[], &[]);
    let blaze = pet("BLAZE", "LEGENDARY", None);
    let bid = mw.model.pet_id(&blaze);
    ml.active_pets = [Some(bid), Some(bid)];
    let b = mw.eval(&ml).stats[0];
    ml.active_pets = [None, None];
    let n = mw.eval(&ml).stats[0];
    assert!(b.damage > n.damage);
    // Death's Touch adds damage against dragons.
    let mut dl = mw.build(&[new_it(&res, "HYPERION")], &[], &[], &[]);
    dl.active_pets = [None, None];
    let base = mw.eval(&dl).dragon();
    let wsid = mw.model.pet_id(&ws);
    dl.active_pets = [Some(wsid), Some(wsid)];
    assert!(mw.eval(&dl).dragon() > base);
}

#[test]
fn eval_key_and_loadout_key_properties() {
    let res = res();
    let mut w = World::new(mk_ctx(Class::Arch, 50.0, &res, |_| {}), weights(Class::Arch));
    let items = vec![new_it(&res, "TERMINATOR"), new_it(&res, "SPEED_TALISMAN"), new_it(&res, "WITHER_RELIC"), new_it(&res, "MASTER_SKULL_TIER_7")];
    let l = w.build(&items, &[pet("GOLDEN_DRAGON", "LEGENDARY", None)], &[], &[]);
    // Permuting the bag changes the evaluation key (summation order) but not the loadout key.
    let mut ids: Vec<ItemId> = w.model.bag(l.accessories).items.to_vec();
    ids.reverse();
    let mut l2 = l.clone();
    l2.accessories = w.model.intern_bag(&ids);
    assert_ne!(l.accessories, l2.accessories);
    assert_eq!(LoadoutKey::new(&w.model, &l), LoadoutKey::new(&w.model, &l2));
    assert_ne!(EvalKey::new(&w.model, &l), EvalKey::new(&w.model, &l2));
    // Same loadout twice: equal keys; memo returns the same result.
    let memo = EvalMemo::new(&w.ctx);
    let ev = Evaluator::new(&w.model, &w.ctx, w.w).with_memo(&memo);
    let a = ev.evaluate(&l);
    let b = ev.evaluate(&l.clone());
    assert_eq!(a, b);
    assert_eq!(memo.len(), 1);
    assert_eq!(a, Evaluator::new(&w.model, &w.ctx, w.w).evaluate(&l));
    // Owned pets beyond the best Jellyfish / Ragnarock strength do not change the evaluation key.
    let mut l3 = l.clone();
    let extra = w.model.pet_id(&pet("SQUID", "COMMON", None));
    let mut ids = l.pets.ids.clone();
    ids.push(extra);
    l3.pets = PetList::new(ids);
    assert_eq!(EvalKey::new(&w.model, &l), EvalKey::new(&w.model, &l3));
    assert_ne!(LoadoutKey::new(&w.model, &l), LoadoutKey::new(&w.model, &l3));
}

#[test]
fn skeleton_chest_and_excluded_items() {
    let res = with_items(
        &base_resource(),
        vec![
            json!({"id":"SKELETON_MASTER_CHESTPLATE","name":"SMC","category":"CHESTPLATE","tier":"EPIC","dungeon_item":true}),
            json!({"id":"NECRON_CHEST","name":"NC","category":"CHESTPLATE","tier":"EPIC","dungeon_item":true}),
        ],
    );
    assert!(skeleton_master_chest_rule(Class::Arch, 36.0));
    assert!(!skeleton_master_chest_rule(Class::Arch, 35.0));
    assert!(!skeleton_master_chest_rule(Class::Mage, 50.0));
    let arch = mk_ctx(Class::Arch, 50.0, &res, |_| {});
    assert!(arch.excluded_item("NECRON_CHEST"));
    assert!(!arch.excluded_item("SKELETON_MASTER_CHESTPLATE"));
    let any = mk_ctx(Class::Arch, 50.0, &res, |i| i.any_chest = true);
    assert!(!any.excluded_item("NECRON_CHEST"));
    let rcm = mk_ctx(Class::Mage, 50.0, &res, |i| i.mode = Mode::Rcm);
    assert!(rcm.excluded_item("BALLOON_SNAKE"));
    let lcm = mk_ctx(Class::Mage, 50.0, &res, |_| {});
    assert!(!lcm.excluded_item("BALLOON_SNAKE"));
    assert!(lcm.excluded_item("SPEED_WITHER_BOOTS") && lcm.excluded_item("STARRED_SPEED_WITHER_BOOTS"));
    assert!(lcm.excluded_item("POWER_WITHER_BOOTS"));
    assert!(lcm.has_dragon_set() && !rcm.has_dragon_set());
}

/// test/planner.test.js 'Mage LCM: ... a spare Hyperion is upgraded for only what it lacks': the dual-wield copy lists
/// only what the spare still needs (Prosecute shown as Execute; an Execute already on the spare counts).
#[test]
fn spare_blade_missing_upgrades() {
    use dps_calc::parts::{convert_parts, parts_cost};
    let res = res();
    let lcm = with(scrolled(new_it(&res, "HYPERION")), |it| {
        it.hpb = 15;
        enchants(it, &[("ultimate_chimera", 5), ("titan_killer", 7), ("prosecute", 6), ("smite", 7), ("critical", 6)]);
    });
    let mut prices = dps_core::model::PriceTables::default();
    for id in ["ENCHANTMENT_ULTIMATE_CHIMERA_5", "ENCHANTMENT_TITAN_KILLER_7", "ENCHANTMENT_PROSECUTE_6", "ENCHANTMENT_SMITE_7", "ENCHANTMENT_CRITICAL_6", HOT_POTATO.id, FUMING_POTATO.id]
        .into_iter()
        .chain(WITHER_SCROLLS.iter().copied())
    {
        prices.bazaar.insert(id.to_string(), 1e6);
    }
    let swap = [("prosecute", "execute")];
    let scrolls = ["Implosion Scroll", "Shadow Warp Scroll", "Wither Shield Scroll"];

    let (_, parts, names) = convert_parts(&new_it(&res, "HYPERION"), &lcm, &prices, &swap);
    let mut want = vec!["Chimera V", "Titan Killer VII", "Execute VI", "Smite VII", "Critical VI", "10 Hot Potato Books", "5 Fuming Potato Books"];
    want.extend(scrolls);
    assert_eq!(names, want);
    close(parts_cost(&parts, &prices).unwrap(), 23e6);

    let half = with(new_it(&res, "HYPERION"), |it| {
        it.hpb = 10;
        enchants(it, &[("execute", 6), ("smite", 7), ("titan_killer", 6)]);
    });
    let (_, parts, names) = convert_parts(&half, &lcm, &prices, &swap);
    let mut want = vec!["Chimera V", "Titan Killer VII", "Critical VI", "5 Fuming Potato Books"];
    want.extend(scrolls);
    assert_eq!(names, want);
    close(parts_cost(&parts, &prices).unwrap(), 11e6);
}
