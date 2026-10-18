//! `runDps` (src/calc/run.js): one /dps run from a read profile, plus a reusable per-resource context.

use crate::arena::BaseCache;
use crate::planner::plan_upgrades;
use crate::profile::profile_upgrades;
use crate::{build_loadout, weights_for, Ctx, CtxInit, Model};
use dps_core::kinds::{Class, FloorMode, Target};
use dps_core::model::{DpsJob, DpsResult, Member, Plan, ProfileMenu, ProfileRow, Resource, SharedPrices, Step};
use rustc_hash::FxHashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// Why a run did not produce a result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    /// The cancel flag was set (the JS `Plan cancelled.` error).
    Cancelled,
    /// A job field the planner does not know.
    BadJob(String),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::Cancelled => write!(f, "Plan cancelled."),
            PlanError::BadJob(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for PlanError {}

type BaseKey = (Class, u64, FloorMode);

/// Everything that can be reused between runs on one items resource: the base item stats of each (class, Catacombs
/// level, floor mode), which are expensive to derive and do not depend on the player or the prices.
pub struct DpsRunner {
    resource: Arc<Resource>,
    bases: Mutex<FxHashMap<BaseKey, Arc<BaseCache>>>,
}

impl DpsRunner {
    pub fn new(resource: Arc<Resource>) -> DpsRunner {
        DpsRunner { resource, bases: Mutex::new(FxHashMap::default()) }
    }

    pub fn resource(&self) -> &Arc<Resource> {
        &self.resource
    }

    fn base_cache(&self, ctx: &Ctx) -> Arc<BaseCache> {
        let key = (ctx.cls, ctx.cata_level.to_bits(), ctx.floor_mode);
        self.bases.lock().expect("base caches").entry(key).or_default().clone()
    }

    /// JS `runDps({ member, ...job, resource, prices, isCancelled })`. Candidate scoring runs on the rayon pool; set
    /// `cancel` to stop a plan at its next step.
    pub fn run(&self, member: &Member, job: &DpsJob, prices: &SharedPrices, cancel: &AtomicBool) -> Result<DpsResult, PlanError> {
        let init = CtxInit::from_member(member, job, self.resource.clone())
            .ok_or_else(|| PlanError::BadJob(format!("unknown class, mode or floor mode in job {job:?}")))?;
        let mut ctx = Ctx::new(init);
        let target = job.target.as_deref().and_then(Target::from_js);
        let weights = weights_for(ctx.cls, target, ctx.mode, ctx.floor_mode);
        let mut model = Model::with_base_cache(&ctx, self.base_cache(&ctx));
        let loadout = build_loadout(&mut model, &mut ctx, weights, member);
        let floor_mode = ctx.floor_mode.js_name().to_string();
        if job.menu == "profile" {
            let (rows, now) = profile_upgrades(&mut model, &ctx, weights, &loadout, Some(prices));
            return Ok(DpsResult {
                weights,
                menu: job.menu.clone(),
                floor_mode,
                tuning_split: None,
                plan: None,
                profile: Some(ProfileMenu {
                    rows: rows.into_iter().map(|(label, gain)| ProfileRow { label, gain }).collect(),
                    damage: now.to_damage(),
                }),
            });
        }
        let plan = plan_upgrades(&mut model, &ctx, prices, weights, job.limit, &loadout, cancel)?;
        Ok(DpsResult {
            weights,
            menu: job.menu.clone(),
            floor_mode,
            tuning_split: ctx.tuning.split.clone(),
            plan: Some(Plan {
                steps: plan
                    .steps
                    .into_iter()
                    .map(|s| Step {
                        label: s.label,
                        cost: s.cost,
                        source: s.source,
                        unverified: s.unverified,
                        gain: s.gain,
                        per_million: s.per_million,
                        wither_gain: s.wither_gain,
                        dragon_gain: s.dragon_gain,
                    })
                    .collect(),
                start_damage: plan.start_damage.to_damage(),
                end_damage: plan.end_damage.to_damage(),
            }),
            profile: None,
        })
    }
}

static GLOBAL: Mutex<Option<Arc<DpsRunner>>> = Mutex::new(None);

/// The runner for this resource (kept until another resource is passed, so a refreshed resource drops the old caches).
pub fn runner_for(resource: &Arc<Resource>) -> Arc<DpsRunner> {
    let mut g = GLOBAL.lock().expect("runner");
    match &*g {
        Some(r) if Arc::ptr_eq(r.resource(), resource) => r.clone(),
        _ => {
            let r = Arc::new(DpsRunner::new(resource.clone()));
            *g = Some(r.clone());
            r
        }
    }
}

/// JS `runDps`: one /dps run, reusing the per-resource caches of [`runner_for`].
pub fn run_dps(
    member: &Member,
    job: &DpsJob,
    resource: &Arc<Resource>,
    prices: &SharedPrices,
    cancel: &AtomicBool,
) -> Result<DpsResult, PlanError> {
    runner_for(resource).run(member, job, prices, cancel)
}
