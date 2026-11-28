<script lang="ts" generics="T">
	import { untrack, type Snippet } from 'svelte';
	import type { Resource } from '$lib/state/resource.svelte';
	import LoadRow from './load-row.svelte';
	import { createVirtualRows } from './virtual-rows.svelte';
	import { WINDOW_ABOVE, cellClasses, tableMinWidth, type TableColumn } from './table-columns';

	// The one list table. Every list uses it, so they all fit the page (scrolling sideways only below
	// one minimum width), keep a sticky header lined up with the rows, draw only the rows in view above
	// 200 rows, cut long cells with the full text on hover, and show the same loading and empty states.
	// `row` draws one `<tr>`; its cells follow `columns` in order. `embedded` draws a table inside a
	// panel that scrolls with its page instead of on its own.
	let {
		columns,
		rows,
		key,
		row,
		head,
		resource,
		loading = false,
		error = null,
		empty,
		rowHeight,
		embedded = false,
		class: className = '',
		label,
		windowAbove = WINDOW_ABOVE,
		overscan = 8,
		onwindow,
		after,
		emptyRow
	}: {
		columns: TableColumn[];
		rows: readonly T[];
		key: (item: T) => string | number;
		row: Snippet<[T, number]>;
		/** A header cell drawn instead of a column's label, e.g. a select-all checkbox. */
		head?: Snippet<[TableColumn]>;
		resource?: Resource<unknown>;
		loading?: boolean;
		error?: string | null;
		empty: string;
		/** A fixed row height for a windowed list, which keeps its rows to it. Without one, a long list
		 *  windows by the average height of the rows it has drawn, so rows may differ, e.g. a log line
		 *  with its queries opened under it. */
		rowHeight?: number;
		embedded?: boolean;
		class?: string;
		label?: string;
		/** Window lists longer than this; 0 windows every list, as an endless list does. */
		windowAbove?: number;
		/** Rows drawn beyond the view on either side of a windowed list. */
		overscan?: number;
		/** The rows drawn, as indices into `rows`; an endless list loads more as the end comes into view. */
		onwindow?: (range: { first: number; last: number }) => void;
		/** Rows after the list, such as "Loading more…". */
		after?: Snippet;
		/** Drawn instead of the shared loading and empty rows when the list is empty. */
		emptyRow?: Snippet;
	} = $props();

	const isWindowed = $derived(rows.length > windowAbove);
	// Rows drawn, and the height they took: the average sizes the spacers for rows not drawn.
	let average = $state(44);
	const fixed = untrack(() => rowHeight);
	const win = createVirtualRows(fixed ?? (() => average), () => rows.length, untrack(() => overscan));
	const first = $derived(isWindowed ? win.first : 0);
	const last = $derived(isWindowed ? win.last : rows.length);
	const shown = $derived(rows.slice(first, last));
	$effect(() => onwindow?.({ first, last }));
	const minWidth = $derived(tableMinWidth(columns, embedded ? 0 : undefined));
	// A plain resource-like state for lists that load some other way.
	const loadState = $derived<Resource<unknown>>(resource ?? { data: loading ? undefined : rows, error, loading, refresh: () => {} });

	/** Gives each body cell its column's classes, as rows are drawn and redrawn, and measures the rows. */
	function cells(tbody: HTMLTableSectionElement) {
		const measure = () => {
			if (fixed !== undefined || !shown.length) return;
			const spacers = [...tbody.rows].filter((r) => r.classList.contains('spacer') || r.classList.contains('load-row'));
			const height = tbody.getBoundingClientRect().height - spacers.reduce((sum, r) => sum + r.getBoundingClientRect().height, 0);
			if (height > 0) average = Math.max(20, height / shown.length);
		};
		const ro = new ResizeObserver(measure);
		ro.observe(tbody);
		const apply = () => {
			for (const tr of tbody.rows) {
				// A row spanning columns, such as an opened statement's SQL, styles itself.
				if (tr.classList.contains('load-row') || tr.classList.contains('spacer') || [...tr.cells].some((td) => td.colSpan > 1)) continue;
				[...tr.cells].forEach((td, i) => {
					// Adding a class a cell already has still writes the attribute, so skip it.
					for (const c of cellClasses(columns[i])) if (!td.classList.contains(c)) td.classList.add(c);
				});
			}
		};
		apply();
		const observer = new MutationObserver(apply);
		observer.observe(tbody, { childList: true, subtree: true });
		return () => {
			observer.disconnect();
			ro.disconnect();
		};
	}

	/** Shows a cut cell's full text on hover. */
	function fullText(e: MouseEvent) {
		const el = (e.target as Element | null)?.closest('td.trunc, .trunc-inline') as HTMLElement | null;
		if (!el || el.title || el.scrollWidth <= el.clientWidth) return;
		const text = el.textContent?.replace(/\s+/g, ' ').trim();
		if (text) el.title = text;
	}
	function hoverText(el: HTMLElement) {
		el.addEventListener('mouseover', fullText);
		return () => el.removeEventListener('mouseover', fullText);
	}
	// A list inside a panel scrolls with its page, so its window follows the page's scroller.
	const scroller = (el: HTMLElement) => (embedded ? win.attachWithin(el) : win.attach(el));
</script>

<div class="sb-table-scroll" class:embedded {@attach scroller}>
	<table class="sb-list sb-table {className}" class:windowed={isWindowed} class:fixed-rows={isWindowed && fixed !== undefined} style:min-width="{minWidth}px" style:--row-h={fixed ? `${fixed}px` : undefined} aria-label={label} {@attach hoverText}>
		<colgroup>
			{#each columns as c (c.id)}<col style:width={c.width} />{/each}
		</colgroup>
		<thead>
			<tr>
				{#each columns as c (c.id)}
					<th class={[c.class, c.align === 'right' && 'num']} title={c.title}>{#if head && !c.label}{@render head(c)}{:else}{c.label}{/if}</th>
				{/each}
			</tr>
		</thead>
		<tbody {@attach cells}>
			{#if first > 0}<tr class="spacer" style:height="{win.before}px"><td colspan={columns.length}></td></tr>{/if}
			{#each shown as item, i (key(item))}
				{@render row(item, first + i)}
			{:else}
				{#if emptyRow}{@render emptyRow()}{:else}<LoadRow colspan={columns.length} resource={loadState} {empty} />{/if}
			{/each}
			{#if isWindowed && last < rows.length}<tr class="spacer" style:height="{win.after}px"><td colspan={columns.length}></td></tr>{/if}
			{#if after && rows.length}{@render after()}{/if}
		</tbody>
	</table>
</div>

<style>
	.sb-table-scroll {
		min-height: 0;
		flex: 1;
		overflow: auto;
	}
	.sb-table-scroll.embedded {
		flex: none;
		overflow-x: auto;
		overflow-y: visible;
	}
	.sb-table {
		width: 100%;
		table-layout: fixed;
	}
	.sb-table :global(td.trunc) {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.sb-table :global(td.wrap) {
		overflow-wrap: anywhere;
	}
	.sb-table.fixed-rows :global(tbody tr:not(.spacer):not(.load-row)) {
		height: var(--row-h);
	}
	.sb-table.fixed-rows :global(tbody td) {
		padding-top: 4px;
		padding-bottom: 4px;
	}
	.spacer,
	.spacer td {
		padding: 0 !important;
		border: 0 !important;
	}
	.sb-table :global(tbody tr.spacer:hover) {
		background: transparent;
	}
</style>
