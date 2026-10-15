import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { createInteractionServer } from '../src/http/interactions.js';

const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
const publicHex = publicKey.export({ format: 'der', type: 'spki' }).subarray(-32).toString('hex');

async function withServer(onInteraction, fn, fetchImpl) {
  const server = createInteractionServer({ publicKey: publicHex, onInteraction, fetchImpl });
  await new Promise((r) => server.listen(0, r));
  try { await fn(`http://127.0.0.1:${server.address().port}`); } finally { server.close(); }
}
function post(url, payload, { sign = true, timestamp = String(Math.floor(Date.now() / 1000)) } = {}) {
  const body = JSON.stringify(payload);
  const signature = sign ? crypto.sign(null, Buffer.from(timestamp + body), privateKey).toString('hex') : '00'.repeat(64);
  return fetch(url, { method: 'POST', headers: { 'X-Signature-Ed25519': signature, 'X-Signature-Timestamp': timestamp }, body });
}

test('HTTP interactions: PING gets PONG; bad or stale signatures get 401', async () => {
  await withServer(() => {}, async (url) => {
    const ok = await post(url, { type: 1 });
    assert.equal(ok.status, 200);
    assert.deepEqual(await ok.json(), { type: 1 });
    assert.equal((await post(url, { type: 1 }, { sign: false })).status, 401);
    assert.equal((await post(url, { type: 1 }, { timestamp: String(Math.floor(Date.now() / 1000) - 3600) })).status, 401);
  });
});

test('HTTP interactions: commands read options, defer, then edit the original response without pings', async () => {
  const edits = [];
  const fakeFetch = async (url, init) => { edits.push({ url, init }); return { ok: true, status: 200 }; };
  let seen;
  const done = new Promise((resolve) => {
    seen = async (i) => {
      assert.equal(i.isChatInputCommand(), true);
      assert.equal(i.options.getSubcommand(), 'mage');
      assert.equal(i.options.getString('ign', true), 'p3wr');
      assert.equal(i.user.id, 'u1');
      await i.deferReply();
      await i.editReply('done @everyone');
      resolve();
    };
  });
  await withServer((i) => seen(i), async (url) => {
    const res = await post(url, { type: 2, id: 'i1', application_id: 'app', token: 'tok', member: { user: { id: 'u1' } }, data: { name: 'dps', type: 1, options: [{ type: 1, name: 'mage', options: [{ name: 'ign', value: 'p3wr' }] }] } });
    assert.deepEqual(await res.json(), { type: 5 });
    await done;
  }, fakeFetch);
  assert.equal(edits[0].url, 'https://discord.com/api/v10/webhooks/app/tok/messages/@original');
  assert.deepEqual(JSON.parse(edits[0].init.body), { content: 'done @everyone', allowed_mentions: { parse: [] } });
});

test('HTTP interactions: ephemeral replies set the flag; buttons are recognized', async () => {
  await withServer(async (i) => {
    assert.equal(i.isButton(), true);
    assert.equal(i.customId, 'dps:1:next');
    await i.reply({ content: 'only you', ephemeral: true });
  }, async (url) => {
    const res = await post(url, { type: 3, id: 'i2', application_id: 'app', token: 't', user: { id: 'u2' }, data: { component_type: 2, custom_id: 'dps:1:next' } });
    assert.deepEqual(await res.json(), { type: 4, data: { content: 'only you', flags: 64, allowed_mentions: { parse: [] } } });
  });
});
