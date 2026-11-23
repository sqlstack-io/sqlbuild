<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Panel from '$lib/components/platform/panel.svelte';
	import { to } from '$lib/paths';
	import CostHead from './cost-head.svelte';
	import RankPanel from './rank-panel.svelte';
	import RunsPanel from './runs-panel.svelte';
	import SparkLine from './spark-line.svelte';
	import type { CostPage } from './types';
	import { change, costTitle, dayLabel, seriesOf, usd } from './utils';

	// D: what changed. Totals against the period before; jobs with their daily trend; the models
	// whose cost rose most; then sizes, folders and the most expensive runs.
	let { cost }: { cost: CostPage } = $props();
	let allJobs = $state(false);
	const jobs = $derived(allJobs ? cost.jobs : cost.jobs.slice(0, 12));
	const risers = $derived(
		cost.models
			.filter((m) => m.previous > 0 && m.credits > m.previous)
			.sort((a, b) => b.credits - b.previous - (a.credits - a.previous))
			.slice(0, 8)
	);
	const jobColumns: TableColumn[] = [
		{ id: 'job', label: 'Job', minWidth: 160 },
		{ id: 'cost', label: 'Cost', width: '92px', align: 'right' },
		{ id: 'trend', label: 'Per day', width: '124px', align: 'right' },
		{ id: 'runs', label: 'Runs', width: '72px', align: 'right' },
		{ id: 'change', label: 'Change', width: '76px', align: 'right', title: 'Against the period before' }
	];
	const riseColumns: TableColumn[] = [
		{ id: 'model', label: 'Model', minWidth: 140 },
		{ id: 'now', label: 'Cost', width: '92px', align: 'right' },
		{ id: 'before', label: 'Before', width: '92px', align: 'right', title: 'The period before' },
		{ id: 'rise', label: 'Increase', width: '96px', align: 'right' }
	];
	const span = $derived(cost.daily.length ? `${dayLabel(cost.daily[0].date, true)} – ${dayLabel(cost.daily.at(-1)!.date, true)}` : '');
	const jobHref = (k: string) => (k === 'Manual or CLI' ? undefined : to(`/jobs/${k}`));
</script>

<CostHead {cost} compare />
<div class="grid">
	<div>
		<Panel title="By job" note={span}>
			<DataTable class="compact framed static" embedded columns={jobColumns} rows={jobs} key={(r) => r.key} empty="No cost in this period.">
				{#snippet row(r)}
					{@const c = change(r.credits, r.previous)}
					{@const link = jobHref(r.key)}
					<tr>
						<td>{#if link}<a href={link} title={r.key}>{r.key}</a>{:else}<span title={r.key}>{r.key}</span>{/if}</td>
						<td title={costTitle(r.credits, cost.price)}>{usd(r.credits, cost.price)}</td>
						<td><SparkLine values={seriesOf(cost.daily, r.key)} title="Cost per day of {r.key}" /></td>
						<td class="muted">{r.runs.toLocaleString('en')}</td>
						<td class="change {c?.tone ?? ''}">{c?.text ?? '—'}</td>
					</tr>
				{/snippet}
			</DataTable>
			{#if cost.jobs.length > 12}<button class="more" onclick={() => (allJobs = !allJobs)}>{allJobs ? 'Show fewer' : `Show all ${cost.jobs.length}`}</button>{/if}
		</Panel>
		<RunsPanel runs={cost.runs} price={cost.price} limit={6} />
	</div>
	<div>
		<Panel title="Largest increases" help="Models whose cost rose most against the period before">
			<DataTable class="compact framed static" embedded columns={riseColumns} rows={risers} key={(r) => r.key} empty="No model costs more than before.">
				{#snippet row(r)}
					<tr>
						<td><a href={to(`/catalog/${r.key}`)} title="{r.key} · {r.job}">{r.key}</a><span class="sub">{r.folder}</span></td>
						<td title={costTitle(r.credits, cost.price)}>{usd(r.credits, cost.price)}</td>
						<td class="muted" title={costTitle(r.previous, cost.price)}>{usd(r.previous, cost.price)}</td>
						<td class="change up">+{usd(r.credits - r.previous, cost.price)}</td>
					</tr>
				{/snippet}
			</DataTable>
		</Panel>
		<RankPanel title="By warehouse size" rows={cost.sizes} total={cost.credits} price={cost.price} name="Size" count={{ label: 'Runs', value: (r) => r.runs }} showChange />
		<RankPanel title="By folder" rows={cost.folders} total={cost.credits} price={cost.price} name="Folder" count={{ label: 'Models', value: (r) => r.nodes }} limit={6} showChange />
	</div>
</div>

<style>
	.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 560px), 760px)); gap: 0 32px; }
	.sub { margin-left: 8px; color: var(--sb-text-faint); font-size: 11.5px; }
	.change.up { color: var(--sb-warning); }
	.change.down { color: var(--sb-success); }
	.change.flat { color: var(--sb-text-faint); }
	.more { margin-top: 6px; padding: 2px 0; color: var(--muted-foreground); font-size: 12px; }
	.more:hover { color: var(--primary); text-decoration: underline; }
</style>
