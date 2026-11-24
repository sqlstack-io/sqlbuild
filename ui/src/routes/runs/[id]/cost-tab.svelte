<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Button from '$lib/components/ui/button/button.svelte';
	import type { RunCostBreakdown } from '$lib/api';
	import HelpTip from '$lib/components/help-tip.svelte';
	import { CREDIT_PRICE_HINT, costTitle } from '$lib/operate/cost';
	import { costSummary, formatQueryCost, formatSpan } from './utils';

	let { cost, filteredBy, onnode }: { cost: RunCostBreakdown; filteredBy: string[]; onnode?: (key: string) => void } = $props();

	const summary = $derived(costSummary(cost));
	const shown = $derived(filteredBy.length ? cost.resources.filter((r) => filteredBy.includes(r.nodeKey)) : cost.resources);
	const counted = $derived(cost.status === 'complete' || cost.status === 'partial');
	const share = (credits: number) => (cost.credits > 0 ? `${Math.round((credits / cost.credits) * 100)}%` : '');
	// The run's own statements, such as creating schemas, are a resource named after the target.
	const runStatements = (key: string) => key.startsWith('run:');
	const gb = (bytes: number) => (bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : bytes >= 1e6 ? `${(bytes / 1e6).toFixed(0)} MB` : bytes ? `${bytes.toLocaleString('en-GB')} B` : '—');
	const nodeColumns: TableColumn[] = [
		{ id: 'node', label: 'Node', minWidth: 200 },
		{ id: 'cost', label: 'Cost', width: '100px', align: 'right' },
		{ id: 'share', label: 'Share', width: '70px', align: 'right' },
		{ id: 'time', label: 'Warehouse time', width: '130px', align: 'right' },
		{ id: 'queries', label: 'Queries', width: '80px', align: 'right' },
		{ id: 'scanned', label: 'Scanned', width: '100px', align: 'right' },
		{ id: 'warehouse', label: 'Warehouse', width: '18%' }
	];
	const warehouseColumns: TableColumn[] = [
		{ id: 'name', label: 'Warehouse', minWidth: 180 },
		{ id: 'size', label: 'Size', width: '110px' },
		{ id: 'cost', label: 'Cost', width: '100px', align: 'right' },
		{ id: 'share', label: 'Share', width: '70px', align: 'right' },
		{ id: 'time', label: 'Warehouse time', width: '130px', align: 'right' },
		{ id: 'queries', label: 'Queries', width: '80px', align: 'right' }
	];
</script>

<div class="cost" data-testid="run-cost">
	<div class="total">
		<span class="label">Estimated query cost&nbsp;<HelpTip text="Estimated from the warehouse's query history: each query's share of its warehouse's credits. Idle time and the 60-second minimum are not included. Cost appears after the location syncs this run's query history from a Snowflake warehouse; other warehouses show duration only." /></span>
		<strong>{summary.text}</strong>
		{#if counted && cost.usd == null}<HelpTip text={CREDIT_PRICE_HINT} label="Why credits" />{/if}
		{#if summary.note}<span class="note">{summary.note}</span>{/if}
	</div>

	{#if counted && cost.resources.length}
		<h4>By node</h4>
		<DataTable class="static compact" embedded columns={nodeColumns} rows={shown} key={(r) => r.nodeKey} empty="No cost for the selected nodes.">
			{#snippet row(r)}
					<tr>
						<td class="mono">{#if runStatements(r.nodeKey)}<span class="faint" title="Statements outside any node, such as creating schemas">Run statements</span>{:else if onnode}<Button variant="link" size="inline" class="font-mono" onclick={() => onnode(r.nodeKey)}>{r.nodeKey}</Button>{:else}{r.nodeKey}{/if}</td>
						<td class="num" title={costTitle(r)}>{formatQueryCost(r)}</td>
						<td class="num faint">{share(r.credits)}</td>
						<td class="num faint">{formatSpan(r.seconds)}</td>
						<td class="num faint">{r.queryCount}</td>
						<td class="num faint">{gb(r.bytesScanned)}</td>
						<td class="mono faint">{r.warehouses.join(', ')}</td>
					</tr>
			{/snippet}
		</DataTable>

		<h4>By warehouse</h4>
		<DataTable class="static compact" embedded columns={warehouseColumns} rows={cost.warehouses} key={(w) => `${w.name}|${w.size}`} empty="No warehouse reported.">
			{#snippet row(w)}
					<tr>
						<td class="mono">{w.name}</td>
						<td class="faint">{w.size}</td>
						<td class="num" title={costTitle(w)}>{formatQueryCost(w)}</td>
						<td class="num faint">{share(w.credits)}</td>
						<td class="num faint">{formatSpan(w.seconds)}</td>
						<td class="num faint">{w.queryCount}</td>
					</tr>
			{/snippet}
		</DataTable>
	{/if}
</div>

<style>
	.cost {
		padding: 12px 16px 20px;
		font-size: 12.5px;
	}
	.total {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 4px 10px;
		margin-bottom: 14px;
	}
	.label {
		color: var(--muted-foreground);
	}
	.total strong {
		font-size: 18px;
		font-weight: 600;
		font-variant-numeric: tabular-nums;
	}
	.note {
		color: var(--sb-warn, var(--muted-foreground));
	}
	h4 {
		margin: 14px 0 6px;
		font-size: 12.5px;
		font-weight: 600;
	}
	.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.faint {
		color: var(--muted-foreground);
	}
</style>
