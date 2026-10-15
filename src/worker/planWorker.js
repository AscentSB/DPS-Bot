// Plan worker: runs one /dps plan at a time off the main thread, so the bot stays responsive. The main thread sends
// the item resource and price tables with a job whenever they changed (it owns the only price cache), and the
// planner spreads candidate scoring over this worker's evaluation pool.
import { parentPort } from 'node:worker_threads';
import { priceLookup } from '../api/prices.js';
import { runDps } from '../calc/run.js';
import { createEvalPool } from './evalPool.js';

const pool = createEvalPool();
let resource = new Map();
let tables = { bazaar: new Map(), auctions: new Map() };

parentPort.on('message', async ({ id, job, data }) => {
  try {
    if (data.resource) resource = new Map(data.resource);
    // New maps on every change, so a plan's frozen price tables never change under it.
    if (data.bazaar || data.auctions) tables = { bazaar: data.bazaar ? new Map(data.bazaar) : tables.bazaar, auctions: data.auctions ? new Map(data.auctions) : tables.auctions };
    const prices = { ...priceLookup(tables), tables };
    parentPort.postMessage({ id, result: await runDps({ ...job, resource, prices, pool }) });
  } catch (err) {
    parentPort.postMessage({ id, error: err?.message ?? String(err) });
  }
});
