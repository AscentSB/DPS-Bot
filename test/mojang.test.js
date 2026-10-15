import test from 'node:test';
import assert from 'node:assert/strict';
import { isValidIgn, getMinecraftProfile } from '../src/api/mojang.js';

test('isValidIgn accepts Minecraft usernames only', () => {
  for (const ok of ['p3wr', 'BOBSTEVE21', 'a_b', 'ab', 'abcdefghijklmnop']) assert.equal(isValidIgn(ok), true, ok);
  for (const bad of ['a', 'abcdefghijklmnopq', '@everyone', 'a b', 'x/../y', 'https://x.y', '', null]) assert.equal(isValidIgn(bad), false, String(bad));
});

test('getMinecraftProfile never requests an invalid username', async (t) => {
  const fetch = t.mock.method(globalThis, 'fetch', async () => { throw new Error('should not fetch'); });
  assert.equal(await getMinecraftProfile('@everyone'), null);
  assert.equal(fetch.mock.callCount(), 0);
});

test('getMinecraftProfile rejects a malformed response', async (t) => {
  t.mock.method(globalThis, 'fetch', async () => new Response(JSON.stringify({ id: '../x', name: 'p3wr' })));
  assert.equal(await getMinecraftProfile('p3wr'), null);
});
