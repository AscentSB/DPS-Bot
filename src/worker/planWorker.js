// Worker thread: the planner takes seconds to minutes, so it runs here and never blocks the Discord connection.
// It keeps its own item resource and price caches.
import { parentPort } from 'node:worker_threads';
import { getItemsResource } from '../api/hypixel.js';
import { ensurePrices, prices, startPriceRefresh } from '../api/prices.js';
import { runDps } from '../calc/run.js';

startPriceRefresh();

parentPort.on('message', async ({ id, job }) => {
  try {
    const [resource] = await Promise.all([getItemsResource(), ensurePrices()]);
    parentPort.postMessage({ id, result: runDps({ ...job, resource, prices }) });
  } catch (err) {
    parentPort.postMessage({ id, error: err?.message ?? String(err) });
  }
});
