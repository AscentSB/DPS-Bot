import http from 'node:http';
import crypto from 'node:crypto';

// HTTP interactions (Interactions Endpoint URL): Discord POSTs each interaction here instead of sending it over the
// Gateway. Every request is checked against the app's public key (Ed25519 over timestamp + body, 401 if invalid),
// PINGs get a PONG, and commands and buttons get a small object with the discord.js methods the handlers use.
// The first reply / deferReply / update / deferUpdate is the HTTP response (due within 3 seconds); editReply goes
// through the interaction webhook, which works for 15 minutes.
// Source: https://docs.discord.com/developers/interactions/overview and .../receiving-and-responding

const API = 'https://discord.com/api/v10';
const MAX_BODY = 1_000_000;
// Requests older than this are refused, so a captured request cannot be replayed later.
const MAX_AGE_S = 5 * 60;
const TYPE = { PING: 1, COMMAND: 2, COMPONENT: 3 };
const CALLBACK = { PONG: 1, MESSAGE: 4, DEFERRED_MESSAGE: 5, DEFERRED_UPDATE: 6, UPDATE: 7 };
const EPHEMERAL = 1 << 6;
// DER header that turns a raw 32-byte Ed25519 key into an SPKI key Node can load.
const ED25519_SPKI_PREFIX = Buffer.from('302a300506032b6570032100', 'hex');

export function publicKeyFromHex(hex) {
  return crypto.createPublicKey({ key: Buffer.concat([ED25519_SPKI_PREFIX, Buffer.from(hex, 'hex')]), format: 'der', type: 'spki' });
}

export function verifySignature(key, signature, timestamp, body, now = Date.now()) {
  if (!/^[0-9a-f]{128}$/i.test(signature ?? '') || !/^\d{1,12}$/.test(timestamp ?? '')) return false;
  if (Math.abs(now / 1000 - Number(timestamp)) > MAX_AGE_S) return false;
  return crypto.verify(null, Buffer.from(timestamp + body), key, Buffer.from(signature, 'hex'));
}

// Message payload for the API: a string becomes content, ephemeral becomes the flag, and nothing can ping anyone.
function message(payload) {
  const { ephemeral, ...rest } = typeof payload === 'string' ? { content: payload } : payload;
  return { ...rest, ...(ephemeral ? { flags: EPHEMERAL } : {}), allowed_mentions: { parse: [] } };
}

// The parts of a discord.js interaction the /dps handlers read and call.
export function adaptInteraction(raw, respond, fetchImpl = fetch) {
  const sub = raw.data?.options?.find((o) => o.type === 1);
  const opts = sub?.options ?? raw.data?.options ?? [];
  const webhook = `${API}/webhooks/${raw.application_id}/${raw.token}/messages/@original`;
  return {
    id: raw.id,
    user: raw.member?.user ?? raw.user,
    commandName: raw.data?.name,
    customId: raw.data?.custom_id,
    isButton: () => raw.type === TYPE.COMPONENT && raw.data?.component_type === 2,
    isChatInputCommand: () => raw.type === TYPE.COMMAND && (raw.data?.type ?? 1) === 1,
    options: {
      getSubcommand: () => sub?.name ?? null,
      getString: (name, required = false) => {
        const value = opts.find((o) => o.name === name)?.value;
        if (value == null && required) throw new Error(`Missing option ${name}`);
        return value ?? null;
      },
    },
    reply: async (payload) => respond({ type: CALLBACK.MESSAGE, data: message(payload) }),
    deferReply: async () => respond({ type: CALLBACK.DEFERRED_MESSAGE }),
    update: async (payload) => respond({ type: CALLBACK.UPDATE, data: message(payload) }),
    deferUpdate: async () => respond({ type: CALLBACK.DEFERRED_UPDATE }),
    followUp: async (payload) => {
      const res = await fetchImpl(`${API}/webhooks/${raw.application_id}/${raw.token}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(message(payload)) });
      if (!res.ok) throw new Error(`Followup failed: HTTP ${res.status}`);
    },
    editReply: async (payload) => {
      const res = await fetchImpl(webhook, { method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(message(payload)) });
      if (!res.ok) throw new Error(`Edit original response failed: HTTP ${res.status}`);
    },
  };
}

// Node HTTP server for the Interactions Endpoint URL. onInteraction(interaction) gets each command or button.
export function createInteractionServer({ publicKey, onInteraction, fetchImpl = fetch }) {
  const key = publicKeyFromHex(publicKey);
  return http.createServer((req, res) => {
    const send = (status, body) => {
      if (res.headersSent) return;
      res.writeHead(status, { 'Content-Type': 'application/json' });
      res.end(body === undefined ? '' : JSON.stringify(body));
    };
    if (req.method !== 'POST') return send(405, { error: 'method not allowed' });
    const chunks = [];
    let size = 0;
    req.on('data', (chunk) => {
      size += chunk.length;
      if (size > MAX_BODY) { send(413, { error: 'too large' }); req.destroy(); } else chunks.push(chunk);
    });
    req.on('end', async () => {
      if (size > MAX_BODY) return;
      const body = Buffer.concat(chunks).toString('utf8');
      if (!verifySignature(key, req.headers['x-signature-ed25519'], req.headers['x-signature-timestamp'], body)) {
        return send(401, { error: 'invalid request signature' });
      }
      let raw;
      try { raw = JSON.parse(body); } catch { return send(400, { error: 'bad json' }); }
      if (raw.type === TYPE.PING) return send(200, { type: CALLBACK.PONG });
      const interaction = adaptInteraction(raw, async (payload) => send(200, payload), fetchImpl);
      try {
        await onInteraction(interaction);
      } catch (err) {
        console.error(err);
      }
      // A handler that never answered still has to close the request.
      send(500, { error: 'no response' });
    });
  });
}
