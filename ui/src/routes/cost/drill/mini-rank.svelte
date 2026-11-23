<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import type { CostPage } from '../types';
	import { costTitle, sharePct, usd, type DrillRow } from '../utils';

	// A short ranked list for a side column: name, cost and share of the level.
	let { cost, rows, total, name, hrefOf, count }: { cost: CostPage; rows: DrillRow[]; total: number; name: string; hrefOf?: (r: DrillRow) => string; count?: { label: string; value: (r: DrillRow) => number } } = $props();
	const columns = $derived<TableColumn[]>([
		{ id: 'name', label: name, minWidth: 150 },
		{ id: 'cost', label: 'Cost', width: '88px', align: 'right' },
		{ id: 'share', label: 'Share', width: '72px', align: 'right' },
		...(count ? [{ id: 'count', label: count.label, width: '78px', align: 'right' as const }] : [])
	]);
</script>

<DataTable class="compact static framed mini" embedded {columns} {rows} key={(r) => r.key} empty="No cost in this period.">
	{#snippet row(r)}
		<tr>
			<td>{#if hrefOf}<a class:mono={r.kind === 'model'} href={hrefOf(r)} title={r.key} data-sveltekit-noscroll>{r.label}</a>{:else}{r.label}{/if}{#if r.sub}<small>{r.sub}</small>{/if}</td>
			<td title={costTitle(r.credits, cost.price)}>{usd(r.credits, cost.price)}</td>
			<td class="muted">{sharePct(r.credits, total)}</td>
			{#if count}<td class="muted">{count.value(r).toLocaleString('en')}</td>{/if}
		</tr>
	{/snippet}
</DataTable>

<style>
	a:hover { color: var(--primary); text-decoration: underline; }
	.mono { font-family: var(--font-mono); font-size: 12px; }
	small { margin-left: 8px; color: var(--sb-text-faint); font-size: 11.5px; }
</style>
