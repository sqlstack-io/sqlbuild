<script lang="ts">
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import FolderOpenIcon from '@lucide/svelte/icons/folder-open';
	import WorkflowIcon from '@lucide/svelte/icons/workflow';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import Table2Icon from '@lucide/svelte/icons/table-2';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Button from '$lib/components/ui/button/button.svelte';
	import type { CostPage } from '../types';
	import { change, costTitle, sharePct, usd, type DrillRow, type Scope, type TreeRow } from '../utils';
	import SparkFill from './spark-fill.svelte';

	// A level's children ranked by cost, as the Catalog's folder tree draws its rows: a chevron opens
	// a row in place (`tree`), its name opens it as the level. Share is of the row's parent.
	let {
		cost,
		rows,
		name,
		hrefOf,
		moreHref,
		tree = false,
		ontoggle,
		selected = null,
		trend,
		embedded = false,
		empty = 'No cost in this period.',
		rowToggle = false,
		peakBars = false,
		smooth = false,
		fillTrend = false,
		onadd,
		dayTitle,
		narrow = false
	}: {
		cost: CostPage;
		rows: TreeRow[];
		name: string;
		hrefOf: (row: DrillRow, id: string) => string;
		moreHref?: (scope: Scope) => string;
		tree?: boolean;
		ontoggle?: (id: string) => void;
		selected?: string | null;
		trend?: (row: DrillRow, id: string) => number[];
		embedded?: boolean;
		empty?: string;
		/** A click or Enter anywhere on a row, outside its links and buttons, opens or closes it. */
		rowToggle?: boolean;
		/** Bars relative to the largest row of the level; the percentage stays of the parent. */
		peakBars?: boolean;
		smooth?: boolean;
		/** The trend takes every pixel left, the name a fixed width, so numbers stay by their names. */
		fillTrend?: boolean;
		/** "Add" on hover: adds the row to the selection. */
		onadd?: (row: DrillRow, id: string) => void;
		/** A day's tooltip on the trend. */
		dayTitle?: (day: number, value: number) => string;
		/** Leaves out queries and change, for a drawer. */
		narrow?: boolean;
	} = $props();

	function rowClick(e: MouseEvent, t: Extract<TreeRow, { row: DrillRow }>) {
		if (!rowToggle || !t.row.deeper || e.button !== 0) return;
		if ((e.target as Element).closest('a, button')) return;
		const sel = window.getSelection();
		if (sel && !sel.isCollapsed) return;
		ontoggle?.(t.id);
	}
	function rowKey(e: KeyboardEvent, t: Extract<TreeRow, { row: DrillRow }>) {
		if (!rowToggle || e.target !== e.currentTarget) return;
		if ((e.key === 'Enter' || e.key === ' ') && t.row.deeper) {
			e.preventDefault();
			ontoggle?.(t.id);
		} else if (e.key === 'ArrowRight' && t.row.deeper && !t.open) ontoggle?.(t.id);
		else if (e.key === 'ArrowLeft' && t.open) ontoggle?.(t.id);
	}

	const INDENT = 18;
	const columns = $derived<TableColumn[]>([
		{ id: 'name', label: name, minWidth: 220, ...(trend ? { width: fillTrend ? '400px' : '30%' } : {}) },
		{ id: 'cost', label: 'Cost', width: '92px', align: 'right' },
		{ id: 'share', label: 'Share', width: '116px', align: 'right', title: 'Share of the level above' },
		{ id: 'runs', label: 'Runs', width: '72px', align: 'right' },
		...(narrow ? [] : [{ id: 'queries', label: 'Queries', width: '84px', align: 'right' as const }, { id: 'change', label: 'Change', width: '82px', align: 'right' as const, title: 'Against the period before' }]),
		...(trend ? [{ id: 'trend', label: 'Per day', minWidth: 140, truncate: false }] : [])
	]);
	const icon = (r: DrillRow, open: boolean) => (r.kind === 'job' ? WorkflowIcon : r.kind === 'model' ? Table2Icon : open ? FolderOpenIcon : FolderIcon);
</script>

