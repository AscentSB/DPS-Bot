import test from 'node:test';
import assert from 'node:assert/strict';
import { execute } from '../src/commands/dps.js';

const fakeInteraction = (userId, replies, ign = '@everyone') => ({
  user: { id: userId },
  options: { getSubcommand: () => 'archer', getString: (name) => (name === 'ign' ? ign : null) },
  reply: async (msg) => replies.push(msg.content),
});

test('/dps has a 10 second per-user cooldown', async () => {
  const replies = [];
  await execute(fakeInteraction('u1', replies));
  await execute(fakeInteraction('u1', replies));
  await execute(fakeInteraction('u2', replies));
  assert.match(replies[0], /not a valid Minecraft username/);
  assert.match(replies[1], /wait 10s/);
  assert.match(replies[2], /not a valid Minecraft username/);
});

test('/dps answers right away while prices are still loading', async () => {
  const replies = [];
  await execute(fakeInteraction('u3', replies, 'p3wr'));
  assert.match(replies[0], /Prices are still loading/);
});

test('/dps sessions go inactive after INACTIVITY_MS: buttons removed, data freed, timers independent, clicks reset', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout', 'Date'] });
  const { testing, handleButton, INACTIVITY_MS } = await import('../src/commands/dps.js');
  const step = { label: 'Step', cost: 1e6, gain: 1, perMillion: 1 };
  const damage = { wither: 1, dragon: 1, tuning: {} };
  const edits = { A: [], B: [] };
  const session = (key, editImpl) => ({
    cls: 'arch', mode: 'lcm', floorMode: 'master', name: 'x', view: 'coins', page: 0, upgradeMode: 'all', owner: 'u',
    coins: { all: { plan: { steps: Array(20).fill(step), startDamage: damage, endDamage: damage }, tuningSplit: null } },
    message: { edit: editImpl ?? (async (e) => edits[key].push(e)) }, usedAt: Date.now(),
  });
  testing.sessions.set('A', session('A'));
  testing.sessions.set('B', session('B'));
  testing.startTimer('A', testing.sessions.get('A'));
  testing.startTimer('B', testing.sessions.get('B'));
  const click = (id, replies = []) => handleButton({ customId: `dps:${id}:next`, user: { id: 'u' }, update: async () => {}, reply: async (r) => replies.push(r.content) });

  t.mock.timers.tick(INACTIVITY_MS - 5000);
  await click('A');
  t.mock.timers.tick(6000);
  await new Promise(setImmediate);
  // B timed out on its own; A's click restarted A's timer.
  assert.equal(testing.sessions.has('B'), false);
  assert.deepEqual(edits.B, [{ content: 'This session is inactive. Run /dps again to start a new one.', components: [] }]);
  assert.equal(testing.sessions.has('A'), true);
  assert.equal(edits.A.length, 0);

  t.mock.timers.tick(INACTIVITY_MS);
  await new Promise(setImmediate);
  assert.equal(testing.sessions.has('A'), false);
  assert.equal(edits.A.length, 1);
  const replies = [];
  await click('A', replies);
  assert.match(replies[0], /session expired/);

  // A deleted message (edit fails) and no usable token: logged, never thrown.
  testing.sessions.set('C', session('C', async () => { throw Object.assign(new Error('Unknown Message'), { code: 10008 }); }));
  testing.startTimer('C', testing.sessions.get('C'));
  t.mock.timers.tick(INACTIVITY_MS);
  await new Promise(setImmediate);
  assert.equal(testing.sessions.has('C'), false);
});
