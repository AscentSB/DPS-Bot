//! Conversion between the serialized JS loadout (the test-data format `L`, see rust/PORTING_NOTES.md) and
//! [`Loadout`].

use crate::arena::Model;
use crate::ctx::power_index;
use crate::ids::*;
use crate::loadout::{DragonGear, ExtraSet, LevelKind, Levels, Loadout, Owned, PetList, Rare};
use dps_core::config::accessories::POWERS;
use dps_core::model::{ItemState, Pet};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SerDragonGear {
    #[serde(default)]
    pub weapon: Option<ItemState>,
    #[serde(default)]
    pub armor: IndexMap<String, Option<ItemState>>,
    #[serde(default)]
    pub equipment: IndexMap<String, Option<ItemState>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SerActivePets {
    #[serde(default)]
    pub wither: Option<Pet>,
    #[serde(default)]
    pub dragon: Option<Pet>,
}

/// A loadout as `oracle/serialize.mjs` writes it (JS `JSON.stringify` of the loadout with Maps and Sets as arrays).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerLoadout {
    #[serde(default)]
    pub weapon: Option<ItemState>,
    #[serde(default)]
    pub armor: IndexMap<String, Option<ItemState>>,
    #[serde(default)]
    pub equipment: IndexMap<String, Option<ItemState>>,
    #[serde(default)]
    pub rag: Option<ItemState>,
    #[serde(default)]
    pub clear_boots: Option<ItemState>,
    #[serde(default)]
    pub dragon_gear: Option<SerDragonGear>,
    #[serde(default)]
    pub accessories: Vec<ItemState>,
    #[serde(default)]
    pub owned: Vec<String>,
    #[serde(default)]
    pub extras: Vec<String>,
    #[serde(default)]
    pub shards: IndexMap<String, f64>,
    #[serde(default)]
    pub perks: IndexMap<String, f64>,
    #[serde(default)]
    pub pets: Vec<Pet>,
    #[serde(default)]
    pub active_pets: SerActivePets,
    #[serde(default)]
    pub power: Option<String>,
    #[serde(default)]
    pub bop_bonus: Option<String>,
    /// `null` only on a loadout whose bank was never set (the JS then reads the context's pet profile).
    #[serde(default)]
    pub bank: Option<f64>,
    #[serde(default)]
    pub wither_impact: bool,
    #[serde(default)]
    pub blades: Vec<ItemState>,
    #[serde(default)]
    pub spare_blades: Vec<ItemState>,
    #[serde(default)]
    pub rcm_blade: Option<ItemState>,
    #[serde(default)]
    pub unlocked_powers: Vec<String>,
}

fn slot_map(
    model: &mut Model,
    names: &[&str],
    src: &IndexMap<String, Option<ItemState>>,
) -> Result<SlotMap, String> {
    let mut m = SlotMap::new();
    for (name, item) in src {
        let slot = names.iter().position(|n| n == name).ok_or_else(|| format!("unknown slot {name}"))?;
        let id = item.as_ref().map(|s| model.intern_item(s.clone()));
        m.set(slot, id);
    }
    Ok(m)
}

fn slot_map_back(model: &Model, names: &[&str], m: &SlotMap) -> IndexMap<String, Option<ItemState>> {
    m.entries().map(|(slot, it)| (names[slot].to_string(), it.map(|i| model.item(i).clone()))).collect()
}

fn power_field(name: &Option<String>) -> PowerField {
    match name {
        None => PowerField::Unset,
        Some(n) => match power_index(n) {
            Some(i) => PowerField::Known(i as u8),
            None => PowerField::Unknown,
        },
    }
}

