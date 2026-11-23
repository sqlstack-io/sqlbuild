<script lang="ts">
	import HelpTip from '$lib/components/help-tip.svelte';
	import { plural } from '$lib/format/plural';
	import type { CostPage } from './types';
	import { change, costTitle, creditsText, hours, isEstimate, priceSourceHelp, priceSourceLabel, usd } from './utils';

	// The period's totals as the Overview's metric strip draws them. `compare` adds the change
	// against the period before, which the API does not report yet.
	let { cost, compare = false }: { cost: CostPage; compare?: boolean } = $props();
	const c = $derived(change(cost.credits, cost.previous));
	const perDay = $derived(cost.credits / cost.days);
</script>

<section class="metric-strip" style:--metrics={4}>
	<div class="metric">
		<span class="metric-label">Cost · {cost.days} days <HelpTip text="Warehouse time the runs' queries used, priced per credit. Not the billed warehouse time." /></span>
		<div title={costTitle(cost.credits, cost.price)}><strong>{usd(cost.credits, cost.price)}</strong>{#if isEstimate(cost.price)}<small class="est">est.</small>{/if}</div>
		<span>{creditsText(cost.credits)}</span>
	</div>
	{#if compare}
		<div class="metric">
			<span class="metric-label">Change</span>
			<div><strong class={c?.tone}>{c?.text ?? '—'}</strong></div>
			<span title={costTitle(cost.previous, cost.price)}>{usd(cost.previous, cost.price)} previous {cost.days} days</span>
		</div>
	{/if}
	{#if !compare}<div class="metric">
		<span class="metric-label">Per day</span>
		<div title={costTitle(perDay, cost.price)}><strong>{usd(perDay, cost.price)}</strong></div>
		<span>{hours(cost.seconds)} warehouse time</span>
	</div>{/if}
	<div class="metric">
		<span class="metric-label">Runs</span>
		<div><strong>{cost.runCount.toLocaleString('en')}</strong><small>{usd(cost.credits / Math.max(1, cost.runCount), cost.price)} per run</small></div>
		<span class:warning-note={cost.incompleteRuns > 0} title="Cost still pending or partial for these runs">{cost.incompleteRuns ? `${plural(cost.incompleteRuns, 'run')} incomplete` : 'All complete'}</span>
	</div>
	<div class="metric">
		<span class="metric-label">Price <HelpTip text={priceSourceHelp[cost.price.source]} /></span>
		<div><strong>${cost.price.usdPerCredit.toFixed(2)}</strong><small>per credit</small>{#if isEstimate(cost.price)}<small class="est">est.</small>{/if}</div>
		<span title={cost.price.detail}>{priceSourceLabel[cost.price.source]}{#if cost.price.detail}&nbsp;· {cost.price.detail}{/if}</span>
	</div>
</section>

<style>
	.metric-strip { display: grid; margin-bottom: 22px; grid-template-columns: repeat(var(--metrics), minmax(0, 1fr)); overflow: hidden; box-shadow: var(--panel-shadow); border: 1px solid var(--panel-border); border-radius: 2px; background: var(--card); }
	.metric { min-width: 0; padding: 13px 16px 12px; border-right: 1px solid var(--border); }
	.metric:last-child { border-right: 0; }
	.metric-label { display: flex; align-items: center; gap: 4px; color: var(--muted-foreground); font: 500 12px var(--font-sans); }
	.metric > div { display: flex; align-items: baseline; gap: 8px; margin-top: 7px; }
	.metric strong { font: 600 24px/1 var(--font-sans); letter-spacing: -0.02em; font-variant-numeric: tabular-nums; }
	.metric small { color: var(--muted-foreground); font-size: 12px; }
	.metric small.est { color: var(--sb-warning); }
	.metric > span:last-child { display: block; margin-top: 9px; overflow: hidden; color: var(--sb-text-faint); font: 12px var(--font-sans); text-overflow: ellipsis; white-space: nowrap; }
	.metric .warning-note, strong.up { color: var(--sb-warning); }
	strong.down { color: var(--sb-success); }
</style>
