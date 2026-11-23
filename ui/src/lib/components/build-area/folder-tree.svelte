<script lang="ts">
	// The catalog's folders as a tree that expands in place. Each folder shows where its nodes stand,
	// its subfolders' included; sorting applies within each level. An open folder lists its subfolders,
	// then its own nodes (the first 20, then a link to the rest). Clicking anywhere on a folder's label
	// opens or closes it, except on its name, which, like each count, opens those nodes in the catalog.
	import type { CatalogNode } from '$lib/api';
	import { folderPaths, folderTree, visibleTreeRows, type FolderNode, type FolderSort, type TreeRow } from '$lib/build-area/folder-tree';
	import { needsAttention } from '$lib/build-area/overview-panels';
	import { nodeIcon, nodeIconKey, shortName } from '$lib/build-area/labels';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import FolderOpenIcon from '@lucide/svelte/icons/folder-open';

	let {
		nodes,
		linksNodes = true,
		onopen
	}: {
		nodes: readonly CatalogNode[];
		/** Whether a node's name opens its page. */
		linksNodes?: boolean;
		/** Opens nodes in the catalog's list, by selection and optionally one status. */
		onopen: (selection: string, status: string | null) => void;
	} = $props();

	const NODE_LIMIT = 20;
	const ROW_H = 36;
	const INDENT = 18;
	let sort = $state<FolderSort>('urgency');
	let reverse = $state(false);
	let attentionOnly = $state(false);
	let expanded = $state<ReadonlySet<string>>(new Set());
	const tree = $derived(folderTree(nodes));
	const rows = $derived(visibleTreeRows(tree, { expanded, sort, reverse, attentionOnly, listNodes: true, nodeLimit: NODE_LIMIT }));
	const needing = $derived(tree.filter(needsAttention).length);

	const columns: TableColumn[] = [
		{ id: 'group', label: '', minWidth: 260 },
		{ id: 'bar', label: '', width: '160px', truncate: false },
		{ id: 'current', label: '', width: '104px', align: 'right' },
		{ id: 'changed', label: '', width: '96px', align: 'right' },
		{ id: 'behind', label: '', width: '96px', align: 'right' },
		{ id: 'failed', label: '', width: '96px', align: 'right' }
	];
	const LABELS: Record<string, string> = { group: 'Folder', current: 'Current', changed: 'Changed', behind: 'Behind', failed: 'Failed' };
	const COUNTS = [
		{ id: 'changed', tone: 'var(--sb-stale)' },
		{ id: 'behind', tone: 'var(--sb-behind)' },
		{ id: 'failed', tone: 'var(--sb-error)' }
	] as const;

	function toggle(path: string) {
		const next = new Set(expanded);
		if (!next.delete(path)) next.add(path);
		expanded = next;
	}
	/** A click on a folder's label, outside its links and button, opens or closes it; selecting text in it does not. */
	function labelClick(e: MouseEvent, row: Extract<TreeRow, { kind: 'folder' }>) {
		if (!row.expandable || e.button !== 0) return;
		const target = e.target as Element;
		if (target.closest('a, button')) return;
		const selection = window.getSelection();
		if (selection && !selection.isCollapsed && (e.currentTarget as Node).contains(selection.anchorNode)) return;
		toggle(row.folder.path);
	}
	function sortBy(id: FolderSort) {
		if (sort === id) reverse = !reverse;
		else [sort, reverse] = [id, false];
	}
	const arrow = (id: string) => (sort === id ? (reverse ? ' ↑' : ' ↓') : '');
	const href = (selection: string, status: string | null = null) => to(`/catalog?${new URLSearchParams({ select: selection, ...(status ? { status } : {}) })}`);
	function open(e: MouseEvent, selection: string, status: string | null = null) {
		if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return;
		e.preventDefault();
		onopen(selection, status);
	}
	/** A folder's own nodes as a selection: the folder itself when it has no subfolders. */
	const ownSelection = (f: FolderNode) => (f.children.length ? [...new Set(f.members.map((n) => shortName(n.key)))].join(' ') : f.selection);
	const nodeState = (n: CatalogNode): { label: string; tone: string } =>
		n.lastOutcome === 'failed'
			? { label: 'Failed', tone: 'var(--sb-error)' }
			: n.stale?.some((s) => s.reason !== 'behind')
				? { label: 'Changed', tone: 'var(--sb-stale)' }
				: n.stale?.length
					? { label: 'Behind', tone: 'var(--sb-behind)' }
					: n.lastBuild
						? { label: 'Current', tone: 'var(--sb-success)' }
						: { label: 'Not built', tone: 'var(--muted-foreground)' };
	const rowKey = (r: TreeRow) => (r.kind === 'folder' ? `f:${r.folder.path}` : r.kind === 'node' ? `n:${r.node.key}` : `m:${r.folder.path}`);
