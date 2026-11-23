<script lang="ts">
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import { plural } from '$lib/format/plural';
	import { to } from '$lib/paths';
	import RankPanel from './rank-panel.svelte';
	import RunsPanel from './runs-panel.svelte';
	import type { CostPage } from './types';
	import { bytes, change, costTitle, creditsText, hours, rollUp, runsByJob, sharePct, usd, type ExploreRow, type GroupBy } from './utils';

	// What one row of the explore table is made of: a job's runs and models, a folder's models and
	// jobs, a size's jobs, or a model's or run's own figures.
	let { cost, by, row }: { cost: CostPage; by: GroupBy; row: ExploreRow } = $props();
	const p = $derived(cost.price);
	const c = $derived(change(row.credits, row.previous));
	const model = $derived(by === 'model' ? cost.models.find((m) => m.key === row.key) : undefined);
	const run = $derived(by === 'run' ? cost.runs.find((r) => r.id === row.key) : undefined);
	const jobRuns = $derived(by === 'job' ? cost.runs.filter((r) => (r.job ?? 'Manual or CLI') === row.key) : []);
	const jobModels = $derived(by === 'job' ? cost.models.filter((m) => m.job === row.key) : []);
	const folderModels = $derived(by === 'folder' ? cost.models.filter((m) => m.folder === row.key) : []);
	const sizeRuns = $derived(by === 'size' ? cost.runs.filter((r) => r.size === row.key) : []);
	const href = $derived(by === 'job' && row.key !== 'Manual or CLI' ? to(`/jobs/${row.key}`) : by === 'model' ? to(`/catalog/${row.key}`) : by === 'run' ? to(`/runs/${row.key}`) : by === 'folder' ? to(`/catalog?folder=${row.key}`) : null);
	const facts = $derived([
		{ label: 'Cost', value: usd(row.credits, p), title: costTitle(row.credits, p) },
		{ label: 'Credits', value: creditsText(row.credits) },
		{ label: 'Share', value: sharePct(row.credits, cost.credits) },
		...(by !== 'run' ? [{ label: 'Change', value: c?.text ?? '—', title: `${usd(row.previous, p)} in the ${cost.days} days before` }] : []),
		{ label: 'Warehouse time', value: hours(row.seconds) },
		...(by === 'job' || by === 'size' ? [{ label: 'Runs', value: `${row.runs.toLocaleString('en')} · ${usd(row.credits / Math.max(1, row.runs), p)} each` }] : []),
		...(model ? [{ label: 'Folder', value: model.folder }, { label: 'Job', value: model.job }, { label: 'Builds', value: model.runs.toLocaleString('en') }, { label: 'Queries', value: model.queries.toLocaleString('en') }, { label: 'Scanned', value: bytes(model.bytesScanned) }] : []),
		...(run ? [{ label: 'Job', value: run.job ?? 'Manual or CLI' }, { label: 'Warehouse size', value: run.size }, { label: 'Models', value: run.nodes.toLocaleString('en') }] : [])
	]);
</script>

<aside class="detail">
	<header>
		<h2 title={row.key}>{row.key}</h2>
		{#if href}<a href={href}>Open →</a>{/if}
	</header>
	<div class="facts"><KeyValues rows={facts} /></div>
	{#if by === 'job'}
		<RunsPanel runs={jobRuns} price={p} title="Runs" limit={6} showJob={false} />
		<RankPanel title="Models" rows={jobModels} total={row.credits} price={p} name="Model" count={{ label: 'Builds', value: (r) => r.runs }} limit={6} />
	{:else if by === 'folder'}
		<RankPanel title="Jobs" rows={rollUp(folderModels, (m) => m.job)} total={row.credits} price={p} name="Job" count={{ label: 'Models', value: (r) => r.nodes }} limit={6} />
		<RankPanel title="Models" rows={folderModels} total={row.credits} price={p} name="Model" count={{ label: 'Builds', value: (r) => r.runs }} limit={6} />
	{:else if by === 'size'}
		<RankPanel title="Jobs" rows={runsByJob(sizeRuns)} total={row.credits} price={p} name="Job" count={{ label: 'Runs', value: (r) => r.runs }} limit={8} />
	{:else if by === 'model' && model}
		<p class="muted">{plural(model.runs, 'build')} by {model.job}</p>
	{/if}
</aside>

<style>
	.detail { max-width: 620px; position: sticky; top: 0; min-width: 0; padding: 12px 16px 4px; border: 1px solid var(--panel-border); border-radius: 2px; background: var(--card); box-shadow: var(--panel-shadow); }
	header { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; margin-bottom: 12px; }
	h2 { overflow: hidden; font-size: 14px; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
	header a { flex: none; font-size: 12.5px; }
	.facts { margin-bottom: 18px; }
	.muted { margin-bottom: 14px; color: var(--muted-foreground); font-size: 12.5px; }
</style>
