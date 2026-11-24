<script lang="ts">
	import type { CostPrice } from '$lib/api';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { change, costTitle, count, money, sharePct } from '../utils/format';
	import type { DrillRow, TreeRow } from '../utils/tree';
	import { SPARK_H, sparkPaths } from '../utils/spark';

	// A level's children ranked by cost, as the Catalog's folder tree draws its rows: a click on a
	// row (or Enter, or the arrow keys) opens it in place, its name drills in. Bars are of the largest
	// row beside it; the percentage is of the row's parent.
	let {
		price,
		rows,
		name,
		hrefOf,
		moreHref,
		ontoggle,
		selected = null,
		trend,
		onadd,
		dayTitle,
		onwindow,
		onfollow,
		embedded = false,
		narrow = false,
		empty = 'No cost in this period.'
	}: {
		price: CostPrice | null;
		rows: readonly TreeRow[];
		name: string;
		/** Where a row's name drills to; null for rows that can't be selected, such as builds by hand. */
		hrefOf: (row: DrillRow, id: string) => string | null;
		moreHref?: () => string;
		ontoggle?: (id: string) => void;
		selected?: string | null;
		/** A row's cost per day; null while it loads. */
		trend?: (row: DrillRow, id: string) => readonly number[] | null;
		/** "Add" on hover: adds the row to the selection. */
		onadd?: (row: DrillRow) => void;
		dayTitle?: (day: number, value: number) => string;
		/** The rows drawn, for loading their trends as they come into view. */
		onwindow?: (range: { first: number; last: number }) => void;
		/** A click on a row's name; the page may take the link in place instead of navigating. */
		onfollow?: (event: MouseEvent, href: string) => void;
		embedded?: boolean;
		/** Leaves out queries and change, for a drawer. */
		narrow?: boolean;
		empty?: string;
	} = $props();

	type Opened = Extract<TreeRow, { row: DrillRow }>;
	function rowClick(e: MouseEvent, t: Opened) {
		if (!ontoggle || !t.row.deeper || e.button !== 0) return;
		if ((e.target as Element).closest('a, button')) return;
		const sel = window.getSelection();
		if (sel && !sel.isCollapsed) return;
		ontoggle(t.id);
	}
	function rowKey(e: KeyboardEvent, t: Opened) {
		if (!ontoggle || e.target !== e.currentTarget || !t.row.deeper) return;
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			ontoggle(t.id);
		} else if ((e.key === 'ArrowRight' && !t.open) || (e.key === 'ArrowLeft' && t.open)) ontoggle(t.id);
	}

	const INDENT = 18;
	const columns = $derived<TableColumn[]>([
		{ id: 'name', label: name, minWidth: 220, ...(trend ? { width: '400px' } : {}) },
		{ id: 'cost', label: 'Cost', width: '100px', align: 'right' },
		{ id: 'share', label: 'Share', width: '116px', align: 'right', title: 'Share of the level above' },
		{ id: 'runs', label: 'Runs', width: '72px', align: 'right' },
		...(narrow ? [] : [{ id: 'queries', label: 'Queries', width: '84px', align: 'right' as const }, { id: 'change', label: 'Change', width: '82px', align: 'right' as const, title: 'Against the period before' }]),
		...(trend ? [{ id: 'trend', label: 'Per day', minWidth: 140, truncate: false }] : [])
	]);
	// A level's own rows are keyed by their place, so moving to another level updates the rows drawn
	// in place; rows opened under them are keyed by their path, so opening a row draws only its rows.
	const slots = $derived.by(() => {
		const keys = new Map<TreeRow, string>();
		let top = 0;
		for (const r of rows) keys.set(r, r.depth === 0 ? `top:${top++}` : r.id);
		return keys;
	});
	/** The day under the pointer as the trend's tooltip, set on the element: no state, no redraw. */
	function dayTip(e: PointerEvent, days: readonly number[]) {
		if (!dayTitle || !days.length) return;
		const svg = e.currentTarget as SVGSVGElement;
		const box = svg.getBoundingClientRect();
		const day = Math.min(days.length - 1, Math.max(0, Math.round(((e.clientX - box.left) / Math.max(1, box.width)) * (days.length - 1))));
		const title = svg.querySelector('title');
		if (title) title.textContent = dayTitle(day, days[day]);
	}
	/** Each kind's icon, Lucide's shapes as one path: workflow, table, folder and open folder. */
	const ICONS: Record<DrillRow['kind'] | 'open', string> = {
		job: 'M5 3h4a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zM7 11v4a2 2 0 0 0 2 2h4M15 13h4a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2h-4a2 2 0 0 1-2-2v-4a2 2 0 0 1 2-2z',
		model: 'M9 3H5a2 2 0 0 0-2 2v4m6-6h10a2 2 0 0 1 2 2v4M9 3v18m0 0h10a2 2 0 0 0 2-2V9M9 21H5a2 2 0 0 1-2-2V9m0 0h18',
		folder: 'M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z',
		open: 'm6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2'
	};
	const NO_SPARK = { area: '', line: '' };
	const addTitle = (r: DrillRow) => (r.kind === 'job' ? `Add every node of ${r.key} to the selection` : r.kind === 'folder' ? `Add every node in ${r.key} to the selection` : `Add ${r.label} to the selection`);
