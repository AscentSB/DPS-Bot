//! Small copyable handles into a [`crate::Model`] (the interning arena) and the slot containers loadouts use.

use dps_core::config::constants::{ARMOR_SLOTS, EQUIPMENT_SLOTS};
use std::num::NonZeroU32;

/// An interned item state. Two ids are equal exactly when the item contents are equal (order of enchants and gem
/// slots included, because that order decides float summation order).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(NonZeroU32);

impl ItemId {
    #[inline]
    pub(crate) fn from_index(i: usize) -> ItemId {
        ItemId(NonZeroU32::new(i as u32 + 1).expect("item arena overflow"))
    }
    #[inline]
    pub fn index(self) -> usize {
        self.0.get() as usize - 1
    }
    /// The raw number (stable within one Model), for keys and logs.
    #[inline]
    pub fn raw(self) -> u32 {
        self.0.get()
    }
}

impl std::fmt::Debug for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ItemId({})", self.0.get())
    }
}

/// An interned pet (type, tier, held item).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct PetId(pub(crate) u32);

impl PetId {
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// An interned accessory bag (JS `loadout.accessories` Map: item id -> item, in insertion order).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct BagId(pub(crate) u32);

impl BagId {
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// An interned coin amount in the bank (Golden Dragon's Legendary Treasure).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct BankId(pub(crate) u32);

impl BankId {
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// An interned item id string (`HYPERION`), the key of the accessory bag Map.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct KindId(pub(crate) u32);

// ---------------------------------------------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------------------------------------------

/// The armor slot names in index order (`HELMET`, `CHESTPLATE`, `LEGGINGS`, `BOOTS`).
pub const ARMOR_NAMES: [&str; 4] = ARMOR_SLOTS;
/// The equipment slot names in index order (`NECKLACE`, `CLOAK`, `BELT`, `GLOVES`).
pub const EQUIPMENT_NAMES: [&str; 4] = EQUIPMENT_SLOTS;

pub const HELMET: usize = 0;
pub const CHESTPLATE: usize = 1;
pub const LEGGINGS: usize = 2;
pub const BOOTS: usize = 3;
pub const NECKLACE: usize = 0;
pub const CLOAK: usize = 1;
pub const BELT: usize = 2;
pub const GLOVES: usize = 3;

/// A JS object `{ SLOT: item }` over four possible keys, keeping the JS property order (insertion order). A key can
/// be present with a `null` value. `Copy`, hashable (the order is part of the identity: gear is summed in this order).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct SlotMap {
    /// Slot indexes in insertion order; only the first `len` count.
    order: [u8; 4],
    len: u8,
    items: [Option<ItemId>; 4],
}

impl SlotMap {
    pub const fn new() -> SlotMap {
        SlotMap { order: [0; 4], len: 0, items: [None; 4] }
    }

    #[inline]
    fn present(&self, slot: usize) -> bool {
        self.order[..self.len as usize].contains(&(slot as u8))
    }

    /// `obj[slot]` (`None` for a missing key or a null value).
    #[inline]
    pub fn get(&self, slot: usize) -> Option<ItemId> {
        self.items[slot]
    }

    /// `slot in obj`.
    #[inline]
    pub fn has_key(&self, slot: usize) -> bool {
        self.present(slot)
    }

    /// `obj[slot] = item` (an existing key keeps its position).
    pub fn set(&mut self, slot: usize, item: Option<ItemId>) {
        if !self.present(slot) {
            self.order[self.len as usize] = slot as u8;
            self.len += 1;
        }
        self.items[slot] = item;
    }

    /// `delete obj[slot]`.
    pub fn remove(&mut self, slot: usize) {
        if let Some(p) = self.order[..self.len as usize].iter().position(|&s| s as usize == slot) {
            for i in p..self.len as usize - 1 {
                self.order[i] = self.order[i + 1];
            }
            self.len -= 1;
            self.order[self.len as usize] = 0; // keep Eq / Hash independent of stale bytes
        }
        self.items[slot] = None;
    }

    /// `(slot index, value)` in property order, including null values.
    #[inline]
    pub fn entries(&self) -> impl Iterator<Item = (usize, Option<ItemId>)> + '_ {
        self.order[..self.len as usize].iter().map(move |&s| (s as usize, self.items[s as usize]))
    }

    /// `Object.values(obj).filter(Boolean)` in property order.
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = ItemId> + '_ {
        self.order[..self.len as usize].iter().filter_map(move |&s| self.items[s as usize])
    }

    /// `Object.keys(obj).length`.
    pub fn key_count(&self) -> usize {
        self.len as usize
    }

    /// `{ ...self, ...other }`: keys of `other` replace in place or append.
    pub fn merged(&self, other: &SlotMap) -> SlotMap {
        let mut out = *self;
        for (slot, item) in other.entries() {
            out.set(slot, item);
        }
        out
    }

    /// Map every value (`Object.fromEntries(entries.map(([k, v]) => [k, f(v)]))`).
    pub fn map_values(&self, mut f: impl FnMut(Option<ItemId>) -> Option<ItemId>) -> SlotMap {
        let mut out = *self;
        for i in 0..self.len as usize {
            let s = self.order[i] as usize;
            out.items[s] = f(self.items[s]);
        }
        out
    }
}

/// A gear slot of the main set or the dragon set: JS slot names `weapon`, armor and equipment slots.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum GearSlot {
    Weapon,
    Armor(u8),
    Equipment(u8),
}

impl GearSlot {
    /// Parse a JS slot name (`weapon`, `HELMET`, `BELT`, ...). `BRACELET` is not a slot (it goes in `GLOVES`).
    pub fn from_name(name: &str) -> Option<GearSlot> {
        if name == "weapon" {
            return Some(GearSlot::Weapon);
        }
        if let Some(i) = ARMOR_NAMES.iter().position(|&n| n == name) {
            return Some(GearSlot::Armor(i as u8));
        }
        EQUIPMENT_NAMES.iter().position(|&n| n == name).map(|i| GearSlot::Equipment(i as u8))
    }

    pub fn name(self) -> &'static str {
        match self {
            GearSlot::Weapon => "weapon",
            GearSlot::Armor(i) => ARMOR_NAMES[i as usize],
            GearSlot::Equipment(i) => EQUIPMENT_NAMES[i as usize],
        }
    }

    pub fn is_armor(self) -> bool {
        matches!(self, GearSlot::Armor(_))
    }
}

/// Where `setSlot` can put an item: a gear slot or the Ragnarock ability item.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Slot {
    Gear(GearSlot),
    Rag,
}

impl Slot {
    pub fn from_name(name: &str) -> Option<Slot> {
        if name == "rag" {
            Some(Slot::Rag)
        } else {
            GearSlot::from_name(name).map(Slot::Gear)
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Slot::Rag => "rag",
            Slot::Gear(g) => g.name(),
        }
    }
}

/// JS `loadout.power`: unset (`null` / `undefined`, the context's power applies), a power in `POWERS`, or a name
/// `POWERS` does not know (JS then falls back to the class default power, not the context's).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PowerField {
    #[default]
    Unset,
    Known(u8),
    Unknown,
}
