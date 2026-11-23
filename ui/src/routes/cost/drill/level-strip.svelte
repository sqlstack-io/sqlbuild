<script lang="ts">
	import HelpTip from '$lib/components/help-tip.svelte';
	import type { CostPage } from '../types';
	import { change, costTitle, hours, isEstimate, priceSourceHelp, priceSourceLabel, sharePct, usd, type DrillRow } from '../utils';

	// The open level's totals as one row of facts, with the price and where it came from.
	let { cost, totals, showModels = true }: { cost: CostPage; totals: Omit<DrillRow, 'kind' | 'key' | 'label' | 'deeper'>; showModels?: boolean } = $props();
	const c = $derived(change(totals.credits, totals.previous));
</script>

<dl class="strip">
	<div class="big" title={costTitle(totals.credits, cost.price)}>
		<dt>Cost · {cost.days} days</dt>
		<dd><strong>{usd(totals.credits, cost.price)}</strong>{#if isEstimate(cost.price)}<small class="est">est.</small>{/if}</dd>
	</div>
	<div><dt>Share of total</dt><dd>{sharePct(totals.credits, cost.credits)}</dd></div>
	<div title="{usd(totals.previous, cost.price)} in the {cost.days} days before"><dt>Change</dt><dd class={c?.tone}>{c?.text ?? '—'}</dd></div>
	<div><dt>Runs</dt><dd>{totals.runs.toLocaleString('en')}</dd></div>
	<div><dt>Queries</dt><dd>{totals.queries.toLocaleString('en')}</dd></div>
	{#if showModels}<div><dt>Models</dt><dd>{totals.models.toLocaleString('en')}</dd></div>{/if}
	<div><dt>Warehouse time</dt><dd>{hours(totals.seconds)}</dd></div>
	<div title={cost.price.detail}>
		<dt>Price <HelpTip text={priceSourceHelp[cost.price.source]} /></dt>
		<dd>${cost.price.usdPerCredit.toFixed(2)} per credit{#if isEstimate(cost.price)}<small class="est">est.</small>{/if} <small>{priceSourceLabel[cost.price.source]}</small></dd>
	</div>
</dl>

<style>
	.strip { display: flex; flex-wrap: wrap; align-items: flex-end; gap: 10px 28px; margin: 0; }
	.strip > div { min-width: 0; }
	dt { display: flex; align-items: center; gap: 4px; color: var(--muted-foreground); font-size: 11.5px; white-space: nowrap; }
	dd { margin: 3px 0 0; font-size: 14px; font-weight: 600; font-variant-numeric: tabular-nums; white-space: nowrap; }
	.big strong { font-size: 24px; letter-spacing: -0.02em; line-height: 1; }
	small { margin-left: 6px; color: var(--muted-foreground); font-size: 11.5px; font-weight: 400; }
	small.est { color: var(--sb-warning); }
	dd.up { color: var(--sb-warning); }
	dd.down { color: var(--sb-success); }
</style>
