// Main-thread side of the plan worker: one worker, jobs answered in order, restarted if it dies.
import { Worker } from 'node:worker_threads';

let worker = null;
let nextId = 1;
const pending = new Map();

function start() {
  worker = new Worker(new URL('./planWorker.js', import.meta.url));
  worker.on('message', ({ id, result, error }) => {
    const job = pending.get(id);
    if (!job) return;
    pending.delete(id);
    if (error) job.reject(new Error(error));
    else job.resolve(result);
  });
  worker.on('error', (err) => console.error('Plan worker error:', err));
  worker.on('exit', (code) => {
    for (const job of pending.values()) job.reject(new Error(`Plan worker stopped (code ${code}).`));
    pending.clear();
    worker = null;
  });
}

// Start the worker early so its price cache warms up before the first command.
export function startWorker() {
  if (!worker) start();
}

export function runInWorker(job) {
  if (!worker) start();
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    worker.postMessage({ id, job });
  });
}
