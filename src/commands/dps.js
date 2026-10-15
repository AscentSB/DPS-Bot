import { SlashCommandBuilder, EmbedBuilder, ActionRowBuilder, ButtonBuilder, ButtonStyle } from 'discord.js';
import { CLASSES } from '../config/constants.js';
import { getMinecraftProfile } from '../api/mojang.js';
import { getItemsResource, getSelectedProfile, readMember } from '../api/hypixel.js';
import { runInWorker } from '../worker/client.js';

// One subcommand per class, so the Mage-only options only show under /dps mage.
const SUBCOMMAND_CLASS = { archer: 'arch', berserker: 'bers', mage: 'mage' };
const ign = (o) => o.setName('ign').setDescription('Your Hypixel IGN').setRequired(true);
const target = (o) => o.setName('target').setDescription('Only rank for Withers or Dragons')
  .addChoices({ name: 'Withers', value: 'wither' }, { name: 'Dragons', value: 'dragon' });
const levels = (o) => o.setName('upgrades').setDescription('List every level (Duplex I, II, ...) or jump to the max level')
  .addChoices({ name: 'All levels', value: 'all' }, { name: 'Max level only', value: 'max' });

export const data = new SlashCommandBuilder()
  .setName('dps')
  .setDescription('Your next DPS upgrades, ranked by DPS gained per million coins')
  .addSubcommand((c) => c.setName('archer').setDescription('Archer upgrades').addStringOption(ign).addStringOption(target).addStringOption(levels))
  .addSubcommand((c) => c.setName('berserker').setDescription('Berserker upgrades').addStringOption(ign).addStringOption(target).addStringOption(levels))
  .addSubcommand((c) => c.setName('mage').setDescription('Mage upgrades').addStringOption(ign)
    .addStringOption((o) => o.setName('set').setDescription('Boss set (LCM beam) or clear set (RCM, 500 speed)')
      .addChoices({ name: 'Boss set (LCM)', value: 'lcm' }, { name: 'Clear set (RCM)', value: 'rcm' }))
    .addStringOption((o) => o.setName('floor').setDescription('Boss set only: Master Mode (M7) or Normal Mode (F7)')
      .addChoices({ name: 'Master Mode', value: 'master' }, { name: 'Normal Mode', value: 'normal' }))
    .addStringOption(target).addStringOption(levels));

// Coin plans are computed this deep and shown PAGE_SIZE at a time.
const PLAN_STEPS = 40;
const PAGE_SIZE = 8;
// Button sessions are kept this long after the last use.
const SESSION_MS = 30 * 60 * 1000;
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
  return `${s.name} · ${CLASSES[s.cls].label}${set}`;
}

export function pageCount(s) {
  const rows = s.view === 'profile' ? s.profile?.rows ?? [] : s.coins?.plan.steps ?? [];
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
  const { plan } = s.coins;
  const steps = plan.steps.slice(first, first + PAGE_SIZE);
  const lines = steps.map((st, i) => `**${first + i + 1}. ${st.label}**\n+${st.gain.toFixed(2)}% DPS · ${formatCoins(st.cost)} coins · ${st.perMillion >= 1 ? st.perMillion.toFixed(2) : st.perMillion.toFixed(3)}% per 1M`);
  const unverified = plan.steps.some((st) => st.unverified);
  return new EmbedBuilder()
    .setTitle(`${title(s)} · Coin upgrades`)
    .setDescription(lines.join('\n\n') || 'No priced upgrade raises your damage.')
    .addFields(
      { name: 'Damage now', value: damageText(plan.startDamage, rcm), inline: true },
      { name: `After all ${plan.steps.length}`, value: damageText(plan.endDamage, rcm), inline: true },
      { name: 'Tuning', value: tuningText(plan.startDamage.tuning, s.coins.tuningSplit) },
    )
    .setFooter({ text: [`Page ${s.page + 1}/${pageCount(s)} · Each step assumes the ones above it. Prices: bazaar and auction lowest BIN.`, unverified ? '(?) = not in the public API; skip it if you already have it.' : null].filter(Boolean).join('\n') })
    .setColor(0x8e44ad);
}

