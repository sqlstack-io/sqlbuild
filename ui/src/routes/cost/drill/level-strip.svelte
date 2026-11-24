<script lang="ts">
	import type { CostPrice } from '$lib/api';
	import HelpTip from '$lib/components/help-tip.svelte';
	import { CREDIT_PRICE_HINT, PRICE_SOURCE_LABEL, isEstimate } from '$lib/operate/cost';
	import { change, costTitle, count, hours, money, priceHelp, sharePct } from '../utils/format';
	import type { Totals } from '../utils/tree';

	// The level's totals as one row of facts, with the price and where it came from.
	let { totals, days, whole, price }: { totals: Totals; days: number; whole: number; price: CostPrice | null } = $props();
	const c = $derived(change(totals.credits, totals.previous));
</script>

<dl class="strip">
	<div class="big" title={costTitle(totals.credits, price)}>
		<dt>Cost · {days} days</dt>
		<dd><strong>{money(totals.credits, price)}</strong>{#if isEstimate(price?.source)}<small class="est" title="Priced at the list price: an estimate">est.</small>{/if}</dd>
	</div>
	<div><dt>Share of total</dt><dd>{sharePct(totals.credits, whole)}</dd></div>
	<div title="{money(totals.previous, price)} in the {days} days before"><dt>Change</dt><dd class={c?.tone}>{c?.text ?? '—'}</dd></div>
	<div><dt>Runs</dt><dd>{count(totals.runs)}</dd></div>
	<div><dt>Queries</dt><dd>{count(totals.queries)}</dd></div>
	<div><dt>Models</dt><dd>{count(totals.models)}</dd></div>
	<div><dt>Warehouse time</dt><dd>{hours(totals.seconds)}</dd></div>
	<div>
		{#if price}
			<dt>Price <HelpTip text={priceHelp(price)} label="Where the price comes from" /></dt>
			<dd>${price.usdPerCredit.toFixed(2)} per credit{#if isEstimate(price.source)}<small class="est">est.</small>{/if} <small>{PRICE_SOURCE_LABEL[price.source]}</small></dd>
		{:else}
			<dt>Price <HelpTip text={CREDIT_PRICE_HINT} label="Why credits" /></dt>
			<dd class="muted">Not set</dd>
		{/if}
	</div>
</dl>

<style>
	.strip { display: flex; flex-wrap: wrap; align-items: flex-end; gap: 10px 28px; margin: 0; }
	.strip > div { min-width: 0; }
	dt { display: flex; align-items: center; gap: 4px; color: var(--muted-foreground); font-size: 11.5px; white-space: nowrap; }
	dd { margin: 3px 0 0; font-size: 14px; font-weight: 600; font-variant-numeric: tabular-nums; white-space: nowrap; }
	dd.muted { color: var(--muted-foreground); font-weight: 400; }
	small { margin-left: 6px; color: var(--muted-foreground); font-size: 11.5px; font-weight: 400; }
	small.est { color: var(--sb-warning); }
	.big strong { font-size: 24px; letter-spacing: -0.02em; line-height: 1; }
	dd.up { color: var(--sb-warning); }
	dd.down { color: var(--sb-success); }
</style>