<DataTable class="compact static drill {embedded ? 'framed' : ''}" {embedded} {columns} {rows} key={(r) => r.id} rowHeight={36} {empty}>
	{#snippet row(t)}
		{#if 'more' in t}
			<tr class="more-row">
				<td><div class="inner" style:padding-left="{t.depth * INDENT + 26}px">{#if moreHref}<a class="more" href={moreHref(t.scope)} data-sveltekit-noscroll>{t.more} more models →</a>{:else}<span class="more">{t.more} more models</span>{/if}</div></td>
				{#each columns.slice(1) as col (col.id)}<td></td>{/each}
			</tr>
		{:else}
			{@const r = t.row}
			{@const c = change(r.credits, r.previous)}
			{@const Icon = icon(r, t.open)}
			<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
			<tr class:selected={selected === r.key} class:toggles={rowToggle && r.deeper} data-kind={r.kind} tabindex={rowToggle ? 0 : undefined} aria-expanded={rowToggle && r.deeper ? t.open : undefined} onclick={(e) => rowClick(e, t)} onkeydown={(e) => rowKey(e, t)}>
				<td>
					<div class="inner" style:padding-left="{t.depth * INDENT}px">
						{#if tree && r.deeper}
							<Button variant="ghost" size="icon-xs" class="chev {t.open ? 'open' : ''}" aria-expanded={t.open} aria-label="{t.open ? 'Collapse' : 'Expand'} {r.label}" onclick={() => ontoggle?.(t.id)}><ChevronRightIcon /></Button>
						{:else if tree}
							<span class="chev-space"></span>
						{/if}
						<Icon size={14} strokeWidth={1.75} class="ficon" />
						<a href={hrefOf(r, t.id)} title={r.key} data-sveltekit-noscroll>{r.label}</a>
						{#if r.sub}<small>{r.sub}</small>{:else if r.kind !== 'model'}<small>{r.models.toLocaleString('en')}</small>{/if}
						{#if onadd}<!-- raw-button: a chip that shows on the row's hover, styled with the row, not a toolbar button -->
							<button class="add" onclick={() => onadd(r, t.id)} title={r.kind === 'job' ? `Add every node of ${r.key} to the selection` : r.kind === 'folder' ? `Add every node in ${r.key} to the selection` : `Add ${r.key} to the selection`} aria-label="Add {r.label} to the selection"><PlusIcon size={12} />Add</button>{/if}
					</div>
				</td>
				<td title={costTitle(r.credits, cost.price)}>{usd(r.credits, cost.price)}</td>
				<td><span class="share"><span class="bar"><i style:width="{peakBars ? ((t.peak ?? 0) > 0 ? (r.credits / t.peak!) * 100 : 0) : t.parent > 0 ? (r.credits / t.parent) * 100 : 0}%"></i></span><span class="pct">{sharePct(r.credits, t.parent)}</span></span></td>
				<td class="muted">{r.runs.toLocaleString('en')}</td>
				{#if !narrow}
					<td class="muted">{r.queries.toLocaleString('en')}</td>
					<td class="change {c?.tone ?? ''}">{c?.text ?? '—'}</td>
				{/if}
				{#if trend}{@const days = trend(r, t.id)}<td><SparkFill values={days} label="Cost per day of {r.label}" {smooth} fill={fillTrend} titles={dayTitle ? days.map((v, i) => dayTitle(i, v)) : undefined} /></td>{/if}
			</tr>
		{/if}
	{/snippet}
</DataTable>

<style>
	:global(.drill td) { height: 36px; padding-block: 0 !important; }
	.inner { display: flex; min-width: 0; align-items: center; gap: 6px; }
	.inner a { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.inner a:hover { color: var(--primary); text-decoration: underline; }
	.inner small { flex-shrink: 0; color: var(--sb-text-faint); font-size: 11.5px; }
	.inner :global(.chev svg) { transition: transform 0.12s; }
	.inner :global(.chev.open svg) { transform: rotate(90deg); }
	.chev-space { width: 20px; flex-shrink: 0; }
	.inner :global(.ficon) { flex-shrink: 0; color: var(--sb-text-faint); }
	tr[data-kind='model'] a { font-family: var(--font-mono); font-size: 12px; }
	.share { display: inline-flex; align-items: center; justify-content: flex-end; gap: 8px; }
	.bar { width: 40px; height: 4px; border-radius: 2px; background: var(--inner-line); }
	.bar i { display: block; height: 100%; border-radius: 2px; background: var(--primary); }
	.pct { width: 34px; color: var(--muted-foreground); }
	.change.up { color: var(--sb-warning); }
	.change.down { color: var(--sb-success); }
	.change.flat { color: var(--sb-text-faint); }
	tr.selected { background: color-mix(in srgb, var(--primary) 12%, transparent); }
	tr.selected td:first-child { box-shadow: inset 2px 0 0 var(--primary); }
	.more { color: var(--primary); font-size: 12px; }
	tr.toggles { cursor: pointer; }
	tr:focus-visible { outline: 2px solid var(--primary); outline-offset: -2px; }
	.add { display: inline-flex; flex-shrink: 0; align-items: center; gap: 3px; margin-left: 4px; padding: 1px 6px; border: 1px solid var(--border); border-radius: 3px; color: var(--muted-foreground); font-size: 11px; opacity: 0; }
	tr:hover .add, tr:focus-within .add { opacity: 1; }
	.add:hover { border-color: var(--primary); color: var(--primary); }
	@media (prefers-reduced-motion: reduce) { .inner :global(.chev svg) { transition: none; } }
</style>
