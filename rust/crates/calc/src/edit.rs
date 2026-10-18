//! The "apply" closures of `availableUpgrades` as data. Every item an edit creates is interned when the candidate is
//! built (single-threaded, `&mut Model`), so applying an edit only needs `&Model` and can run on any thread.
//!
//! An [`Edit`] is valid for the loadout its candidate list was built from (and, for edits that do not touch the
//! accessory bag, for loadouts that only differ in other places, which is how armor bundles apply several armor buys
//! one after the other). Bag-changing edits remember the bag they were built for and assert it.

use crate::arena::Model;
use crate::ids::*;
use crate::loadout::{ExtraSet, Levels, Loadout, PetList};
use crate::pick::Evaluator;
use dps_core::config::accessories::POWERS;
use std::sync::Arc;

/// What else a [`Edit::Replace`] does besides replacing an item everywhere.
#[derive(Clone, Debug, PartialEq)]
pub enum ReplacePost {
    None,
    /// `next.spareBlades = next.spareBlades.filter(b => b !== spare)`: drops the first copy.
    RemoveSpare(ItemId),
    /// Wither scrolls: `next.blades = blades.map(b => b === blade ? done : b)` and `next.witherImpact = true`.
    Scrolls,
}

/// Where a bought item goes (the closure in `buyInto`).
#[derive(Clone, Debug, PartialEq)]
pub struct BuyEdit {
    pub item: ItemId,
    /// `set === 'dragon'`: the item goes in the dragon set only.
    pub dragon: bool,
    /// `set === 'wither'`: the old main piece moves to the dragon set (`keep_main`).
    pub wither: bool,
    /// The gear slot (`slot` in the JS): weapon, an armor slot or an equipment slot; `None` for Ragnarock and
    /// accessories.
    pub slot: Option<GearSlot>,
    pub is_rag: bool,
    pub is_weapon: bool,
    /// Armor slot index when the item is armor.
    pub armor: Option<u8>,
    /// Equipment slot index when the item is equipment.
    pub equipment: Option<u8>,
    /// `(bag it was built for, bag with the accessory set)` when the item is an accessory.
    pub bag: Option<(BagId, BagId)>,
    /// `mainItem(l, slot)` of the loadout the candidate was built from (the piece kept for dragons).
    pub keep_main: Option<ItemId>,
}

#[derive(Clone)]
pub enum Edit {
    /// `replaceItem(l, old, next)` plus `post`.
    Replace { old: ItemId, next: ItemId, copy: bool, bag: Option<(BagId, BagId)>, post: ReplacePost },
    /// Several accessories replaced in place (Enrichment Swapper): the final bag and the ids to mark owned.
    SetBag { base: BagId, bag: BagId, owned: Vec<ItemId> },
    Buy(BuyEdit),
    /// `next.owned.add(id); next.clearBoots = boots`.
    ClearBoots(ItemId),
    /// `shards: { ...l.shards, [id]: level }`, built when the candidate was.
    Shards(Arc<Levels>),
    /// `perks: { ...l.perks, [key]: level }`.
    Perks(Arc<Levels>),
    /// `bopBonus = BOOK_DAMAGE_OPTION`.
    Bop(Arc<str>),
    /// `bank = min(cap, bank + 1M)`.
    Bank(BankId),
    /// Learn a power (index into `POWERS`) and switch to it.
    LearnPower(u8),
    /// `next.pets.push(pet)` then `selectPet`.
    BuyPet(PetId),
    /// A held item or Kat upgrade on an active pet: the pet in the list and in the active slots becomes `to`.
    SwapPet { from: PetId, to: PetId },
    /// A spare blade becomes the boss weapon (`dragon == false`: the old weapon becomes a spare) or the dragon-set
    /// weapon.
    Spare { spare: ItemId, item: ItemId, dragon: bool, weapon: Option<ItemId> },
    /// `extras.add(key)`, as an index into `EXTRA_UPGRADES`.
    Extra(u8),
}

/// JS `replaceItem(loadout, oldItem, next)` with the accessory bag already worked out.
pub fn replace_item_with_bag(model: &Model, l: &Loadout, old: ItemId, next: ItemId, bag: Option<(BagId, BagId)>) -> Loadout {
    replace_item_impl(model, l, old, next, false, bag, true)
}

fn mark_owned(model: &Model, out: &mut Loadout, item: ItemId, full: bool) {
    if full {
        out.owned.insert(&model.item(item).id);
    } else if model.flags(item).is_medallion {
        out.owned.medallion = true;
    }
}

