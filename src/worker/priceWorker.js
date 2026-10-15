// Price worker: downloads and decodes bazaar and auction prices so the planner thread never does it.
// Replies with [id, price] entries, or null when the auction house has not changed since the last download.
import { parentPort } from 'node:worker_threads';
import { fetchBazaar, fetchAuctions, smoothLowest } from '../api/priceFetch.js';
import { AUCTION_SMOOTH_DOWNLOADS } from '../config/constants.js';

let lastUpdated = null;
const history = [];

async function auctions() {
  const result = await fetchAuctions(lastUpdated);
  if (!result) return null;
  lastUpdated = result.lastUpdated;
  history.push(new Map(result.entries));
  if (history.length > AUCTION_SMOOTH_DOWNLOADS) history.shift();
  return smoothLowest(history);
}

const FETCH = { bazaar: fetchBazaar, auctions };

parentPort.on('message', async ({ id, kind }) => {
  try {
    parentPort.postMessage({ id, entries: await FETCH[kind]() });
  } catch (err) {
    parentPort.postMessage({ id, error: err?.message ?? String(err) });
  }
});
