<script lang="ts">
	import type { Edge, Node } from '@xyflow/svelte';
	import { tick, untrack } from 'svelte';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import type { CatalogNode, JobGroupMember } from '$lib/api';
	import Button from '$lib/components/ui/button/button.svelte';
	import AssetNode from '$lib/components/lineage/asset-node.svelte';
	import FolderNode from '$lib/components/lineage/folder-node.svelte';
	import TooManyCard from '$lib/components/lineage/too-many-card.svelte';
	import GraphCanvas from '$lib/components/graph/graph-canvas.svelte';
	import GraphToolbar from '$lib/components/graph/graph-toolbar.svelte';
	import { GROUPED_ABOVE } from '$lib/lineage/catalog-graph';
	import { MAX_DRAWN, expandAll, folderGraph, folderTree, layoutFolderGraph, tooManyToDraw, withFolderClosed } from '$lib/lineage/folder-graph';
	import { jobGraph, layoutJobGraph, type JobBox } from '$lib/lineage/job-graph';
	import JobBoxNode from './job-box-node.svelte';
	import CountEdge from './count-edge.svelte';

	// Nodes and the lineage between them; each box, such as a group's member job, is drawn around its nodes.
	// Clicking a node selects it; Shift, Ctrl or Cmd adds to the selection.
	// As on Lineage, a large graph opens with each folder as one card, and a graph with more cards than
	// the browser can lay out shows a message instead, so no page drawing it can hang the browser.
	let {
		keys,
		catalog,
		boxes = [],
		members = [],
		startCollapsed = [],
		wholeFloor,
		onselect
	}: { keys: string[]; catalog: CatalogNode[]; boxes?: JobBox[]; members?: JobGroupMember[]; startCollapsed?: string[]; wholeFloor?: number; onselect: (keys: string[]) => void } = $props();

	const nodeTypes = { asset: AssetNode, job: JobBoxNode, folder: FolderNode, folderBox: JobBoxNode };
	const edgeTypes = { count: CountEdge };
	let nodes = $state.raw<Node[]>([]);
	let edges = $state.raw<Edge[]>([]);
	let fit = $state<((opts?: { duration?: number }) => void) | undefined>();
	let fitted = $state(false);
	let shape = '';
	let laidOut = '';
	/** Boxes showing only their header. */
	let collapsed = $state<ReadonlySet<string>>(untrack(() => new Set(startCollapsed)));
	function toggle(jobKey: string) {
		const next = new Set(collapsed);
		if (!next.delete(jobKey)) next.add(jobKey);
		collapsed = next;
	}

	// Folders apply to a plain set of nodes; member boxes already group a group's nodes.
	const canGroup = $derived(boxes.length === 0);
	// Each folder as one card; starts on for a graph as large as the one Lineage opens by folder.
	let grouped = $state(untrack(() => keys.length > GROUPED_ABOVE));
	let open = $state<ReadonlySet<string>>(new Set());
	// Focus: only the selected nodes and the nodes they read or feed, as on Lineage.
	// Read from the drawn nodes: writing state from the library's selection callback stops its updates.
	const selectedKeys = $derived(nodes.filter((n) => n.selected && n.type === 'asset').map((n) => n.id));
	let focusKeys = $state<ReadonlySet<string> | null>(null);
	const shownKeys = $derived(focusKeys ? keys.filter((k) => focusKeys!.has(k)) : keys);
	const inScope = $derived.by(() => {
		const wanted = new Set(shownKeys);
		return catalog.filter((n) => wanted.has(n.key));
	});
	function toggleFocus() {
		if (focusKeys) {
			focusKeys = null;
			return;
		}
		const picked = new Set(selectedKeys);
		const around = new Set(picked);
		for (const n of catalog) {
			if (picked.has(n.key)) for (const up of n.upstream) around.add(up);
			if (n.upstream.some((up) => picked.has(up))) around.add(n.key);
		}
		focusKeys = around;
	}
	let expandNote = $state('');
	function expandEverything() {
		const result = expandAll(tree, open);
		open = result.open;
		expandNote = result.complete ? '' : `Opened ${result.opened} of ${result.total} folders: the graph draws at most ${MAX_DRAWN.toLocaleString('en')} cards`;
	}
	/** Back to how the graph opened: no focus, folders closed, every box open, laid out and fitted again. */
	async function reset() {
		focusKeys = null;
		open = new Set();
		collapsed = new Set(startCollapsed);
		grouped = keys.length > GROUPED_ABOVE;
		expandNote = '';
		await tick();
		// A reset that changed what is drawn is laid out, and fitted, once the new cards are measured.
		if (laidOut !== shape) return;
		if (nodes.length) nodes = folderMode ? layoutFolderGraph(nodes, edges) : layoutJobGraph(nodes, edges);
		await tick();
		requestAnimationFrame(() => fit?.());
	}
	const tree = $derived(folderTree(inScope));
	const closeFolder = (folder: string) => (open = withFolderClosed(open, folder));
	const openFolder = (folder: string) => (open = new Set([...open, folder]));

	const graph = $derived.by(() => {
		if (canGroup && grouped) return folderGraph(inScope, open, { tree, onclose: closeFolder });
		const next = jobGraph(shownKeys, catalog, boxes, members, collapsed);
		for (const n of next.nodes) if (n.type === 'job') n.data = { ...n.data, ontoggle: toggle };
		return next;
	});
	const tooMany = $derived(tooManyToDraw(graph));
	const folderMode = $derived(canGroup && grouped);

	/** Clears the selection, as clicking the empty canvas does. */
	export function clear() {
		nodes = nodes.map((n) => (n.selected ? { ...n, selected: false } : n));
		onselect([]);
	}

	$effect(() => {
		const next = graph;
		if (tooMany) {
			shape = '';
			nodes = [];
			edges = [];
			untrack(() => onselect([]));
			return;
		}
		const sig = `${folderMode}|` + next.nodes.map((n) => `${n.id}@${n.parentId ?? ''}`).join(',') + `|${[...collapsed].join(',')}`;
		if (sig === shape) {
			// Same shape: keep positions and sizes, refresh build states.
			const data = new Map(next.nodes.map((n) => [n.id, n.data]));
			nodes = untrack(() => nodes).map((n) => ({ ...n, data: data.get(n.id) ?? n.data }));
		} else {
			const selected = new Set(untrack(() => nodes).filter((n) => n.selected).map((n) => n.id));
			shape = sig;
			laidOut = '';
			nodes = next.nodes.map((n) => (selected.has(n.id) ? { ...n, selected: true } : n));
		}
		edges = next.edges;
	});

	$effect(() => {
		const cards = nodes.filter((n) => n.type !== 'job' && n.type !== 'folderBox');
		if (laidOut === shape || !nodes.length || !cards.every((n) => n.measured?.width && n.measured?.height)) return;
		laidOut = shape;
		nodes = folderMode ? layoutFolderGraph(nodes, edges) : layoutJobGraph(nodes, edges);
		// Fit once Svelte Flow has taken the new box sizes.
		tick().then(() => requestAnimationFrame(() => fit?.({ duration: 0 })));
	});