/// `copy`: the item replaced is the dragon set's byte-identical copy of the main set's piece (a different object in the
/// JS), so only the dragon slots holding such a copy change; otherwise the copies stay as they are.
fn replace_item_impl(model: &Model, l: &Loadout, old: ItemId, next: ItemId, copy: bool, bag: Option<(BagId, BagId)>, full: bool) -> Loadout {
    let mut out = l.clone();
    let swap = |x: Option<ItemId>| if x == Some(old) { Some(next) } else { x };
    if !copy {
        out.weapon = swap(out.weapon);
        out.rag = swap(out.rag);
        out.clear_boots = swap(out.clear_boots);
        out.armor = out.armor.map_values(swap);
        out.equipment = out.equipment.map_values(swap);
    }
    if let Some(d) = &mut out.dragon_gear {
        let is_copy = |main: Option<ItemId>, dragon: Option<ItemId>| dragon == Some(old) && main == Some(old);
        if is_copy(l.weapon, d.weapon) == copy {
            d.weapon = swap(d.weapon);
        }
        for i in 0..4 {
            if d.armor.get(i) == Some(old) && is_copy(l.armor.get(i), d.armor.get(i)) == copy {
                d.armor.set(i, Some(next));
            }
            if d.equipment.get(i) == Some(old) && is_copy(l.equipment.get(i), d.equipment.get(i)) == copy {
                d.equipment.set(i, Some(next));
            }
        }
    }
    if let Some((base, new)) = bag {
        assert_eq!(l.accessories, base, "edit applied to a loadout with another accessory bag than it was built for");
        out.accessories = new;
    }
    // A fragged item is a new id the player now owns, so it is never offered as a buy.
    mark_owned(model, &mut out, next, full);
    out
}

fn first_position(list: &[ItemId], x: ItemId) -> Option<usize> {
    list.iter().position(|&i| i == x)
}

impl Edit {
    /// JS `up.apply(loadout)`. `ev` supplies the model, and the context and weights `buyPet` re-picks the pet with.
    pub fn apply(&self, ev: &Evaluator<'_>, l: &Loadout) -> Loadout {
        self.apply_impl(ev, l, true)
    }

    /// Like [`Edit::apply`] but the `owned` set is not extended (it is a copy of a large set, and scoring never reads
    /// it except for the General's Medallion, whose flag is kept). Use it to score candidates; apply the edit of the
    /// step that is taken with [`Edit::apply`].
    pub fn apply_scoring(&self, ev: &Evaluator<'_>, l: &Loadout) -> Loadout {
        self.apply_impl(ev, l, false)
    }

    fn apply_impl(&self, ev: &Evaluator<'_>, l: &Loadout, full: bool) -> Loadout {
        let model: &Model = ev.model;
        match self {
            Edit::Replace { old, next, copy, bag, post } => {
                let mut out = replace_item_impl(model, l, *old, *next, *copy, *bag, full);
                match post {
                    ReplacePost::None => {}
                    ReplacePost::RemoveSpare(spare) => {
                        if let Some(p) = first_position(&out.spare_blades, *spare) {
                            let mut v = out.spare_blades.to_vec();
                            v.remove(p);
                            out.spare_blades = v.into();
                        }
                    }
                    ReplacePost::Scrolls => {
                        if let Some(p) = first_position(&out.blades, *old) {
                            let mut v = out.blades.to_vec();
                            v[p] = *next;
                            out.blades = v.into();
                        }
                        out.wither_impact = true;
                    }
                }
                out
            }
            Edit::SetBag { base, bag, owned } => {
                assert_eq!(l.accessories, *base, "edit applied to a loadout with another accessory bag than it was built for");
                let mut out = l.clone();
                out.accessories = *bag;
                for &i in owned {
                    mark_owned(model, &mut out, i, full);
                }
                out
            }
            Edit::Buy(b) => apply_buy(model, l, b, full),
            Edit::ClearBoots(boots) => {
                let mut out = l.clone();
                mark_owned(model, &mut out, *boots, full);
                out.clear_boots = Some(*boots);
                out
            }
            Edit::Shards(s) => {
                let mut out = l.clone();
                out.shards = s.clone();
                out
            }
            Edit::Perks(p) => {
                let mut out = l.clone();
                out.perks = p.clone();
                out
            }
            Edit::Bop(opt) => {
                let mut out = l.clone();
                out.bop_bonus = Some(opt.clone());
                out
            }
            Edit::Bank(b) => {
                let mut out = l.clone();
                out.bank = *b;
                out
            }
            Edit::LearnPower(i) => {
                let mut out = l.clone();
                out.unlock_power(POWERS[*i as usize].key);
                out.power = PowerField::Known(*i);
                out
            }
            Edit::BuyPet(pet) => {
                let mut out = l.clone();
                let mut ids = l.pets.ids.clone();
                ids.push(*pet);
                let dps = ev.dps_after_buy(&l.pets, *pet);
                out.pets = PetList::new(ids);
                let _ = out.pets.dps.set(dps);
                ev.select_pet(&out)
            }
            Edit::SwapPet { from, to } => {
                let mut out = l.clone();
                let mut ids = l.pets.ids.clone();
                if let Some(p) = ids.iter().position(|x| x == from) {
                    ids[p] = *to;
                }
                out.pets = PetList::new(ids);
                for a in out.active_pets.iter_mut() {
                    if *a == Some(*from) {
                        *a = Some(*to);
                    }
                }
                out
            }
            Edit::Spare { spare, item, dragon, weapon } => {
                let mut out = l.clone();
                if let Some(p) = first_position(&out.spare_blades, *spare) {
                    let mut v = out.spare_blades.to_vec();
                    v.remove(p);
                    out.spare_blades = v.into();
                }
                if *dragon {
                    out.set_dragon_slot(GearSlot::Weapon, Some(*item));
                } else {
                    let mut v = out.spare_blades.to_vec();
                    if let Some(w) = weapon {
                        v.push(*w);
                    }
                    out.spare_blades = v.into();
                    out.weapon = Some(*item);
                }
                out
            }
            Edit::Extra(i) => {
                let mut out = l.clone();
                let mut e: ExtraSet = out.extras;
                e.insert_index(*i as usize);
                out.extras = e;
                out
            }
        }
    }

