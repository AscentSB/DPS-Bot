import { escapeMarkdown, SlashCommandBuilder, EmbedBuilder, ActionRowBuilder, ButtonBuilder, ButtonStyle } from 'discord.js';
import { CLASSES } from '../config/constants.js';
import { getMinecraftProfile, isValidIgn } from '../api/mojang.js';
import { getItemsResource, getSelectedProfile, readMember } from '../api/hypixel.js';
import { runInWorker, PlanTimeoutError, PlanCancelledError } from '../worker/client.js';
import { pricesReady } from '../api/prices.js';

// A /dps message goes inactive after this long without a button press (its buttons are removed, its data freed and
// any calculation it started is stopped). Change it here.
export const INACTIVITY_MS = 20 * 1000;
const INACTIVE_NOTE = 'This session is inactive. Run /dps again to start a new one.';
const EXPIRED_REPLY = 'This /dps session expired. Run /dps again to start a new one.';
// Interaction tokens last 15 minutes; past this the fallback edit through the token is not tried.
const TOKEN_EDIT_MS = 14 * 60 * 1000;

// One subcommand per class, so the Mage-only options only show under /dps mage.
const SUBCOMMAND_CLASS = { archer: 'arch', berserker: 'bers', mage: 'mage' };
const ign = (o) => o.setName('ign').setDescription('Your Hypixel IGN').setRequired(true);
const target = (o) => o.setName('target').setDescription('Only rank for Withers or Dragons')
  .addChoices({ name: 'Withers', value: 'wither' }, { name: 'Dragons', value: 'dragon' });

export const data = new SlashCommandBuilder()
  .setName('dps')
  .setDescription('Your next DPS upgrades, ranked by DPS gained per million coins')
  .addSubcommand((c) => c.setName('archer').setDescription('Archer upgrades').addStringOption(ign).addStringOption(target))
  .addSubcommand((c) => c.setName('berserker').setDescription('Berserker upgrades').addStringOption(ign).addStringOption(target))
  .addSubcommand((c) => c.setName('mage').setDescription('Mage upgrades').addStringOption(ign)
    .addStringOption((o) => o.setName('set').setDescription('Boss set (LCM beam) or clear set (RCM, 500 speed)')
      .addChoices({ name: 'Boss set (LCM)', value: 'lcm' }, { name: 'Clear set (RCM)', value: 'rcm' }))
    .addStringOption((o) => o.setName('floor').setDescription('Boss set only: Master Mode (M7) or Normal Mode (F7)')
      .addChoices({ name: 'Master Mode', value: 'master' }, { name: 'Normal Mode', value: 'normal' }))
    .addStringOption(target));

// Coin plans are computed this deep and shown PAGE_SIZE at a time.
const PLAN_STEPS = 40;
const PAGE_SIZE = 8;
// Button sessions are kept this long after the last use.
const SESSION_MS = 30 * 60 * 1000;
// Per-user /dps cooldown.
const COOLDOWN_MS = 10 * 1000;
const lastRun = new Map();
const sessions = new Map();

const TUNING_LABELS = { strength: 'strength', critDamage: 'crit damage', critChance: 'crit chance', intelligence: 'intelligence' };

function tuningText(tuning, split) {
  const parts = [];
  if (tuning?.attackSpeed) parts.push(`${tuning.attackSpeed} attack speed`);
  if (tuning?.speed) parts.push(`${tuning.speed} speed`);
  for (const [stat, share] of Object.entries(split ?? {})) {
    const points = Math.round((tuning?.dps ?? 0) * share);
    if (points) parts.push(`${points} ${TUNING_LABELS[stat] ?? stat}`);
  }
  return parts.join(' · ') || 'none';
}

