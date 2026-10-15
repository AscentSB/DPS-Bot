// A few evaluation workers that score the planner's candidates in parallel. Each worker builds the same
// candidate list for a state (same code, same data) and scores every Nth one; the results merge back in list
// order, so the plan is identical to scoring in one thread.
import { Worker } from 'node:worker_threads';
import os from 'node:os';

// EVAL_WORKERS sets the pool size (0 turns it off).
export function createEvalPool(size = Number(process.env.EVAL_WORKERS ?? Math.min(4, os.availableParallelism() - 1))) {
  if (size < 2) return null;
  const pending = new Map();
  let nextId = 1;
  // A worker that dies fails its pending jobs (the plan errors instead of hanging) and is replaced.
  const spawn = (slot) => {
    slot.worker = new Worker(new URL('./evalWorker.js', import.meta.url));
    slot.sent = {};
    slot.worker.unref();
    slot.worker.on('message', ({ id, result, error }) => {
      const job = pending.get(id);
      pending.delete(id);
      if (error) job?.reject(new Error(error));
      else job?.resolve(result);
    });
    slot.worker.on('exit', () => {
      for (const [id, job] of pending) if (job.slot === slot) { pending.delete(id); job.reject(new Error('Evaluation worker stopped.')); }
      spawn(slot);
    });
    return slot;
  };
  const slots = Array.from({ length: size }, () => spawn({}));
  const call = (slot, msg) => new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject, slot });
    slot.worker.postMessage({ id, ...msg });
  });
  // Big data (items, price tables) goes to a worker only when it changed since the last plan.
  const sync = (slot, name, value, msg) => {
    if (slot.sent[name] === value) return null;
    slot.sent[name] = value;
    return call(slot, msg);
  };

  return {
    // A scorer for one plan. tables: the plan's frozen price tables (snapshotPrices().tables).
    scorer(ctx, weights, tables) {
      const { resource, reforgeCache, ...plainCtx } = ctx;
      // Checked before every state (a no-op unless a worker is new or the data changed).
      const ready = () => Promise.all(slots.flatMap((slot) => [
        sync(slot, 'resource', resource, { type: 'resource', entries: [...resource] }),
        ...['bazaar', 'auctions'].map((kind) => sync(slot, kind, tables[kind], { type: 'prices', kind, entries: [...tables[kind]] })),
      ]));
      return async ({ loadout, current, banned, count }) => {
        await ready();
        // The plan's reforge picks so far go along, so every worker builds the exact same candidate list.
        const job = { ctx: { ...plainCtx, weights: ctx.weights }, reforges: [...(ctx.reforgeCache ?? [])], weights, loadout, current, banned, parts: size };
        const parts = await Promise.all(slots.map((slot, part) => call(slot, { job: { ...job, part } })));
        // Every worker must have built the same list as the planner, or the indexes would not line up.
        if (parts.some((p) => p.count !== count)) throw new Error('Evaluation worker built a different candidate list.');
        return parts.flatMap((p) => p.scored).sort((a, b) => a.index - b.index);
      };
    },
  };
}
