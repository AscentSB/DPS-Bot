//! Damage model and upgrade planner (port of src/calc). See rust/PORTING_NOTES.md.
//!
//! Part 1: the damage model, `src/calc/stats.js`, and the loadout helpers of `src/calc/upgrades.js` (`makeContext`,
//! `weightsFor`, `hasDragonSet`, `skeletonMasterChestRule`, `excludedItem`, `newItem`, `upgradesInto`, the loadout shape
//! and its edits, `score`, `selectPower`, `bestTuningSplit`).
//!
//! Part 2 (planner A): [`build_loadout`] (`buildLoadout`: the start loadout) and [`CandidateGen::available_upgrades`]
//! (`availableUpgrades`: every upgrade from a loadout, in the JS order, with its [`Edit`] as data). The planner itself
//! (`planUpgrades`, `scoreUpgrades`, armor bundles, `profileUpgrades`, the scoring pool) builds on these.
//!
//! Every number is bit-identical to the JavaScript (max relative difference 0 on 6,810 oracle loadouts, 6,000
//! randomised ones and the planner's `states`), because additions happen per stat in the JS order.
//!
//! # Shape
//!
//! - [`Ctx`] (`makeContext`, built from [`CtxInit`]): the run's profile data plus everything derived once (profile stat
//!   totals, the gear-independent part of every stat bucket, class multipliers, the tuning split plan). All fields are
//!   public; `Ctx` is cheap to clone. `ctx.set_tuning_split(..)` is `ctx.tuning = { ...ctx.tuning, split }`.
//! - [`Model`]: the interning arena for **one item context** (class, Catacombs level, floor mode, mode, collections,
//!   secrets, items resource; `Model::accepts(&ctx)` checks). Items ([`ItemId`]), accessory bags ([`BagId`]), pets
//!   ([`PetId`]) and bank amounts ([`BankId`]) are hash-consed; everything derived from them (item stats per secret-digit
//!   variant, Accessory Power, damage flags, pet stats per bank) is cached. Interning (`intern_item`, `intern_bag`,
//!   `pet_id`, `bank_id`, `modify_item`, `pet_list`, `bag_with`, ...) needs `&mut Model` and belongs to the
//!   single-threaded phases; scoring only reads (`&Model`), lazily computed data sits in `OnceLock`s.
//! - [`Loadout`]: gear as ids, copy-on-write shared accessory bag / shards / perks / pets / owned set. `clone()` is
//!   about 30 ns. Edits mirror the JS helpers: [`Loadout::set_slot`], [`Loadout::set_dragon_slot`],
//!   [`Loadout::replace_item`], [`Loadout::main_item`], [`Loadout::dragon_item`], [`Loadout::slot_name`],
//!   [`Loadout::upgradable_items`], plus small setters (`set_shard`, `set_perk`, `add_extra`, `set_bank`, `set_power`,
//!   `unlock_power`, `set_pets`, ...).
//! - [`Evaluator`]: `Model` + `Ctx` + weights (+ optional [`EvalMemo`]); `Copy + Sync`. `evaluate`, `score`,
//!   `select_pet`, `dps_pets`, `select_power`, `best_tuning_split`.
//! - Free functions: [`evaluate`], [`evaluate_with`], [`active_extras`], [`weighted_gain`], [`tuning_allocation`],
//!   [`weights_for`], [`skeleton_master_chest_rule`], and in [`helpers`] `new_item`, `upgrades_into`, `can_recomb`,
//!   `accessory_superseded`, `is_dungeon`, `has_wither_impact`, reforge helpers.
//! - Item level data on the model: [`Model::raw_stats`] (`itemRawStats`), [`Model::contribution`] (`itemContribution`),
//!   [`Model::accessory_item_stats`], [`Model::accessory_stats`], [`Model::counted_accessories`],
//!   [`Model::total_accessory_power`], [`Model::base_item_stats`], flags in [`ItemFlags`] ([`Model::flags`]).
//!   `variant` is 0 (no General's Medallion) or 1 (medallion: the secret-digit bonus applies; same as 0 when the
//!   secrets have no digits).
//! - Keys: [`EvalKey`] is exactly what `evaluate` reads (owned pets only through the best Jellyfish `potionBoost` and the
//!   Ragnarock pet strength); [`EvalMemo`] is a sharded concurrent memo on it. [`LoadoutKey`] is JS `loadoutKey` (the
//!   bag counts as a set), a superset: use it for planner state caches.
//! - [`SerLoadout`] is the test-data loadout format; [`Loadout::from_serialized`] / [`Loadout::to_serialized`].
//!
//! # Typical use
//!
//! ```ignore
//! let ctx = Ctx::new(CtxInit::from_member(&member, &job, resource).unwrap());
//! let mut model = Model::new(&ctx);
//! let mut loadout = Loadout::empty(&mut model);       // or Loadout::from_serialized(..)
//! // ... build it: model.intern_item(state), loadout.set_slot(..), loadout.accessories = model.intern_bag(&ids)
//! let ev = Evaluator::new(&model, &ctx, weights);
//! let r = ev.evaluate(&loadout);                       // EvalResult { damage, stats, ap, tuning, extras, .. }
//! let shown = r.to_damage();                           // dps_core::model::Damage (what runDps returns)
//! ```
//!
//! Parallel scoring: build the candidate loadouts single-threaded (interning), then
//! `candidates.par_iter().map(|l| ev.evaluate(l))` with one shared `Evaluator` (about 65 ns per evaluate wall time on 16
//! threads, 400-550 ns single thread; the JS needs about 44 us). First evaluation of a new item or bag costs
//! 2-8 us (its stats are derived once); candidate loadouts built by `replace_item` cost 0.4-0.8 us.
//!
//! # Planner part A
//!
//! ```ignore
//! let mut ctx = Ctx::new(CtxInit::from_member(&member, &job, resource)?);
//! let weights = weights_for(ctx.cls, target, ctx.mode, ctx.floor_mode);
//! let mut model = Model::new(&ctx);
//! let start = build_loadout(&mut model, &mut ctx, weights, &member);   // also sets ctx's leftover tuning split
//! let mut gen = CandidateGen::new();                                   // one per plan, shared by every re-plan round
//! let cands = gen.available_upgrades(&mut model, &ctx, weights, &prices, &start);   // single-threaded
//! // score in parallel, with a shared &Model:
//! let ev = Evaluator::new(&model, &ctx, weights);
//! let gains: Vec<f64> = cands.par_iter().map(|c| weighted_gain(&current, &ev.evaluate(&c.edit.apply_scoring(&ev, &start)), &weights)).collect();
//! let next = cands[best].edit.apply(&ev, &start);                      // the step that is taken: full apply
//! ```
//!
//! - A candidate list is built from one loadout, and its edits are valid for that loadout (they remember the
//!   accessory bag they were built for and assert it). Armor buys do not touch the bag, so bundles may apply several
//!   of them one after the other, like the JS `picked.reduce((acc, u) => u.apply(acc), l)`.
//! - [`CandidateGen`] holds the JS `ctx.reforgeCache` (the best reforge per category / rarity / dungeon is decided in
//!   the first state that needs it, with that state's loadout), so call `available_upgrades` for the states of a plan
//!   in the order the JS scores them and keep one generator for the whole plan including re-plan rounds. It also caches
//!   what only depends on the prices (buy templates), so it must not be reused with other prices or another context.
//! - `Edit::apply_scoring` leaves the `owned` set alone (it is a copy of a large set and evaluation only reads the
//!   General's Medallion flag from it); use `Edit::apply` for the loadout that is kept.
//!
//! # Differences from the JS worth knowing
//!
//! - Items are compared by content (`ItemId` equality), the JS compares object identity. Equal items cannot differ in
//!   any number, so only identity-keyed bookkeeping (`upgradableItems`' `Set`) can notice, and two identical items in
//!   one loadout collapse.
//! - Sets (`owned`, `extras`, `unlockedPowers`) keep their insertion order only where the JS reads it: `unlockedPowers`
//!   (`selectPower` ties), `owned` (iteration for serialization). `extras` is a bit set over `EXTRA_UPGRADES`.
//! - A `Loadout` always holds a bank amount ([`BankId`]); the JS `bank ?? ctx.petProfile.bank ?? 0` is resolved when
//!   converting.
//! - A [`PetList`] caches `dpsPets` for the (single) pet profile of the context it is first used with.
//! - An [`EvalMemo`] is valid for one context, tuning split and weights; `Evaluator::with_memo` ignores a memo made for
//!   another tuning split.