</script>

<div class="scope-graph">
	<GraphToolbar
		folders={canGroup ? { on: grouped, toggle: () => (grouped = !grouped) } : undefined}
		expand={folderMode && !tooMany ? { all: expandEverything, collapse: () => ((open = new Set()), (expandNote = '')), canCollapse: open.size > 0, note: expandNote } : undefined}
		focus={{ on: !!focusKeys, can: selectedKeys.length > 0, toggle: toggleFocus, title: 'Show only the selected nodes and what they read or feed' }}
		onreset={reset}
	/>
	<GraphCanvas
		bind:nodes
		bind:edges
		bind:fit
		bind:fitted
		{wholeFloor}
		{nodeTypes}
		{edgeTypes}
		nodesDraggable={false}
		multiSelectionKey={['Meta', 'Control', 'Shift']}
		onnodeclick={({ node }) => node.type === 'folder' && openFolder(String(node.data.group))}
		onselectionchange={({ nodes: picked }) => onselect(picked.filter((n) => n.type === 'asset').map((n) => n.id))}
	/>
	{#if tooMany}
		<TooManyCard count={graph.nodes.length}>
			{#snippet actions()}
				{#if canGroup && !grouped}<Button variant="outline" size="bar" onclick={() => (grouped = true)}><FolderIcon size={13} />Show folders</Button>{/if}
				{#if folderMode && open.size}<Button variant="outline" size="bar" onclick={() => (open = new Set())}>Close every folder</Button>{/if}
			{/snippet}
		</TooManyCard>
	{/if}
</div>

<style>
	.scope-graph {
		position: relative;
		display: flex;
		height: 100%;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--sb-inset);
	}
	.scope-graph :global(.svelte-flow__node-asset),
	.scope-graph :global(.svelte-flow__node-folder) {
		cursor: pointer;
	}
</style>
