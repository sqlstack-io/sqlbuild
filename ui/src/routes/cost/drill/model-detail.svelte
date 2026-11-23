<script lang="ts">
	import { page } from '$app/state';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Panel from '$lib/components/platform/panel.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import { MANUAL, mockStatements } from '$lib/api/mock/cost-page';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import { app } from '$lib/state/app.svelte';
	import { to } from '$lib/paths';
	import type { CostPage } from '../types';
	import { costTitle, dailyOf, factsIn, hours, runsIn, scopeParams, sharePct, sizesIn, total, type DrillRow, type Scope } from '../utils';
	import { usd } from '../utils';
	import DrillTable from './drill-table.svelte';
	import TrendBars from './trend-bars.svelte';

	// A model, the explorer's deepest level: which jobs spent its cost, its runs over the period, the
	// warehouse sizes, and its statements. `narrow` stacks it for a drawer.
	let { cost, scope, narrow = false }: { cost: CostPage; scope: Scope; narrow?: boolean } = $props();
	const facts = $derived(factsIn(cost, { ...scope, job: null, folder: null, flat: false }));
	const sum = $derived(total(facts));
	const jobs = $derived(
		[...new Set(facts.map((f) => f.job))]
			.map((job) => ({ kind: 'job' as const, key: job, label: job, deeper: false, ...total(facts.filter((f) => f.job === job)) }))
			.sort((a, b) => b.credits - a.credits)
			.map((row) => ({ row, depth: 0, parent: sum.credits, id: row.key, open: false }))
	);
	const runs = $derived(runsIn(cost, facts, MANUAL));
	let allRuns = $state(false);
	const sizes = $derived(sizesIn(cost, facts));
	const statements = $derived(mockStatements(scope.model ?? '', sum.credits, sum.seconds, sum.queries));
	const jobHref = (r: DrillRow) => `?${scopeParams(page.url.searchParams, { ...scope, by: 'job', job: r.key, flat: false })}`;
	const runColumns: TableColumn[] = [
		{ id: 'run', label: 'Run', width: '110px' },
		{ id: 'job', label: 'Job', minWidth: 120 },
		{ id: 'cost', label: 'Cost', width: '84px', align: 'right' },
		{ id: 'time', label: 'Time', width: '64px', align: 'right', title: 'Warehouse time the queries used' },
		{ id: 'started', label: 'Started', width: '84px' }
	];
	const sizeColumns: TableColumn[] = [
		{ id: 'size', label: 'Size', minWidth: 90 },
		{ id: 'cost', label: 'Cost', width: '84px', align: 'right' },
		{ id: 'share', label: 'Share', width: '64px', align: 'right' },
		{ id: 'runs', label: 'Runs', width: '64px', align: 'right' }
	];
	const statementColumns: TableColumn[] = [
		{ id: 'text', label: 'Statement', minWidth: 160 },
		{ id: 'cost', label: 'Cost', width: '84px', align: 'right' },
		{ id: 'share', label: 'Share', width: '64px', align: 'right' },
		{ id: 'queries', label: 'Queries', width: '72px', align: 'right' }
	];
</script>

<div class="detail" class:narrow>
	<div class="col">
		{#if narrow}
			<Panel title="Cost per day">
				<TrendBars {cost} values={dailyOf(cost, facts)} height={44} />
			</Panel>
		{/if}
		<Panel title="Jobs" note={jobs.length === 1 ? 'one job' : `${jobs.length} jobs`} help="The jobs whose runs built this model, and the part of its cost each spent">
			<DrillTable {cost} rows={jobs} name="Job" hrefOf={jobHref} embedded selected={scope.job} />
		</Panel>
		<Panel title="Runs" note="of {runs.length.toLocaleString('en')}">
			<DataTable class="compact static framed" embedded columns={runColumns} rows={allRuns ? runs.slice(0, 100) : runs.slice(0, 8)} key={(r) => r.id} empty="No runs in this period.">
				{#snippet row(r)}
					<tr>
						<td><span class="run"><i style:background={runStatusTone[r.status]} title={runStatusLabel[r.status]}></i><a class="code" href={to(`/runs/${r.id}`)}>{r.id}</a></span></td>
						<td class="muted">{r.job ?? MANUAL}</td>
						<td title={costTitle(r.credits, cost.price)}>{usd(r.credits, cost.price)}</td>
						<td class="muted">{hours(r.seconds)}</td>
						<td><RelativeTime value={formatAgo(r.startedAt, app.now)} exact={formatStamp(r.startedAt)} /></td>
					</tr>
				{/snippet}
			</DataTable>
			{#if runs.length > 8}<button class="more" onclick={() => (allRuns = !allRuns)}>{allRuns ? 'Show fewer' : 'Show top 100'}</button>{/if}
		</Panel>
	</div>
	<div class="col">
		<Panel title="Warehouse sizes">
			<DataTable class="compact static framed" embedded columns={sizeColumns} rows={sizes} key={(r) => r.key} empty="No cost in this period.">
				{#snippet row(r)}
					<tr>
						<td>{r.key}</td>
						<td title={costTitle(r.credits, cost.price)}>{usd(r.credits, cost.price)}</td>
						<td class="muted">{sharePct(r.credits, sum.credits)}</td>
						<td class="muted">{r.runs.toLocaleString('en')}</td>
					</tr>
				{/snippet}
			</DataTable>
		</Panel>
		<Panel title="Statements" note="mock" help="Needs API work: the cost read has no per-statement breakdown yet. Shows the model's statements by cost.">
			<DataTable class="compact static framed" embedded columns={statementColumns} rows={statements} key={(s) => s.text} empty="No statements recorded.">
				{#snippet row(s)}
					<tr>
						<td><code title={s.text}>{s.text}</code></td>
						<td title={costTitle(s.credits, cost.price)}>{usd(s.credits, cost.price)}</td>
						<td class="muted">{sharePct(s.credits, sum.credits)}</td>
						<td class="muted">{s.queries.toLocaleString('en')}</td>
					</tr>
				{/snippet}
			</DataTable>
		</Panel>
		<a class="catalog" href={to(`/catalog/${scope.model}`)}>Open in the catalog →</a>
	</div>
</div>

<style>
	.detail { display: grid; grid-template-columns: minmax(0, 3fr) minmax(0, 2fr); gap: 0 28px; }
	.detail.narrow { grid-template-columns: minmax(0, 1fr); }
	/* Two columns only where both fit; the explorer's pane is a size container. */
	@container (max-width: 1100px) { .detail { grid-template-columns: minmax(0, 1fr); } }
	.col { min-width: 0; }
	.run { display: flex; min-width: 0; align-items: center; gap: 8px; }
	.run i { width: 7px; height: 7px; flex: none; border-radius: 50%; }
	.more { margin-top: 6px; padding: 2px 0; color: var(--muted-foreground); font-size: 12px; }
	.more:hover, .catalog:hover { color: var(--primary); text-decoration: underline; }
	.catalog { font-size: 12.5px; }
</style>
