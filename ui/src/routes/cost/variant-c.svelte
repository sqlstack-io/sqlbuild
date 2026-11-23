<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { ofTotal } from '$lib/components/list/table-columns';
	import CostHead from './cost-head.svelte';
	import ExploreDetail from './explore-detail.svelte';
	import type { CostPage } from './types';
	import { change, costTitle, exploreRows, sharePct, sortRows, usd, type GroupBy, type SortBy } from './utils';

	// C: one table, grouped and sorted at will; picking a row shows what it is made of beside it.
	let { cost }: { cost: CostPage } = $props();
	let by = $state<GroupBy>('job');
	let sort = $state<SortBy | ''>('');
	let query = $state('');
	let picked = $state<string | null>(null);
	const all = $derived(exploreRows(cost, by));
	const rows = $derived.by(() => {
		const q = query.trim().toLowerCase();
		return sortRows(q ? all.filter((r) => `${r.key} ${r.sub ?? ''}`.toLowerCase().includes(q)) : all, sort || 'cost');
	});
	const selected = $derived(rows.find((r) => r.key === picked) ?? rows[0]);
	const top = $derived(Math.max(...rows.map((r) => r.credits), 0) || 1);
	const NAMES: Record<GroupBy, string> = { job: 'Job', folder: 'Folder', model: 'Model', size: 'Warehouse size', run: 'Run' };
	const columns = $derived<TableColumn[]>([
		{ id: 'name', label: NAMES[by], minWidth: 170 },
		{ id: 'cost', label: 'Cost', width: '92px', align: 'right' },
		{ id: 'share', label: 'Share', width: '116px', align: 'right' },
		{ id: 'each', label: by === 'model' ? 'Per build' : 'Per run', width: '92px', align: 'right' },
		{ id: 'runs', label: by === 'folder' ? 'Models' : by === 'model' ? 'Builds' : by === 'run' ? 'Models' : 'Runs', width: '78px', align: 'right' },
		...(by === 'run' ? [] : [{ id: 'change', label: 'Change', width: '76px', align: 'right' as const, title: 'Against the period before' }])
	]);
	const count = (r: (typeof rows)[number]) => (by === 'folder' || by === 'run' ? r.nodes : r.runs);
</script>

<CostHead {cost} />
<div class="toolbar">
	<ListToolbar bind:query placeholder="Search {NAMES[by].toLowerCase()}s" total={ofTotal(rows.length, all.length)}>
		{#snippet filters()}
			<SegmentedFilter label="Group by" bind:value={by} onselect={() => (picked = null)} options={[{ value: 'job', label: 'Job', count: cost.jobs.length }, { value: 'folder', label: 'Folder', count: cost.folders.length }, { value: 'model', label: 'Model', count: cost.models.length }, { value: 'size', label: 'Size', count: cost.sizes.length }, { value: 'run', label: 'Run', count: cost.runCount }]} />
			<ChoiceFilter label="Sort" bind:value={sort} all="Cost" options={[{ value: 'change', label: 'Increase' }, { value: 'runs', label: 'Runs' }, { value: 'perRun', label: 'Per run' }]} />
		{/snippet}
	</ListToolbar>
</div>
<div class="split">
	<div class="list">
		<DataTable class="compact" embedded {columns} {rows} key={(r) => r.key} rowHeight={33} empty="No cost matches the search.">
			{#snippet row(r)}
				{@const c = change(r.credits, r.previous)}
				<tr class:selected={selected?.key === r.key} onclick={() => (picked = r.key)}>
					<td><span title={r.key}>{r.key}</span>{#if r.sub}<span class="sub">{r.sub}</span>{/if}</td>
					<td title={costTitle(r.credits, cost.price)}>{usd(r.credits, cost.price)}</td>
					<td><span class="share"><span class="bar"><i style:width="{(r.credits / top) * 100}%"></i></span><span class="pct">{sharePct(r.credits, cost.credits)}</span></span></td>
					<td class="muted">{usd(r.credits / Math.max(1, r.runs), cost.price)}</td>
					<td class="muted">{count(r).toLocaleString('en')}</td>
					{#if by !== 'run'}<td class="change {c?.tone ?? ''}">{c?.text ?? '—'}</td>{/if}
				</tr>
			{/snippet}
		</DataTable>
	</div>
	{#if selected}<ExploreDetail {cost} {by} row={selected} />{/if}
</div>

<style>
	.toolbar { margin: 0 -24px 12px; border-top: 1px solid var(--border); }
	.split { display: grid; grid-template-columns: minmax(590px, 860px) minmax(340px, 1fr); align-items: start; gap: 24px; }
	.list { min-width: 0; border: 1px solid var(--panel-border); border-radius: 2px; background: var(--card); }
	.sub { margin-left: 8px; color: var(--sb-text-faint); font-size: 11.5px; }
	.share { display: inline-flex; align-items: center; justify-content: flex-end; gap: 8px; }
	.bar { width: 40px; height: 4px; border-radius: 2px; background: var(--inner-line); }
	.bar i { display: block; height: 100%; border-radius: 2px; background: var(--primary); }
	.pct { width: 34px; color: var(--muted-foreground); }
	.change.up { color: var(--sb-warning); }
	.change.down { color: var(--sb-success); }
	.change.flat { color: var(--sb-text-faint); }
	tr.selected { background: color-mix(in srgb, var(--primary) 12%, transparent); }
	tr.selected td:first-child { box-shadow: inset 2px 0 0 var(--primary); }
</style>
