// Plan worker: the planner takes seconds to minutes, so it runs here and never blocks the Discord connection.
// It keeps its own item resource and price caches, and spreads candidate scoring over an evaluation pool.
import { parentPort } from 'node:worker_threads';
import { getItemsResource } from '../api/hypixel.js';
import { ensurePrices, snapshotPrices, startPriceRefresh } from '../api/prices.js';
import { runDps } from '../calc/run.js';
import { createEvalPool } from './evalPool.js';

startPriceRefresh();
const pool = createEvalPool();

parentPort.on('message', async ({ id, job }) => {
  try {
    const [resource] = await Promise.all([getItemsResource(), ensurePrices()]);
    parentPort.postMessage({ id, result: await runDps({ ...job, resource, prices: snapshotPrices(), pool }) });
  } catch (err) {
    parentPort.postMessage({ id, error: err?.message ?? String(err) });
  }
});