    /// True when applying the edit needs the evaluator's context (it re-picks the pet).
    pub fn needs_evaluator(&self) -> bool {
        matches!(self, Edit::BuyPet(_))
    }
}

fn apply_buy(model: &Model, l: &Loadout, b: &BuyEdit, full: bool) -> Loadout {
    let mut next = l.clone();
    mark_owned(model, &mut next, b.item, full);
    if b.dragon {
        next.set_dragon_slot(b.slot.expect("dragon buy has a slot"), Some(b.item));
        return next;
    }
    if b.is_rag {
        next.rag = Some(b.item);
    }
    if b.is_weapon && (!b.is_rag || next.weapon.is_none()) {
        next.weapon = Some(b.item);
    } else if let Some(a) = b.armor {
        next.armor.set(a as usize, Some(b.item));
    } else if let Some(e) = b.equipment {
        next.equipment.set(e as usize, Some(b.item));
    } else if let Some((base, new)) = b.bag {
        assert_eq!(l.accessories, base, "edit applied to a loadout with another accessory bag than it was built for");
        next.accessories = new;
    }
    if b.wither {
        next.set_dragon_slot(b.slot.expect("wither buy has a slot"), b.keep_main);
    }
    next
}

/// JS `depositBank(l)` for a bank amount already interned.
pub fn deposit_bank(l: &Loadout, bank: BankId) -> Loadout {
    let mut out = l.clone();
    out.bank = bank;
    out
}

impl std::fmt::Debug for Edit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Edit::Replace { old, next, post, .. } => write!(f, "Replace({old:?} -> {next:?}, {post:?})"),
            Edit::SetBag { owned, .. } => write!(f, "SetBag({} items)", owned.len()),
            Edit::Buy(b) => write!(f, "Buy({b:?})"),
            Edit::ClearBoots(b) => write!(f, "ClearBoots({b:?})"),
            Edit::Shards(s) => write!(f, "Shards({:?})", s.entries()),
            Edit::Perks(p) => write!(f, "Perks({:?})", p.entries()),
            Edit::Bop(o) => write!(f, "Bop({o})"),
            Edit::Bank(b) => write!(f, "Bank({b:?})"),
            Edit::LearnPower(i) => write!(f, "LearnPower({})", POWERS[*i as usize].key),
            Edit::BuyPet(p) => write!(f, "BuyPet({p:?})"),
            Edit::SwapPet { from, to } => write!(f, "SwapPet({from:?} -> {to:?})"),
            Edit::Spare { spare, item, dragon, .. } => write!(f, "Spare({spare:?} -> {item:?}, dragon {dragon})"),
            Edit::Extra(i) => write!(f, "Extra({i})"),
        }
    }
}