impl Loadout {
    /// Build a loadout from the serialized JS one. `ctx_bank` is `ctx.petProfile.bank`, used when `bank` is null.
    pub fn from_serialized(model: &mut Model, ser: &SerLoadout, ctx_bank: Option<f64>) -> Result<Loadout, String> {
        let weapon = ser.weapon.as_ref().map(|s| model.intern_item(s.clone()));
        let armor = slot_map(model, &ARMOR_NAMES, &ser.armor)?;
        let equipment = slot_map(model, &EQUIPMENT_NAMES, &ser.equipment)?;
        let rag = ser.rag.as_ref().map(|s| model.intern_item(s.clone()));
        let clear_boots = ser.clear_boots.as_ref().map(|s| model.intern_item(s.clone()));
        let dragon_gear = match &ser.dragon_gear {
            None => None,
            Some(d) => Some(DragonGear {
                weapon: d.weapon.as_ref().map(|s| model.intern_item(s.clone())),
                armor: slot_map(model, &ARMOR_NAMES, &d.armor)?,
                equipment: slot_map(model, &EQUIPMENT_NAMES, &d.equipment)?,
            }),
        };
        let acc: Vec<ItemId> = ser.accessories.iter().map(|s| model.intern_item(s.clone())).collect();
        let bag = model.intern_bag(&acc);
        let mut extras = ExtraSet::default();
        for k in &ser.extras {
            extras.insert(k);
        }
        let pet_ids: Vec<PetId> = ser.pets.iter().map(|p| model.pet_id(p)).collect();
        let active_pets = [
            ser.active_pets.wither.as_ref().map(|p| model.pet_id(p)),
            ser.active_pets.dragon.as_ref().map(|p| model.pet_id(p)),
        ];
        let bank = model.bank_id(ser.bank.or(ctx_bank).unwrap_or(0.0));
        let items = |model: &mut Model, v: &[ItemState]| -> Arc<[ItemId]> {
            v.iter().map(|s| model.intern_item(s.clone())).collect::<Vec<_>>().into()
        };
        let blades = items(model, &ser.blades);
        let spare_blades = items(model, &ser.spare_blades);
        let rcm_blade = ser.rcm_blade.as_ref().map(|s| model.intern_item(s.clone()));
        let unlocked: Vec<u8> = ser.unlocked_powers.iter().filter_map(|p| power_index(p)).map(|i| i as u8).collect();
        Ok(Loadout {
            weapon,
            armor,
            equipment,
            rag,
            clear_boots,
            dragon_gear,
            accessories: bag,
            extras,
            active_pets,
            power: power_field(&ser.power),
            bank,
            wither_impact: ser.wither_impact,
            rcm_blade,
            rare: Arc::new(Rare {
                owned: Owned::from_ids(ser.owned.iter().cloned()),
                shards: Levels::new(LevelKind::Shards, ser.shards.iter().map(|(k, v)| (k.clone(), *v)).collect()),
                perks: Levels::new(LevelKind::Perks, ser.perks.iter().map(|(k, v)| (k.clone(), *v)).collect()),
                pets: PetList::new(pet_ids),
                bop_bonus: ser.bop_bonus.as_deref().map(Arc::from),
                blades,
                spare_blades,
                unlocked_powers: unlocked.into(),
            }),
        })
    }

    /// The serialized form (JS key order is lost for sets: extras and powers come out in table order).
    pub fn to_serialized(&self, model: &Model) -> SerLoadout {
        let item = |i: ItemId| model.item(i).clone();
        let items = |v: &[ItemId]| v.iter().map(|&i| item(i)).collect::<Vec<_>>();
        SerLoadout {
            weapon: self.weapon.map(item),
            armor: slot_map_back(model, &ARMOR_NAMES, &self.armor),
            equipment: slot_map_back(model, &EQUIPMENT_NAMES, &self.equipment),
            rag: self.rag.map(item),
            clear_boots: self.clear_boots.map(item),
            dragon_gear: self.dragon_gear.as_ref().map(|d| SerDragonGear {
                weapon: d.weapon.map(item),
                armor: slot_map_back(model, &ARMOR_NAMES, &d.armor),
                equipment: slot_map_back(model, &EQUIPMENT_NAMES, &d.equipment),
            }),
            accessories: items(&model.bag(self.accessories).items),
            owned: self.owned.iter().map(String::from).collect(),
            extras: self.extras.keys().map(String::from).collect(),
            shards: self.shards.entries().iter().cloned().collect(),
            perks: self.perks.entries().iter().cloned().collect(),
            pets: self.pets.ids.iter().map(|&p| model.pet(p).clone()).collect(),
            active_pets: SerActivePets {
                wither: self.active_pets[0].map(|p| model.pet(p).clone()),
                dragon: self.active_pets[1].map(|p| model.pet(p).clone()),
            },
            power: match self.power {
                PowerField::Unset | PowerField::Unknown => None,
                PowerField::Known(i) => Some(POWERS[i as usize].key.to_string()),
            },
            bop_bonus: self.bop_bonus.as_deref().map(String::from),
            bank: Some(model.bank_value(self.bank)),
            wither_impact: self.wither_impact,
            blades: items(&self.blades),
            spare_blades: items(&self.spare_blades),
            rcm_blade: self.rcm_blade.map(item),
            unlocked_powers: self.unlocked_powers.iter().map(|&i| POWERS[i as usize].key.to_string()).collect(),
        }
    }
}