</script>

<div class="folder-tree" data-testid="folder-tree">
	<div class="tree-bar">
		<span class="count">{tree.length} folders · <b class:warn={needing > 0}>{needing} need attention</b></span>
		<label class="only"><input type="checkbox" class="sb-check" bind:checked={attentionOnly} />Needs attention only</label>
		<span class="spacer"></span>
		{#if sort !== 'urgency' || reverse}<Button variant="ghost" size="sm" onclick={() => ([sort, reverse] = ['urgency', false])}>Most urgent first</Button>{/if}
		<Button variant="ghost" size="sm" onclick={() => (expanded = new Set(folderPaths(tree)))}>Expand all</Button>
		<Button variant="ghost" size="sm" disabled={!expanded.size} onclick={() => (expanded = new Set())}>Collapse all</Button>
	</div>
	<DataTable class="folders static" {columns} {rows} key={rowKey} rowHeight={ROW_H} label="Folders" empty={attentionOnly ? 'No folder needs attention.' : 'Nothing deployed yet.'}>
		{#snippet head(c)}
			{#if c.id !== 'bar'}
				<Button variant="ghost" size="inline" class="sort" aria-label="Sort by {LABELS[c.id]}" onclick={() => sortBy(c.id as FolderSort)}>{LABELS[c.id]}{arrow(c.id)}</Button>
			{/if}
		{/snippet}
		{#snippet row(r)}
			{#if r.kind === 'folder'}
				{@const f = r.folder}
				<tr class:quiet={!needsAttention(f)} class:expandable={r.expandable} data-path={f.path} aria-level={f.depth + 1} onclick={(e) => labelClick(e, r)}>
					<td class="label" class:toggles={r.expandable} data-testid="folder-label">
						<div class="inner" style:padding-left="{f.depth * INDENT}px">
							{#if r.expandable}
								<Button variant="ghost" size="icon-xs" class="chev {r.expanded ? 'open' : ''}" aria-expanded={r.expanded} aria-label="{r.expanded ? 'Collapse' : 'Expand'} {f.path}" onclick={() => toggle(f.path)} data-testid="folder-toggle"><ChevronRightIcon /></Button>
							{:else}
								<span class="chev-space"></span>
							{/if}
							{#if r.expanded}<FolderOpenIcon size={14} class="ficon" />{:else}<FolderIcon size={14} class="ficon" />{/if}
							<a class="row-name" href={href(f.selection)} onclick={(e) => open(e, f.selection)} title="Open {f.path} in the catalog" data-testid="folder-name">{f.group}</a>
							{#if !r.expanded && f.children.length}<small>{f.children.length} subfolders</small>{/if}
						</div>
					</td>
					<td>
						<span class="gbar" title="{f.current} of {f.total} current">
							<i style:width="{(f.current / f.total) * 100}%" style:background="var(--sb-success)"></i>
							<i style:width="{(f.changed / f.total) * 100}%" style:background="var(--sb-stale)"></i>
							<i style:width="{(f.failed / f.total) * 100}%" style:background="var(--sb-error)"></i>
						</span>
					</td>
					<td class="num"><b>{f.current}/{f.total}</b></td>
					{#each COUNTS as c (c.id)}
						{@const n = f[c.id]}
						<td class="num">{#if n}<a href={href(f.selection, c.id)} onclick={(e) => open(e, f.selection, c.id)} style:color={c.tone} title="Open its {n} {c.id} nodes in the catalog" data-testid="folder-count-{c.id}">{n}</a>{:else}<span class="dash">—</span>{/if}</td>
					{/each}
				</tr>
			{:else if r.kind === 'node'}
				{@const n = r.node}
				{@const Glyph = nodeIcon[nodeIconKey(n)] ?? nodeIcon.table}
				{@const state = nodeState(n)}
				<tr class="leaf">
					<td>
						<div class="inner" style:padding-left="{r.depth * INDENT + 22}px">
							<Glyph size={13} strokeWidth={1.75} class="ficon" />
							{#if linksNodes}<a class="mono" href={to(`/catalog/${n.key}`)} title={n.key}>{shortName(n.key)}</a>{:else}<span class="mono" title={n.key}>{shortName(n.key)}</span>{/if}
						</div>
					</td>
					<td><span class="state" style:color={state.tone}>{state.label}</span></td>
					<td></td>
					<td></td>
					<td></td>
					<td></td>
				</tr>
			{:else}
				<tr class="leaf">
					<td>
						<div class="inner" style:padding-left="{(r.folder.depth + 1) * INDENT + 22}px">
							<a class="more" href={href(ownSelection(r.folder))} onclick={(e) => open(e, ownSelection(r.folder))} data-testid="folder-more">{r.hidden} more in {r.folder.group} →</a>
						</div>
					</td>
					<td></td>
					<td></td>
					<td></td>
					<td></td>
					<td></td>
				</tr>
			{/if}
		{/snippet}
	</DataTable>
</div>

<style>
	.folder-tree { display: flex; min-height: 0; flex: 1; flex-direction: column; }
	/* Lined up with the table: the same 20 px edges as its first and last cells. */
	.tree-bar { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 14px; padding: 6px 20px; border-bottom: 1px solid var(--border); color: var(--muted-foreground); font-size: 12.5px; }
	.count b { font-weight: 500; }
	.count b.warn { color: var(--sb-warning); }
	.only { display: flex; align-items: center; gap: 6px; }
	.spacer { flex: 1; }
	:global(.folders) td { height: 36px; padding-block: 0 !important; }
	:global(.folders .sort) { color: inherit; font-size: 12px; font-weight: 500; }
	:global(.folders .sort:hover) { color: var(--foreground); }
	.label.toggles { cursor: pointer; }
	.inner { display: flex; min-width: 0; align-items: center; gap: 6px; }
	.inner a { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.label a:hover, .leaf a:hover { color: var(--primary); text-decoration: underline; }
	.inner small { flex-shrink: 0; color: var(--sb-text-faint); font-size: 11.5px; }
	.inner :global(.chev svg) { transition: transform 0.12s; }
	.inner :global(.chev.open svg) { transform: rotate(90deg); }
	.chev-space { width: 20px; flex-shrink: 0; }
	.inner :global(.ficon) { flex-shrink: 0; color: var(--sb-text-faint); }
	tr.quiet .label a { color: var(--muted-foreground); }
	.gbar { display: flex; width: 100%; height: 5px; overflow: hidden; border-radius: 3px; background: var(--inner-line); }
	.gbar i { display: block; height: 100%; }
	.num a { font-weight: 600; }
	.num a:hover { text-decoration: underline; }
	.dash { color: var(--sb-text-faint); }
	.state { font-size: 12px; }
	.mono { font-family: var(--font-mono); font-size: 11.5px; }
	.more { color: var(--primary); font-size: 12px; }
	@media (prefers-reduced-motion: reduce) { .inner :global(.chev svg) { transition: none; } }
</style>