export function formatCoins(n) {
  if (n >= 1e9) return `${(n / 1e9).toFixed(2)}B`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(2)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`;
  return `${Math.round(n)}`;
}

function damageText(d, rcm) {
  return rcm ? `${formatCoins(d.wither)} per Wither Impact` : `Withers ${formatCoins(d.wither)}\nDragons ${formatCoins(d.dragon)}`;
}

function title(s) {
  const set = s.cls === 'mage' ? (s.mode === 'rcm' ? ' · Clear set' : s.floorMode === 'normal' ? ' · Boss set (F7)' : ' · Boss set') : '';
  return `${escapeMarkdown(s.name)} · ${CLASSES[s.cls].label}${set}`;
}

export function pageCount(s) {
  const rows = s.view === 'profile' ? s.profile?.rows ?? [] : s.coins?.[s.upgradeMode]?.plan.steps ?? [];
  return Math.max(1, Math.ceil(rows.length / PAGE_SIZE));
}

export function buildEmbed(s) {
  const rcm = s.cls === 'mage' && s.mode === 'rcm';
  const first = s.page * PAGE_SIZE;
  if (s.view === 'profile') {
    const rows = s.profile.rows.slice(first, first + PAGE_SIZE);
    const lines = rows.map((r, i) => `**${first + i + 1}. ${r.label}**\n+${r.gain.toFixed(2)}% DPS`);
    return new EmbedBuilder()
      .setTitle(`${title(s)} · Profile upgrades`)
      .setDescription(lines.join('\n\n') || 'No profile upgrade raises your damage.')
      .addFields({ name: 'Damage now', value: damageText(s.profile.damage, rcm), inline: true })
      .setFooter({ text: `Page ${s.page + 1}/${pageCount(s)} · Skills, slayers, class and Catacombs levels, SkyBlock level and Harp songs. No coins.` })
      .setColor(0x2e86c1);
  }
  const { plan } = s.coins[s.upgradeMode];
  const steps = plan.steps.slice(first, first + PAGE_SIZE);
  const lines = steps.map((st, i) => `**${first + i + 1}. ${st.label}**\n+${st.gain.toFixed(2)}% DPS · ${formatCoins(st.cost)} coins · ${st.perMillion >= 1 ? st.perMillion.toFixed(5) : st.perMillion.toFixed(6)}% per 1M`);
  const unverified = plan.steps.some((st) => st.unverified);
  return new EmbedBuilder()
    .setTitle(`${title(s)} · Coin upgrades`)
    .setDescription(lines.join('\n\n') || 'No priced upgrade raises your damage.')
    .addFields(
      { name: 'Damage now', value: damageText(plan.startDamage, rcm), inline: true },
      { name: `After all ${plan.steps.length}`, value: damageText(plan.endDamage, rcm), inline: true },
      { name: 'Tuning', value: tuningText(plan.startDamage.tuning, s.coins[s.upgradeMode].tuningSplit) },
    )
    .setFooter({ text: [`Page ${s.page + 1}/${pageCount(s)} · ${s.upgradeMode === 'max' ? 'Max levels' : 'Every level'} · Each step assumes the ones above it. Prices: bazaar and auction lowest BIN.`, unverified ? '(?) = not in the public API; skip it if you already have it.' : null].filter(Boolean).join('\n') })
    .setColor(0x8e44ad);
}

function buttons(id, s) {
  const pages = pageCount(s);
  return [new ActionRowBuilder().addComponents(
    new ButtonBuilder().setCustomId(`dps:${id}:prev`).setLabel('◀ Back').setStyle(ButtonStyle.Secondary).setDisabled(s.page === 0),
    new ButtonBuilder().setCustomId(`dps:${id}:next`).setLabel('Next ▶').setStyle(ButtonStyle.Secondary).setDisabled(s.page >= pages - 1),
    new ButtonBuilder().setCustomId(`dps:${id}:coins`).setLabel('Coin upgrades').setStyle(s.view === 'coins' ? ButtonStyle.Primary : ButtonStyle.Secondary).setDisabled(s.view === 'coins'),
    new ButtonBuilder().setCustomId(`dps:${id}:profile`).setLabel('Profile upgrades').setStyle(s.view === 'profile' ? ButtonStyle.Primary : ButtonStyle.Secondary).setDisabled(s.view === 'profile'),
    // All levels lists Duplex I, II, ... separately; max levels jumps straight to the max level at its cost.
    new ButtonBuilder().setCustomId(`dps:${id}:levels`).setLabel(s.upgradeMode === 'all' ? 'Show max levels' : 'Show every level').setStyle(ButtonStyle.Secondary).setDisabled(s.view !== 'coins'),
  )];
}

function prune() {
  const now = Date.now();
  for (const [id, s] of sessions) if (now - s.usedAt > SESSION_MS) expire(id);
}

// (Re)start a session's inactivity timer. Each session has its own, so one user's never touches another's.
function startTimer(id, s) {
  clearTimeout(s.timer);
  s.timer = setTimeout(() => expire(id), INACTIVITY_MS);
  s.timer.unref?.();
}

// Inactive: forget the session (freeing its plans and profile data), stop a calculation it started, and edit the
// message to drop its buttons and add a note. The edit goes to the message itself (bot permission, no 15-minute
// token limit); if that fails (no access to the channel, e.g. a user-installed app) it falls back to the latest
// interaction's token while still valid. A deleted message or missing permission is logged, never thrown.
async function expire(id) {
  const s = sessions.get(id);
  if (!s) return;
  sessions.delete(id);
  clearTimeout(s.timer);
  s.abort?.abort();
  const edit = { content: INACTIVE_NOTE, components: [] };
  try {
    if (!s.message) throw new Error('No message to edit.');
    await s.message.edit(edit);
  } catch (err) {
    try {
      if (!s.interaction || Date.now() - s.interactionAt > TOKEN_EDIT_MS) throw err;
      await s.interaction.editReply(edit);
    } catch (err2) {
      console.warn(`Could not mark a /dps message inactive (${err2?.code ?? err2?.message ?? err2}).`);
    }
  }
}

// Fixed replies for failures (never the raw error, which can hold outside text).
const PRICES_LOADING = 'Prices are still loading after a restart. Try again in a moment.';
const MOJANG_DOWN = 'Could not reach Mojang to look up that username. Try again in a minute.';
const PLAN_TOO_LONG = 'This calculation took too long and was stopped. Try again in a minute.';
function hypixelError(err) {
  const msg = String(err?.message ?? '');
  if (msg.includes('HYPIXEL_API_KEY') || /HTTP 403/.test(msg)) return 'The bot\'s Hypixel API key is missing or invalid. Tell the bot owner.';
  if (/HTTP 429/.test(msg)) return 'The Hypixel API rate limit was reached. Try again in a minute.';
  return 'Could not reach the Hypixel API. Try again in a minute.';
}

export async function execute(interaction) {
  const now = Date.now();
  for (const [user, at] of lastRun) if (now - at >= COOLDOWN_MS) lastRun.delete(user);
  const last = lastRun.get(interaction.user.id);
  if (last) {
    const wait = Math.ceil((COOLDOWN_MS - (now - last)) / 1000);
    return interaction.reply({ content: `Please wait ${wait}s before using /dps again.`, ephemeral: true });
  }
  lastRun.set(interaction.user.id, now);
  const sub = interaction.options.getSubcommand();
  const cls = SUBCOMMAND_CLASS[sub];
  const name = interaction.options.getString('ign', true).trim();
  const mode = cls === 'mage' ? interaction.options.getString('set') ?? 'lcm' : 'lcm';
  const floorMode = cls === 'mage' && mode === 'lcm' ? interaction.options.getString('floor') ?? 'master' : 'master';
  const job = { cls, target: interaction.options.getString('target'), mode, floorMode };
  // Replies never repeat the typed IGN or raw error text, so user input cannot reach the message.
  if (!isValidIgn(name)) {
    return interaction.reply({ content: 'That is not a valid Minecraft username (2 to 16 letters, digits or underscores).', ephemeral: true });
  }
  if (!pricesReady()) return interaction.reply({ content: PRICES_LOADING, ephemeral: true });
  await interaction.deferReply();

  try {
    let account;
    try { account = await getMinecraftProfile(name); } catch (err) { console.error(err); return interaction.editReply(MOJANG_DOWN); }
    if (!account) return interaction.editReply('No Minecraft account has that username.');
    let resource;
    let profile;
    try { [resource, profile] = await Promise.all([getItemsResource(), getSelectedProfile(account.uuid)]); } catch (err) { console.error(err); return interaction.editReply(hypixelError(err)); }
    if (!profile) return interaction.editReply(`**${escapeMarkdown(account.name)}** has no SkyBlock profiles.`);
    const member = readMember(profile, account.uuid, resource);

    // The planner runs in a worker thread so the bot stays connected while it works.
    const coins = { all: await runInWorker({ ...job, upgradeMode: 'all', member, menu: 'coins', limit: PLAN_STEPS }) };
    prune();
    const id = interaction.id;
    const s = { ...job, upgradeMode: 'all', name: account.name, member, coins, profile: null, view: 'coins', page: 0, owner: interaction.user.id, usedAt: Date.now() };
    sessions.set(id, s);
    // The message is kept for the inactivity edit (discord.js returns it; HTTP mode uses the token instead).
    s.message = await interaction.editReply({ embeds: [buildEmbed(s)], components: buttons(id, s) });
    Object.assign(s, { interaction, interactionAt: Date.now() });
    // The timer starts once results are on screen (the first plan can take a while and is not inactivity).
    startTimer(id, s);
  } catch (err) {
    console.error(err);
    // readMember explains a profile with its Inventory API turned off; that text is ours, not outside input.
    const text = err instanceof PlanTimeoutError ? PLAN_TOO_LONG : /Inventory API is disabled/.test(err?.message) ? err.message
      : 'Something went wrong while loading this profile. Try again in a minute.';
    await interaction.editReply(text);
  }
}

// For tests: the session store and timer start.
export const testing = { sessions, startTimer };

// Back / Next / Coin upgrades / Profile upgrades.
export async function handleButton(interaction) {
  const [, id, action] = interaction.customId.split(':');
  const s = sessions.get(id);
  if (!s) return interaction.reply({ content: EXPIRED_REPLY, ephemeral: true });
  if (interaction.user.id !== s.owner) return interaction.reply({ content: 'Only the person who ran this /dps can use these buttons.', ephemeral: true });
  s.usedAt = Date.now();
  startTimer(id, s);
  Object.assign(s, { interaction, interactionAt: Date.now() });
  if (action === 'prev') s.page = Math.max(0, s.page - 1);
  if (action === 'next') s.page = Math.min(pageCount(s) - 1, s.page + 1);
  if (action === 'coins' || action === 'profile') { s.view = action; s.page = 0; }
  if (action === 'levels') { s.upgradeMode = s.upgradeMode === 'all' ? 'max' : 'all'; s.page = 0; }
  // Profile upgrades and the max-level plan are worked out the first time they are opened.
  const job = { cls: s.cls, target: s.target, mode: s.mode, floorMode: s.floorMode, member: s.member };
  const missing = s.view === 'profile' ? !s.profile : !s.coins[s.upgradeMode];
  if (missing) {
    await interaction.deferUpdate();
    // A calculation started here is stopped if the session goes inactive before it finishes.
    s.abort = new AbortController();
    const { signal } = s.abort;
    try {
      if (s.view === 'profile') s.profile = (await runInWorker({ ...job, menu: 'profile' }, { signal })).profile;
      else s.coins[s.upgradeMode] = await runInWorker({ ...job, upgradeMode: s.upgradeMode, menu: 'coins', limit: PLAN_STEPS }, { signal });
    } catch (err) {
      // Stopped because the session went inactive: the message already says so.
      if (err instanceof PlanCancelledError || sessions.get(id) !== s) return;
      console.error(err);
      s.view = 'coins';
      s.upgradeMode = 'all';
      await interaction.followUp({ content: err instanceof PlanTimeoutError ? PLAN_TOO_LONG : 'Could not work that list out. Try again in a minute.', ephemeral: true });
    }
    // The session may have gone inactive while this ran; then its buttons stay removed.
    if (sessions.get(id) !== s) return;
    s.message = (await interaction.editReply({ embeds: [buildEmbed(s)], components: buttons(id, s) })) ?? s.message;
    // New results on screen: the inactivity timer starts over.
    startTimer(id, s);
    return;
  }
  await interaction.update({ embeds: [buildEmbed(s)], components: buttons(id, s) });
}
