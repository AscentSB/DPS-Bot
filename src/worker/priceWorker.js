// Price worker: downloads and decodes bazaar and auction prices so the planner thread never does it.
import { parentPort } from 'node:worker_threads';
import { fetchBazaar, fetchAuctions } from '../api/priceFetch.js';

const FETCH = { bazaar: fetchBazaar, auctions: fetchAuctions };

parentPort.on('message', async ({ id, kind }) => {
  try {
    parentPort.postMessage({ id, entries: await FETCH[kind]() });
  } catch (err) {
    parentPort.postMessage({ id, error: err?.message ?? String(err) });
  }
});
