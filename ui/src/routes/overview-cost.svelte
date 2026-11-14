<script lang="ts">
	import { to } from '$lib/paths';
	import { ApiError, DEMO } from '$lib/api';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { CREDIT_PRICE_HINT, costByFolder, formatQueryCost } from '$lib/operate/cost';

	// Estimated query cost of the target's recent runs. Hidden until any run has cost, so
	// adapters without query cost estimates show nothing rather than zeros.
	const WINDOWS = [7, 30] as const;
	const TOP = 5;
	let days = $state<(typeof WINDOWS)[number]>(7);
	// The demo recording may hold no cost read; that is no cost data, not an error.
	const unrecorded = (error: unknown) => DEMO && error instanceof ApiError && error.status === 404;
	const summary = createResource(async (client, scope) => {
		const window = days;
		const cost = await client.getCostSummary(scope, window).catch((error: unknown) => {
			if (unrecorded(error)) return null;
			throw error;
		});
		if (!cost) return null;
		return { cost, folders: costByFolder(cost.byNode, await client.listCatalogNodes(scope)) };
	}, ['runs', 'warehouse_sync', 'snapshot']);
	const data = $derived(summary.data ?? null);
	const cost = $derived(data?.cost ?? null);
	type Row = { key: string; label: string; credits: number; usd: number | null; detail: string; href?: string };
	const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
	const columns = $derived<{ title: string; rows: Row[] }[]>(
		cost && data
			? [
					{
						title: 'By job',
						rows: cost.byJob.map((g) => ({ key: g.key ?? '', label: g.key ?? 'Started by hand or from the command line', credits: g.credits, usd: g.usd, detail: plural(g.runs, 'run'), href: g.key ? to(`/jobs/${g.key}`) : undefined }))
					},
					{
						title: 'By folder',
						rows: data.folders.map((f) => ({ key: f.key, label: f.key, credits: f.credits, usd: f.usd, detail: plural(f.nodes, 'node'), href: f.key === 'other' ? undefined : to(`/catalog?group=${f.key}`) }))
					},
					{
						title: 'By warehouse size',
						rows: cost.byWarehouseSize.map((g) => ({ key: g.key ?? '', label: g.key ?? 'Unknown size', credits: g.credits, usd: g.usd, detail: plural(g.runs, 'run') }))
					}
				]
			: []
	);
</script>

{#if app.client.supports('getCostSummary') && (cost?.runs || summary.error)}
	<section class="surface-panel cost">
		<div class="panel-heading">
			<div>
				<h2>Estimated query cost</h2>
				<span>Warehouse time the runs' queries used, not billed warehouse time</span>
			</div>
			<div class="windows" role="group" aria-label="Window">
				{#each WINDOWS as w (w)}
					<button class:on={days === w} aria-pressed={days === w} onclick={() => (days = w)}>{w} days</button>
				{/each}
			</div>
		</div>
		{#if summary.error && !cost}
			<p class="note err">Couldn't load cost: {summary.error}</p>
		{:else if cost}
			<div class="total">
				<strong>{formatQueryCost(cost)}</strong>
				<span>{plural(cost.runs, 'run')} in the last {days} days{#if cost.incompleteRuns}&nbsp;· <b>{cost.incompleteRuns} still pending or partial</b>{/if}</span>
				{#if cost.usd === null}<span class="hint">{CREDIT_PRICE_HINT}</span>{/if}
			</div>
			<div class="columns">
				{#each columns as c (c.title)}
					{@const top = c.rows[0]?.credits || 1}
					<div class="column">
						<h3>{c.title}</h3>
						{#each c.rows.slice(0, TOP) as r (r.key)}
							<svelte:element this={r.href ? 'a' : 'div'} class="row" href={r.href}>
								<span class="label" title={r.label}>{r.label}</span>
								<span class="amount">{formatQueryCost(r)}</span>
								<span class="bar"><i style:width="{(r.credits / top) * 100}%"></i></span>
								<small>{r.detail}</small>
							</svelte:element>
						{/each}
						{#if c.rows.length > TOP}<small class="more">and {c.rows.length - TOP} more</small>{/if}
					</div>
				{/each}
			</div>
		{/if}
	</section>
{/if}

<style>
	.surface-panel { box-shadow: var(--panel-shadow); border: 1px solid var(--panel-border); border-radius: 2px; background: var(--card); }
	.panel-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 11px 14px; border-bottom: 1px solid var(--inner-line); }
	.panel-heading h2 { margin: 0; font-size: 13px; font-weight: 600; }
	.panel-heading span { color: var(--muted-foreground); font-size: 12px; }
	.panel-heading > div:first-child { display: flex; align-items: baseline; gap: 10px; }
	.windows { display: flex; border: 1px solid var(--border); border-radius: 2px; }
	.windows button { height: 24px; padding: 0 9px; color: var(--muted-foreground); font-size: 12px; }
	.windows button + button { border-left: 1px solid var(--border); }
	.windows button.on { background: var(--sb-hover); color: var(--foreground); font-weight: 600; }
	.total { display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 12px; padding: 12px 14px 4px; }
	.total strong { font-size: 20px; font-weight: 600; font-variant-numeric: tabular-nums; }
	.total span { color: var(--muted-foreground); font-size: 12px; }
	.total b { color: var(--sb-warning); font-weight: 500; }
	.total .hint { flex-basis: 100%; color: var(--sb-text-faint); }
	.columns { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 18px; padding: 8px 14px 14px; }
	.column h3 { margin: 6px 0 4px; color: var(--muted-foreground); font-size: 11px; font-weight: 600; letter-spacing: 0.03em; text-transform: uppercase; }
	.row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 2px 10px; padding: 6px 0; border-top: 1px solid var(--inner-line); font-size: 12.5px; }
	a.row:hover .label { color: var(--primary); text-decoration: underline; }
	.label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.amount { font-variant-numeric: tabular-nums; font-weight: 600; }
	.bar { height: 4px; border-radius: 2px; background: var(--inner-line); }
	.bar i { display: block; height: 100%; border-radius: 2px; background: var(--primary); }
	.row small, .more { color: var(--sb-text-faint); font-size: 11.5px; }
	.more { display: block; padding-top: 6px; }
	.note { padding: 14px; color: var(--muted-foreground); font-size: 12px; }
	.err { color: var(--sb-error); }
	@media (max-width: 900px) { .columns { grid-template-columns: 1fr; } }
</style>
