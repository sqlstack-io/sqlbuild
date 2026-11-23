<script lang="ts">
	import { plural } from '$lib/format/plural';
	import HelpTip from '$lib/components/help-tip.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import { to } from '$lib/paths';
	import { ApiError, DEMO } from '$lib/api';
	import { app } from '$lib/state/app.svelte';
	import SyncingNote from '$lib/components/platform/syncing-note.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { useCatalogNodes } from '$lib/state/catalog.svelte';
	import { folderCatalogHref, folderSelections } from '$lib/selection/folders';
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
		return cost;
	}, ['runs', 'warehouse_sync', 'snapshot']);
	const catalog = useCatalogNodes();
	const folderSelection = $derived(folderSelections(catalog.data ?? []));
	const data = $derived(summary.data && catalog.data ? { cost: summary.data, folders: costByFolder(summary.data.byNode, catalog.data) } : null);
	const cost = $derived(data?.cost ?? null);
	type Row = { key: string; label: string; title?: string; credits: number; usd: number | null; detail: string; href?: string };
	const columns = $derived<{ title: string; rows: Row[] }[]>(
		cost && data
			? [
					{
						title: 'By job',
						rows: cost.byJob.map((g) => ({ key: g.key ?? '', label: g.key ?? 'Manual or CLI', title: g.key ? undefined : 'Runs started by hand or from the command line', credits: g.credits, usd: g.usd, detail: plural(g.runs, 'run'), href: g.key ? to(`/jobs/${g.key}`) : undefined }))
					},
					{
						title: 'By folder',
						rows: data.folders.map((f) => ({ key: f.key, label: f.key, credits: f.credits, usd: f.usd, detail: plural(f.nodes, 'node'), href: f.key === 'other' || !folderSelection.has(f.key) ? undefined : to(folderCatalogHref(folderSelection.get(f.key)!)) }))
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
	<section class="surface-panel cost" id="cost">
		<div class="panel-heading">
			<div>
				<h2>Estimated query cost <HelpTip text="Warehouse time the runs' queries used, not billed warehouse time" /></h2>
			</div>
			<SegmentedFilter label="Window" value={String(days)} options={WINDOWS.map((w) => ({ value: String(w), label: `${w} days` }))} onselect={(v) => (days = Number(v) as (typeof WINDOWS)[number])} />
		</div>
		<SyncingNote what="Costs" />
		{#if summary.error && !cost}
			<p class="note err">Couldn't load cost: {summary.error}</p>
		{:else if cost}
			<div class="total">
				<strong>{formatQueryCost(cost)}{#if cost.usd === null}<HelpTip text={CREDIT_PRICE_HINT} label="Why credits" />{/if}</strong>
				<span>{plural(cost.runs, 'run')} in the last {days} days{#if cost.incompleteRuns}&nbsp;· <b title="Cost still pending or partial for these runs">{cost.incompleteRuns} incomplete</b>{/if}</span>
			</div>
			<div class="columns">
				{#each columns as c (c.title)}
					{@const top = c.rows[0]?.credits || 1}
					<div class="column">
						<h3>{c.title}</h3>
						{#each c.rows.slice(0, TOP) as r (r.key)}
							<svelte:element this={r.href ? 'a' : 'div'} class="row" href={r.href}>
								<span class="label" title={r.title ?? r.label}>{r.label}</span>
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
	.panel-heading > div:first-child { display: flex; align-items: baseline; gap: 10px; }
	.total { display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 12px; padding: 12px 14px 4px; }
	.total strong { font-size: 20px; font-weight: 600; font-variant-numeric: tabular-nums; }
	.total span { color: var(--muted-foreground); font-size: 12px; }
	.total b { color: var(--sb-warning); font-weight: 500; }
	.total strong :global(.help-tip) { margin-left: 6px; }
	.cost { scroll-margin-top: 12px; }
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
