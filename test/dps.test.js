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

test('the Skeleton Master chest button shows for Archer / Berserker at cata 36+ only and switches plans', async () => {
  const { testing, handleButton } = await import('../src/commands/dps.js');
  const damage = { wither: 1, dragon: 1, tuning: {} };
  const plan = (label) => ({ plan: { steps: [{ label, cost: 1e6, gain: 1, perMillion: 1 }], startDamage: damage, endDamage: damage }, tuningSplit: null });
  const session = (cls, cataLevel) => ({ cls, mode: 'lcm', floorMode: 'master', name: 'x', view: 'coins', page: 0, upgradeMode: 'all', owner: 'u', anyChest: false,
    member: { cataLevel }, coins: { all: plan('forced'), 'all:anyChest': plan('any') }, usedAt: Date.now() });
  const updates = [];
  const click = (id, action) => handleButton({ customId: `dps:${id}:${action}`, user: { id: 'u' }, update: async (m) => updates.push(m) });
  const ids = (m) => m.components.flatMap((r) => r.toJSON().components.map((c) => c.custom_id.split(':')[2]));
  for (const [id, cls, cata, shown] of [['c1', 'arch', 40, true], ['c2', 'bers', 36, true], ['c3', 'mage', 40, false], ['c4', 'arch', 35, false]]) {
    testing.sessions.set(id, session(cls, cata));
    await click(id, 'next');
    assert.equal(ids(updates.at(-1)).includes('chest'), shown, `${cls} c${cata}`);
  }
  await click('c1', 'chest');
  assert.equal(testing.sessions.get('c1').anyChest, true);
  assert.match(updates.at(-1).embeds[0].toJSON().description, /any/);
  for (const id of ['c1', 'c2', 'c3', 'c4']) { clearTimeout(testing.sessions.get(id).timer); testing.sessions.delete(id); }
});

test('the clear boots button shows for Archer / Berserker at any cata, is on by default and switches plans', async () => {
  const { testing, handleButton } = await import('../src/commands/dps.js');
  const damage = { wither: 1, dragon: 1, tuning: {} };
  const plan = (label) => ({ plan: { steps: [{ label, cost: 1e6, gain: 1, perMillion: 1 }], startDamage: damage, endDamage: damage }, tuningSplit: null });
  const session = (cls, cataLevel) => ({ cls, mode: 'lcm', floorMode: 'master', name: 'x', view: 'coins', page: 0, upgradeMode: 'all', owner: 'u', anyChest: false, noClearBoots: false,
    member: { cataLevel }, coins: { all: plan('swap'), 'all:noClearBoots': plan('onepair'), 'all:anyChest:noClearBoots': plan('anyonepair') }, usedAt: Date.now() });
  const updates = [];
  const click = (id, action) => handleButton({ customId: `dps:${id}:${action}`, user: { id: 'u' }, update: async (m) => updates.push(m) });
  const button = (m, action) => m.components.flatMap((r) => r.toJSON().components).find((c) => c.custom_id.split(':')[2] === action);
  for (const [id, cls, cata, shown] of [['b1', 'arch', 40, true], ['b2', 'bers', 20, true], ['b3', 'mage', 40, false]]) {
    testing.sessions.set(id, session(cls, cata));
    await click(id, 'next');
    assert.equal(Boolean(button(updates.at(-1), 'clearboots')), shown, `${cls} c${cata}`);
  }
  assert.equal(button(updates.at(-3), 'clearboots').label, 'Clear boots: on');
  await click('b1', 'clearboots');
  assert.equal(testing.sessions.get('b1').noClearBoots, true);
  assert.equal(button(updates.at(-1), 'clearboots').label, 'Clear boots: off');
  assert.match(updates.at(-1).embeds[0].toJSON().description, /onepair/);
  assert.match(updates.at(-1).embeds[0].toJSON().footer.text, /No clear boots/);
  await click('b1', 'chest');
  assert.match(updates.at(-1).embeds[0].toJSON().description, /anyonepair/);
  for (const id of ['b1', 'b2', 'b3']) { clearTimeout(testing.sessions.get(id).timer); testing.sessions.delete(id); }
});
