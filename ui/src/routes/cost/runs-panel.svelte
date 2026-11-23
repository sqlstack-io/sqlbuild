<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import { to } from '$lib/paths';
	import type { CostRun, CreditPrice } from './types';
	import { costTitle, hours, usd } from './utils';

	// The most expensive runs of the period, with their job, size and warehouse time.
	let { runs, price, title = 'Most expensive runs', limit = 8, showJob = true }: { runs: CostRun[]; price: CreditPrice; title?: string; limit?: number; showJob?: boolean } = $props();
	let all = $state(false);
	const shown = $derived(all ? runs.slice(0, 100) : runs.slice(0, limit));
	const columns = $derived<TableColumn[]>([
		{ id: 'run', label: showJob ? 'Job' : 'Run', minWidth: 120 },
		{ id: 'cost', label: 'Cost', width: '92px', align: 'right' },
		{ id: 'time', label: 'Time', width: '68px', align: 'right', title: 'Warehouse time the queries used' },
		{ id: 'size', label: 'Size', width: '76px' },
		{ id: 'started', label: 'Started', width: '92px' }
	]);
</script>

<Panel {title} note="of {runs.length.toLocaleString('en')}">
	<DataTable class="compact framed" embedded {columns} rows={shown} key={(r) => r.id} empty="No runs in this period.">
		{#snippet row(r)}
			<tr>
				<td><span class="run"><i style:background={runStatusTone[r.status]} title={runStatusLabel[r.status]}></i><a href={to(`/runs/${r.id}`)} title="{r.job ?? 'Manual or CLI'} · {runStatusLabel[r.status]}" class:code={!showJob}>{showJob ? (r.job ?? 'Manual or CLI') : r.id.slice(0, 8)}</a></span></td>
				<td title={costTitle(r.credits, price)}>{usd(r.credits, price)}</td>
				<td class="muted">{hours(r.seconds)}</td>
				<td class="muted">{r.size}</td>
				<td><RelativeTime value={formatAgo(r.startedAt, app.now)} exact={formatStamp(r.startedAt)} /></td>
			</tr>
		{/snippet}
	</DataTable>
	{#if runs.length > limit}
		<span class="more-row"><Button variant="ghost" size="inline" onclick={() => (all = !all)}>{all ? 'Show fewer' : 'Show top 100'}</Button></span>
	{/if}
</Panel>

<style>
	.run {
		display: flex;
		min-width: 0;
		align-items: center;
		gap: 8px;
	}
	.run a {
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.run i {
		width: 7px;
		height: 7px;
		flex: none;
		border-radius: 50%;
	}
	.more-row {
		display: block;
		margin-top: 6px;
		padding: 2px 0;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.more-row :global(button:hover) {
		color: var(--primary);
		text-decoration: underline;
	}
</style>