</script>

<!-- Always windowed: a tree opened wide draws only the rows in view, and opening one never switches modes. -->
<DataTable class="compact static drill {embedded ? 'framed' : ''}" {embedded} {columns} {rows} key={(r) => slots.get(r) ?? r.id} rowHeight={36} windowAbove={embedded ? undefined : 0} overscan={4} {empty} {onwindow}>
	{#snippet row(t)}
		{#if 'more' in t}
			<tr class="more-row">
				<td><div class="inner" style:padding-left="{t.depth * INDENT + 26}px">{#if moreHref}<a class="more" href={moreHref()} data-sveltekit-noscroll>{count(t.more)} more models →</a>{:else}<span class="more">{count(t.more)} more models</span>{/if}</div></td>
				{#each columns.slice(1) as col (col.id)}<td></td>{/each}
			</tr>
		{:else}
			{@const r = t.row}
			{@const c = change(r.credits, r.previous)}
			{@const href = hrefOf(r, t.id)}
			{@const opens = !!ontoggle && r.deeper}
			{@const days = trend?.(r, t.id) ?? null}
			{@const spark = days ? sparkPaths(days) : NO_SPARK}
			<!-- One row's markup with no blocks inside: what changes are attributes and text, so drawing many
			     rows, or moving to another level, stays cheap. -->
			<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
			<tr class:selected={selected === r.key} class:expandable={opens} data-kind={r.kind} data-id={t.id} tabindex={opens ? 0 : undefined} aria-expanded={opens ? t.open : undefined} onclick={(e) => rowClick(e, t)} onkeydown={(e) => rowKey(e, t)}>
				<td>
					<div class="inner" style:padding-left="{t.depth * INDENT}px">
						<!-- The row itself opens and closes; the chevron only shows which it is. -->
						<span class="chev" class:open={t.open} class:none={!opens} class:gone={!ontoggle} aria-hidden="true"></span>
						<!-- Lucide's shapes as one path per kind. -->
						<svg class="ficon" viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d={ICONS[r.kind === 'folder' && t.open ? 'open' : r.kind]} /></svg>
						<a class="row-name" class:plain={!href} href={href ?? undefined} title={href ? r.key : 'Builds by hand or from the command line have no job to select'} data-sveltekit-noscroll onclick={(e) => href && onfollow?.(e, href)}>{r.label}</a>
						<small>{r.sub ?? (r.kind === 'model' ? '' : count(r.models))}</small>
						<!-- raw-button: a chip that shows on the row's hover, styled with the row, not a toolbar button -->
						<button class="add" class:gone={!onadd || !href} onclick={() => onadd?.(r)} title={addTitle(r)} aria-label="Add {r.label} to the selection">+ Add</button>
					</div>
				</td>
				<td title={costTitle(r.credits, price)}>{money(r.credits, price)}</td>
				<td><span class="share" style:--bar-part={t.peak > 0 ? r.credits / t.peak : 0}>{sharePct(r.credits, t.parent)}</span></td>
				<td class="muted">{count(r.runs)}</td>
				{#if !narrow}
					<td class="muted">{count(r.queries)}</td>
					<td class="change {c?.tone ?? ''}">{c?.text ?? '—'}</td>
				{/if}
				{#if trend}
					<td>
						<svg class="spark" class:loading={!days} viewBox="0 0 100 {SPARK_H}" preserveAspectRatio="none" role="img" aria-label="Cost per day of {r.label}" onpointermove={(e) => days && dayTip(e, days)}>
							<title>Cost per day of {r.label}</title>
							<path class="area" d={spark.area} />
							<path class="line" d={spark.line} vector-effect="non-scaling-stroke" />
						</svg>
					</td>
				{/if}
			</tr>
		{/if}
	{/snippet}
</DataTable>

<style>
	:global(.drill td) { height: 36px; padding-block: 0 !important; }
	.inner { display: flex; min-width: 0; align-items: center; gap: 6px; }
	.inner a { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.inner a:not(.plain):hover { color: var(--primary); }
	.inner small { flex-shrink: 0; color: var(--sb-text-faint); font-size: 11.5px; }
	.chev { display: inline-flex; width: 20px; flex-shrink: 0; justify-content: center; color: var(--sb-text-faint); }
	/* The chevron, drawn by CSS: one element a row. */
	.chev::before { width: 6px; height: 6px; border-right: 1.75px solid currentColor; border-bottom: 1.75px solid currentColor; content: ''; transform: rotate(-45deg); transition: transform 0.12s; }
	.chev.open::before { transform: rotate(45deg); }
	.chev { height: 14px; align-items: center; }
	.ficon { flex-shrink: 0; fill: none; stroke: currentColor; stroke-width: 1.75; stroke-linecap: round; stroke-linejoin: round; color: var(--sb-text-faint); }
	.chev.none::before { content: none; }
	.gone { display: none !important; }
	.inner small:empty { display: none; }
	.row-name.plain { color: inherit; cursor: inherit; }
	.spark.loading { border-bottom: 1px dashed var(--inner-line); }
	tr[data-kind='model'] .row-name { font-family: var(--font-mono); font-size: 12px; }
	/* The bar and the percentage in one element: the bar is its background, left of the text. */
	.share { display: inline-block; width: 82px; padding-left: 48px; color: var(--muted-foreground); text-align: right; background: linear-gradient(var(--primary), var(--primary)) 0 50% / calc(40px * var(--bar-part, 0)) 4px no-repeat, linear-gradient(var(--inner-line), var(--inner-line)) 0 50% / 40px 4px no-repeat; }
	.change.up { color: var(--sb-warning); }
	.change.down { color: var(--sb-success); }
	.change.flat { color: var(--sb-text-faint); }
	tr.selected { background: color-mix(in srgb, var(--primary) 12%, transparent); }
	tr.selected td:first-child { box-shadow: inset 2px 0 0 var(--primary); }
	.more { color: var(--primary); font-size: 12px; }
	tr:focus-visible { outline: 2px solid var(--primary); outline-offset: -2px; }
	.spark { display: block; width: 100%; height: 20px; }
	.spark .area { fill: color-mix(in srgb, var(--primary) 16%, transparent); }
	.spark .line { fill: none; stroke: var(--primary); stroke-width: 1.3; stroke-linejoin: round; }
	.add { display: inline-flex; flex-shrink: 0; align-items: center; gap: 3px; margin-left: 4px; padding: 1px 6px; border: 1px solid var(--border); border-radius: 3px; color: var(--muted-foreground); font-size: 11px; opacity: 0; }
	tr:hover .add, tr:focus-within .add { opacity: 1; }
	.add:hover { border-color: var(--primary); color: var(--primary); }
	@media (prefers-reduced-motion: reduce) { .chev::before { transition: none; } }
</style>
