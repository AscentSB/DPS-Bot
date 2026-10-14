import { SlashCommandBuilder, EmbedBuilder } from 'discord.js';
import { CLASSES } from '../config/constants.js';
import { getMinecraftProfile } from '../api/mojang.js';
import { getItemsResource, getSelectedProfile, readMember } from '../api/hypixel.js';
import { ensurePrices, prices } from '../api/prices.js';
import { makeContext, buildLoadout, planUpgrades, weightsFor } from '../calc/upgrades.js';
import { cataBoost } from '../calc/formulas.js';

export const data = new SlashCommandBuilder()
  .setName('dps')
  .setDescription('Rank your next DPS upgrades by % DPS gained per million coins')
  .addStringOption((o) => o.setName('ign').setDescription('Your Hypixel IGN').setRequired(true))
  .addStringOption((o) => o.setName('class').setDescription('Your dungeon class').setRequired(true)
    .addChoices({ name: 'Archer', value: 'arch' }, { name: 'Berserker', value: 'bers' }, { name: 'Mage', value: 'mage' }))
  .addStringOption((o) => o.setName('target').setDescription('Only rank for Withers or Dragons')
    .addChoices({ name: 'Withers', value: 'wither' }, { name: 'Dragons', value: 'dragon' }));

export function formatCoins(n) {
  if (n >= 1e9) return `${(n / 1e9).toFixed(2)}B`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(2)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`;
  return `${Math.round(n)}`;
}

export function buildEmbed({ name, cls, target, cataLevel, plan, weights }) {
  const lines = plan.steps.map((s, i) =>
    `**${i + 1}.** ${s.label}\n   +${s.gain.toFixed(2)}% for ${formatCoins(s.cost)} (${s.source}) · **${s.perMillion.toFixed(3)}%/M**`);
  const anyUnverified = plan.steps.some((s) => s.unverified);
  const fmt = (d) => `Wither ${formatCoins(d.wither)} · Dragon ${formatCoins(d.dragon)}`;
  const targetLabel = target === 'wither' ? 'Withers only' : target === 'dragon' ? 'Dragons only' : `Wither ${weights.wither} / Dragon ${weights.dragon}`;
  let description = lines.join('\n') || 'No priced upgrades found that raise your damage.';
  if (description.length > 4000) description = `${description.slice(0, 3990)}\n...`;
  return new EmbedBuilder()
    .setTitle(`${name} · ${CLASSES[cls].label} upgrades`)
    .setDescription(description)
    .addFields(
      { name: 'Weights', value: targetLabel, inline: true },
      { name: 'Catacombs', value: `Level ${cataLevel} (+${Math.round(cataBoost(cataLevel) * 100)}%)`, inline: true },
      { name: 'Accessory Power', value: `${plan.startDamage.ap} → ${plan.endDamage.ap}`, inline: true },
      { name: 'Damage now', value: fmt(plan.startDamage) },
      { name: 'Damage after all listed', value: fmt(plan.endDamage) },
    )
    .setFooter({
      text: [
        'Prices: bazaar instant buy (BZ) and auction lowest BIN (AH). Each step assumes the ones above it.',
        anyUnverified ? '(?) = not in the public API; skip it if you already have it.' : null,
      ].filter(Boolean).join('\n'),
    })
    .setColor(0x8e44ad);
}

export async function execute(interaction) {
  const ign = interaction.options.getString('ign', true).trim();
  const cls = interaction.options.getString('class', true);
  const target = interaction.options.getString('target');
  await interaction.deferReply();

  try {
    const account = await getMinecraftProfile(ign);
    if (!account) return interaction.editReply(`No Minecraft account named **${ign}**.`);

    const [resource, profile] = await Promise.all([getItemsResource(), getSelectedProfile(account.uuid), ensurePrices()]);
    if (!profile) return interaction.editReply(`**${account.name}** has no SkyBlock profiles.`);

    const member = readMember(profile, account.uuid, resource);
    const { cataLevel } = member;
    const ctx = makeContext({ cls, cataLevel, secrets: member.secrets, power: member.power, tuning: member.tuning, resource });
    const weights = weightsFor(cls, target);
    const loadout = buildLoadout(member, ctx, weights);
    const plan = planUpgrades(loadout, ctx, prices, weights);

    await interaction.editReply({ embeds: [buildEmbed({ name: account.name, cls, target, cataLevel, plan, weights })] });
  } catch (err) {
    console.error(err);
    await interaction.editReply(`Error: ${err.message}`);
  }
}
