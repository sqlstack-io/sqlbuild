<script lang="ts">
	import { to } from '$lib/paths';
	import { mockCostPage } from '$lib/api/mock/cost-page';
	import { change, costTitle, creditsText, isEstimate, usd } from './cost/utils';

	// Mockup (CHI-1650): the Overview's "Cost · 7 days" card, linking to the Cost page.
	let { variant }: { variant: string } = $props();
	const cost = $derived(mockCostPage(7, variant === 'b' || variant === 'd' ? 'list_price' : 'rate_sheet'));
	const c = $derived(change(cost.credits, cost.previous));
</script>

<a class="metric" href={to(`/cost?variant=${variant}`)}>
	<span class="metric-label">Cost · 7 days</span>
	<div title={costTitle(cost.credits, cost.price)}><strong>{usd(cost.credits, cost.price)}</strong>{#if isEstimate(cost.price)}<small class="est">est.</small>{/if}<small>{creditsText(cost.credits)}</small></div>
	<span class:warning-note={c?.tone === 'up'}>{c?.text ?? '—'} vs previous 7 days</span>
</a>

<style>
	.metric { display: block; min-width: 0; padding: 15px 16px 14px; color: inherit; text-decoration: none; }
	.metric:hover { background: var(--sb-hover); }
	.metric-label { display: block; color: var(--muted-foreground); font: 500 12px var(--font-sans); }
	.metric > div { display: flex; align-items: baseline; gap: 8px; margin-top: 7px; }
	strong { font: 600 24px/1 var(--font-sans); letter-spacing: -0.02em; font-variant-numeric: tabular-nums; }
	small { color: var(--muted-foreground); font-size: 12px; }
	small.est { color: var(--sb-warning); }
	.metric > span:last-child { display: block; margin-top: 9px; overflow: hidden; color: var(--sb-text-faint); font: 12px var(--font-sans); text-overflow: ellipsis; white-space: nowrap; }
	.warning-note { color: var(--sb-warning) !important; }
</style>
