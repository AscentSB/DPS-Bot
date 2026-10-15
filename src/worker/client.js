// Main-thread side of the plan workers. Several workers (PLAN_WORKERS, default 2) each run one /dps plan at a time,
// so one user's plan never waits for another's unless every worker is busy (then it queues for the first free one).
// The main thread owns the item resource and price cache and sends them to a worker only when they changed.
// A plan that takes longer than PLAN_TIMEOUT_MS is stopped (its worker is replaced) and fails with PlanTimeoutError.
import { Worker } from 'node:worker_threads';
import { getItemsResource } from '../api/hypixel.js';
import { snapshotPrices } from '../api/prices.js';

const PLAN_WORKERS = Math.max(1, Number(process.env.PLAN_WORKERS ?? 2));
export const PLAN_TIMEOUT_MS = Number(process.env.PLAN_TIMEOUT_MS ?? 3 * 60 * 1000);

export class PlanTimeoutError extends Error {}
// Thrown when a plan is cancelled through its AbortSignal (the /dps session went inactive).
export class PlanCancelledError extends Error {}

const slots = [];
const queue = [];
let nextId = 1;

function spawn(slot) {
  const worker = new Worker(new URL('./planWorker.js', import.meta.url));
  Object.assign(slot, { worker, sent: {}, job: null });
  // Events from a worker that was already replaced (timed out) are ignored.
  worker.on('message', ({ id, result, error }) => {
    const job = slot.job;
    if (slot.worker !== worker || !job || job.id !== id) return;
    finish(slot);
    if (job.signal?.aborted) job.reject(new PlanCancelledError('Plan cancelled.'));
    else if (error) job.reject(new Error(error));
    else job.resolve(result);
  });
  worker.on('error', (err) => console.error('Plan worker error:', err));
  worker.on('exit', (code) => {
    if (slot.worker !== worker) return;
    const job = slot.job;
    replace(slot);
    job?.reject(new Error(`Plan worker stopped (code ${code}).`));
  });
}

// A fresh worker in this slot (the old one is stopped), then the queue moves on.
function replace(slot) {
  clearTimeout(slot.timer);
  const old = slot.worker;
  spawn(slot);
  old.terminate().catch(() => {});
  pump();
}

function finish(slot) {
  clearTimeout(slot.timer);
  slot.job = null;
  pump();
}

// Start the workers at launch so the first command does not wait for them.
export function startWorker() {
  while (slots.length < PLAN_WORKERS) {
    const slot = {};
    spawn(slot);
    slots.push(slot);
  }
}

// Hand queued jobs to idle workers, with the item resource and price tables they need.
async function pump() {
  for (const slot of slots) {
    if (slot.job || !queue.length) continue;
    const job = queue.shift();
    slot.job = job;
    try {
      const resource = await getItemsResource();
      const { tables } = snapshotPrices();
      const data = {};
      if (slot.sent.resource !== resource) data.resource = [...resource];
      for (const kind of ['bazaar', 'auctions']) if (slot.sent[kind] !== tables[kind]) data[kind] = [...tables[kind]];
      slot.sent = { resource, bazaar: tables.bazaar, auctions: tables.auctions };
      slot.timer = setTimeout(() => {
        if (slot.job !== job) return;
        // Stop the stuck plan so later jobs are not stuck behind it.
        replace(slot);
        job.reject(new PlanTimeoutError('Plan took too long.'));
      }, PLAN_TIMEOUT_MS);
      slot.worker.postMessage({ id: job.id, job: job.payload, data, cancel: job.cancel });
    } catch (err) {
      finish(slot);
      job.reject(err);
    }
  }
}

// signal (optional AbortSignal): aborting drops a queued job, or sets the job's shared cancel flag, which the planner
// checks once per step (threads cannot be stopped from outside), and the promise rejects with PlanCancelledError.
export function runInWorker(payload, { signal } = {}) {
  startWorker();
  return new Promise((resolve, reject) => {
    if (signal?.aborted) return reject(new PlanCancelledError('Plan cancelled.'));
    const job = { id: nextId++, payload, resolve, reject, signal, cancel: new SharedArrayBuffer(4) };
    signal?.addEventListener('abort', () => {
      const queued = queue.indexOf(job);
      if (queued >= 0) {
        queue.splice(queued, 1);
        reject(new PlanCancelledError('Plan cancelled.'));
      } else {
        Atomics.store(new Int32Array(job.cancel), 0, 1);
      }
    }, { once: true });
    queue.push(job);
    pump();
  });
}
