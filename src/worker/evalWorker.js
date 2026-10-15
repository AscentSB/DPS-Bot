// Evaluation worker: scores one share of a planner state's candidates (see src/worker/evalPool.js).
import { parentPort } from 'node:worker_threads';
import { availableUpgrades, scoreUpgrades } from '../calc/upgrades.js';
import { priceLookup } from '../api/prices.js';

let resource = new Map();
const tables = { bazaar: new Map(), auctions: new Map() };
const prices = priceLookup(tables);

parentPort.on('message', ({ id, type, entries, kind, job }) => {
  try {
    if (type === 'resource') resource = new Map(entries);
    else if (type === 'prices') tables[kind] = new Map(entries);
    else {
      const ctx = { ...job.ctx, resource, reforgeCache: new Map(job.reforges) };
      const ups = availableUpgrades(job.loadout, ctx, prices);
      return parentPort.postMessage({ id, result: { count: ups.length, scored: scoreUpgrades(ups, job.loadout, job.current, ctx, job.weights, job.banned, job.part, job.parts) } });
    }
    parentPort.postMessage({ id });
  } catch (err) {
    parentPort.postMessage({ id, error: err?.message ?? String(err) });
  }
});
