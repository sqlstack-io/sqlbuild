<script lang="ts">
	import HelpTip from '$lib/components/help-tip.svelte';
	import { plural } from '$lib/format/plural';
	import type { CostPage } from './types';
	import { change, costTitle, creditsText, isEstimate, priceSourceHelp, priceSourceLabel, priceText, usd } from './utils';

	// The period's totals on one line, for a page that leads with its chart.
	let { cost }: { cost: CostPage } = $props();
	const c = $derived(change(cost.credits, cost.previous));
</script>

<div class="line">
	<strong title={costTitle(cost.credits, cost.price)}>{usd(cost.credits, cost.price)}</strong>
	{#if isEstimate(cost.price)}<span class="est">est.</span>{/if}
	<span>{creditsText(cost.credits)}</span>
	{#if c}<span class={c.tone} title="Against the {cost.days} days before">{c.text}</span>{/if}
	<span class="sep">·</span>
	<span>{plural(cost.runCount, 'run')}</span>
	<span class="sep">·</span>
	<span title={cost.price.detail}>{priceText(cost.price)} · {priceSourceLabel[cost.price.source]}</span>
	<HelpTip text={priceSourceHelp[cost.price.source]} />
</div>

<style>
	.line { display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 10px; margin-bottom: 14px; color: var(--muted-foreground); font-size: 12.5px; }
	strong { color: var(--foreground); font: 600 24px/1 var(--font-sans); letter-spacing: -0.02em; font-variant-numeric: tabular-nums; }
	.est, .up { color: var(--sb-warning); }
	.down { color: var(--sb-success); }
	.sep { color: var(--sb-text-faint); }
</style>