pub mod arena;
pub mod build;
pub mod candidates;
pub mod edit;
pub mod ctx;
pub mod eval;
pub mod helpers;
pub mod ids;
pub mod itemstats;
pub mod loadout;
pub mod loose;
pub mod memo;
pub mod parts;
pub mod pick;
pub mod planner;
pub mod profile;
pub mod run;
pub mod serial;

pub use arena::{BagBase, ItemEntry, ItemFlags, Model, Triple};
pub use ctx::{skeleton_master_chest_rule, weights_for, Ctx, CtxInit, ItemCtxKey, UpgradeMode};
pub use eval::{
    active_extras, evaluate, evaluate_with, tuning_allocation, weighted_gain, ActiveExtras, EvalResult, TuningAllocation,
    TuningPoints,
};
pub use ids::{BagId, BankId, GearSlot, ItemId, KindId, PetId, PowerField, Slot, SlotMap};
pub use loadout::{DragonGear, ExtraSet, GearView, LevelKind, Levels, Loadout, Owned, PetList, Rare};
pub use loose::{Bucket, Loose};
pub use memo::{EvalKey, EvalMemo, LoadoutKey};
pub use pick::Evaluator;
pub use serial::SerLoadout;
pub use build::build_loadout;
pub use candidates::{Candidate, CandidateGen};
pub use edit::Edit;
pub use parts::Part;

pub use planner::{plan_upgrades, PlanOutput, PlanStep};
pub use profile::profile_upgrades;
pub use run::{run_dps, runner_for, DpsRunner, PlanError};
