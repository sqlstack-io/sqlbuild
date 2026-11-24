<script lang="ts">
	import type { CostPrice, NodeBuildCost } from '$lib/api';
	import Button from '$lib/components/ui/button/button.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Panel from '$lib/components/platform/panel.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { to } from '$lib/paths';
	import { costTitle, hours, money } from '../utils/format';
	import { MANUAL_LABEL, nameOf, type DrillRow, type TreeRow } from '../utils/tree';
	import DrillTable from './drill-table.svelte';
	import TrendArea from './trend-area.svelte';

	// A model in the drawer: its cost per day, the jobs that spent it, and its cost in each build of
	// the period. Per-statement cost isn't stored, so there is no statement breakdown.
	let {
		nodeKey,
		job,
		days,
		dates,
		price,
		daily,
		jobs,
		jobHref
	}: { nodeKey: string; job: string | null; days: number; dates: string[]; price: CostPrice | null; daily: readonly number[] | null; jobs: DrillRow[]; jobHref: (row: DrillRow) => string | null } = $props();

	const total = $derived(jobs.reduce((t, r) => t + r.credits, 0));
	const jobRows = $derived<TreeRow[]>(jobs.map((row) => ({ row, depth: 0, id: row.key, open: false, parent: total, peak: jobs[0]?.credits ?? 0 })));
	const runs = createResource<NodeBuildCost[]>((client, scope) => client.listNodeCosts(scope, nodeKey, days), ['runs', 'warehouse_sync']);
	const sorted = $derived([...(runs.data ?? [])].sort((a, b) => b.credits - a.credits));
	let allRuns = $state(false);
	const runColumns: TableColumn[] = [
		{ id: 'run', label: 'Run', width: '120px' },
		{ id: 'cost', label: 'Cost', width: '92px', align: 'right' },
		{ id: 'time', label: 'Time', width: '64px', align: 'right', title: 'Warehouse time the queries used' },
		{ id: 'started', label: 'Started', minWidth: 90 }
	];
</script>

<div class="detail">
	<Panel title="Cost per day">
		{#if daily}<TrendArea {dates} values={[...daily]} {price} height={44} />{:else}<div class="loading-trend"></div>{/if}
	</Panel>
	<Panel title="Jobs" note={jobs.length === 1 ? 'one job' : `${jobs.length} jobs`} help="The jobs whose runs built this model, and the part of its cost each spent">
		<DrillTable {price} rows={jobRows} name="Job" hrefOf={(r) => jobHref(r)} embedded selected={job} narrow />
	</Panel>
	<Panel title="Runs" note={runs.data ? `${runs.data.length.toLocaleString('en')} in ${days} days` : undefined} help="The model's cost in each build, most expensive first">
		<DataTable class="compact static framed" embedded columns={runColumns} rows={allRuns ? sorted.slice(0, 100) : sorted.slice(0, 8)} key={(r) => r.buildRunId} resource={runs} empty="No runs in this period.">
			{#snippet row(r)}
				<tr>
					<td>{#if r.runId}<a class="code" href={to(`/runs/${r.runId}`)}>{r.runId.slice(0, 8)}</a>{:else}<span class="muted" title={r.buildRunId}>{MANUAL_LABEL}</span>{/if}</td>
					<td title={costTitle(r.credits, price)}>{money(r.credits, price)}</td>
					<td class="muted">{hours(r.seconds)}</td>
					<td><RelativeTime value={formatAgo(r.startedAt, app.now)} exact={formatStamp(r.startedAt)} /></td>
				</tr>
			{/snippet}
		</DataTable>
		{#if sorted.length > 8}<span class="more-row"><Button variant="ghost" size="inline" onclick={() => (allRuns = !allRuns)}>{allRuns ? 'Show fewer' : 'Show top 100'}</Button></span>{/if}
	</Panel>
	<a class="catalog" href={to(`/catalog/${encodeURIComponent(nameOf(nodeKey))}`)}>Open in the catalog →</a>
</div>

<style>
	.detail { min-width: 0; }
	.loading-trend { height: 44px; border-bottom: 1px dashed var(--inner-line); }
	.more-row { display: block; margin-top: 6px; padding: 2px 0; color: var(--muted-foreground); font-size: 12px; }
	.catalog { font-size: 12.5px; }
	.catalog:hover { color: var(--primary); text-decoration: underline; }
</style>