function buttons(id, s) {
  const pages = pageCount(s);
  return [new ActionRowBuilder().addComponents(
    new ButtonBuilder().setCustomId(`dps:${id}:prev`).setLabel('◀ Back').setStyle(ButtonStyle.Secondary).setDisabled(s.page === 0),
    new ButtonBuilder().setCustomId(`dps:${id}:next`).setLabel('Next ▶').setStyle(ButtonStyle.Secondary).setDisabled(s.page >= pages - 1),
    new ButtonBuilder().setCustomId(`dps:${id}:coins`).setLabel('Coin upgrades').setStyle(s.view === 'coins' ? ButtonStyle.Primary : ButtonStyle.Secondary).setDisabled(s.view === 'coins'),
    new ButtonBuilder().setCustomId(`dps:${id}:profile`).setLabel('Profile upgrades').setStyle(s.view === 'profile' ? ButtonStyle.Primary : ButtonStyle.Secondary).setDisabled(s.view === 'profile'),
  )];
}

function prune() {
  const now = Date.now();
  for (const [id, s] of sessions) if (now - s.usedAt > SESSION_MS) sessions.delete(id);
}

export async function execute(interaction) {
  const sub = interaction.options.getSubcommand();
  const cls = SUBCOMMAND_CLASS[sub];
  const name = interaction.options.getString('ign', true).trim();
  const mode = cls === 'mage' ? interaction.options.getString('set') ?? 'lcm' : 'lcm';
  const floorMode = cls === 'mage' && mode === 'lcm' ? interaction.options.getString('floor') ?? 'master' : 'master';
  const job = { cls, target: interaction.options.getString('target'), upgradeMode: interaction.options.getString('upgrades') ?? 'all', mode, floorMode };
  await interaction.deferReply();

  try {
    const account = await getMinecraftProfile(name);
    if (!account) return interaction.editReply(`No Minecraft account named **${name}**.`);
    const [resource, profile] = await Promise.all([getItemsResource(), getSelectedProfile(account.uuid)]);
    if (!profile) return interaction.editReply(`**${account.name}** has no SkyBlock profiles.`);
    const member = readMember(profile, account.uuid, resource);

    // The planner runs in a worker thread so the bot stays connected while it works.
    const coins = await runInWorker({ ...job, member, menu: 'coins', limit: PLAN_STEPS });
    prune();
    const id = interaction.id;
    const s = { ...job, name: account.name, member, coins, profile: null, view: 'coins', page: 0, owner: interaction.user.id, usedAt: Date.now() };
    sessions.set(id, s);
    await interaction.editReply({ embeds: [buildEmbed(s)], components: buttons(id, s) });
  } catch (err) {
    console.error(err);
    await interaction.editReply(`Error: ${err.message}`);
  }
}

// Back / Next / Coin upgrades / Profile upgrades.
export async function handleButton(interaction) {
  const [, id, action] = interaction.customId.split(':');
  const s = sessions.get(id);
  if (!s) return interaction.reply({ content: 'These results expired. Run /dps again.', ephemeral: true });
  if (interaction.user.id !== s.owner) return interaction.reply({ content: 'Only the person who ran this /dps can use these buttons.', ephemeral: true });
  s.usedAt = Date.now();
  if (action === 'prev') s.page = Math.max(0, s.page - 1);
  if (action === 'next') s.page = Math.min(pageCount(s) - 1, s.page + 1);
  if (action === 'coins' || action === 'profile') { s.view = action; s.page = 0; }
  if (s.view === 'profile' && !s.profile) {
    await interaction.deferUpdate();
    try {
      s.profile = (await runInWorker({ cls: s.cls, target: s.target, upgradeMode: s.upgradeMode, mode: s.mode, floorMode: s.floorMode, member: s.member, menu: 'profile' })).profile;
    } catch (err) {
      console.error(err);
      s.view = 'coins';
    }
    return interaction.editReply({ embeds: [buildEmbed(s)], components: buttons(id, s) });
  }
  await interaction.update({ embeds: [buildEmbed(s)], components: buttons(id, s) });
}
