<script lang="ts">
	import type { RunCostBreakdown } from '$lib/api';
	import { CREDIT_PRICE_HINT } from '$lib/operate/cost';
	import { costSummary, formatQueryCost, formatSpan } from './utils';

	let { cost, filteredBy, onnode }: { cost: RunCostBreakdown; filteredBy: string[]; onnode?: (key: string) => void } = $props();

	const summary = $derived(costSummary(cost));
	const shown = $derived(filteredBy.length ? cost.resources.filter((r) => filteredBy.includes(r.nodeKey)) : cost.resources);
	const counted = $derived(cost.status === 'complete' || cost.status === 'partial');
	const share = (credits: number) => (cost.credits > 0 ? `${Math.round((credits / cost.credits) * 100)}%` : '');
	// The run's own statements, such as creating schemas, are a resource named after the target.
	const runStatements = (key: string) => key.startsWith('run:');
	const gb = (bytes: number) => (bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : bytes >= 1e6 ? `${(bytes / 1e6).toFixed(0)} MB` : bytes ? `${bytes.toLocaleString('en-GB')} B` : '—');
</script>

<div class="cost" data-testid="run-cost">
	<div class="total">
		<span class="label">Estimated query cost</span>
		<strong>{summary.text}</strong>
		{#if summary.note}<span class="note">{summary.note}</span>{/if}
		{#if counted && cost.usd == null}
			<span class="hint">{CREDIT_PRICE_HINT}</span>
		{/if}
	</div>

	{#if counted && cost.resources.length}
		<h4>By node <span>most expensive first</span></h4>
		<table>
			<thead>
				<tr><th>Node</th><th class="num">Cost</th><th class="num">Share</th><th class="num">Warehouse time</th><th class="num">Queries</th><th class="num">Scanned</th><th>Warehouse</th></tr>
			</thead>
			<tbody>
				{#each shown as r (r.nodeKey)}
					<tr>
						<td class="mono">{#if runStatements(r.nodeKey)}<span class="faint" title="Statements outside any node, such as creating schemas">Run statements</span>{:else if onnode}<button class="node" onclick={() => onnode(r.nodeKey)}>{r.nodeKey}</button>{:else}{r.nodeKey}{/if}</td>
						<td class="num">{formatQueryCost(r)}</td>
						<td class="num faint">{share(r.credits)}</td>
						<td class="num faint">{formatSpan(r.seconds)}</td>
						<td class="num faint">{r.queryCount}</td>
						<td class="num faint">{gb(r.bytesScanned)}</td>
						<td class="mono faint">{r.warehouses.join(', ')}</td>
					</tr>
				{:else}
					<tr><td colspan="7" class="faint">No cost for the selected nodes.</td></tr>
				{/each}
			</tbody>
		</table>

		<h4>By warehouse</h4>
		<table>
			<thead>
				<tr><th>Warehouse</th><th>Size</th><th class="num">Cost</th><th class="num">Share</th><th class="num">Warehouse time</th><th class="num">Queries</th></tr>
			</thead>
			<tbody>
				{#each cost.warehouses as w (`${w.name}|${w.size}`)}
					<tr>
						<td class="mono">{w.name}</td>
						<td class="faint">{w.size}</td>
						<td class="num">{formatQueryCost(w)}</td>
						<td class="num faint">{share(w.credits)}</td>
						<td class="num faint">{formatSpan(w.seconds)}</td>
						<td class="num faint">{w.queryCount}</td>
					</tr>
				{/each}
			</tbody>
		</table>
		<p class="foot">Estimated from the warehouse's query history: each query's share of its warehouse's credits. Idle time and the 60-second minimum are not included.</p>
	{:else if cost.status === 'none'}
		<p class="foot">Cost appears after the location syncs this run's query history from a Snowflake warehouse. Other warehouses show duration only.</p>
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
	.hint,
	.foot {
		color: var(--muted-foreground);
	}
	.hint {
		flex-basis: 100%;
		font-size: 12px;
	}
	.foot {
		margin-top: 10px;
		font-size: 12px;
	}
	h4 {
		margin: 14px 0 6px;
		font-size: 12.5px;
		font-weight: 600;
	}
	h4 span {
		margin-left: 6px;
		color: var(--sb-text-faint);
		font-weight: 400;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 12px;
	}
	th {
		padding: 7px 12px;
		border-bottom: 1px solid var(--border);
		color: var(--sb-text-faint);
		font-size: 11.5px;
		font-weight: 500;
		text-align: left;
	}
	td {
		padding: 6px 12px;
		border-bottom: 1px solid var(--border-subtle);
	}
	.num {
		text-align: right;
		font-variant-numeric: tabular-nums;
	}
	.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.faint {
		color: var(--muted-foreground);
	}
	.node {
		color: var(--foreground);
		text-decoration: underline dotted;
		text-underline-offset: 2px;
	}
</style>
