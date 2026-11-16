<script lang="ts">
	import '@xyflow/svelte/dist/style.css';
	import { Background, Controls, Panel, SvelteFlow, type Edge, type Node } from '@xyflow/svelte';
	import { tick, untrack } from 'svelte';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import type { CatalogNode, JobGroupMember } from '$lib/api';
	import Button from '$lib/components/ui/button/button.svelte';
	import AssetNode from '$lib/components/lineage/asset-node.svelte';
	import FolderNode from '$lib/components/lineage/folder-node.svelte';
	import TooManyCard from '$lib/components/lineage/too-many-card.svelte';
	import FlowController from '$lib/lineage/flow-controller.svelte';
	import { GROUPED_ABOVE } from '$lib/lineage/catalog-graph';
	import { folderGraph, folderTree, layoutFolderGraph, tooManyToDraw, withFolderClosed } from '$lib/lineage/folder-graph';
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
		onselect
	}: { keys: string[]; catalog: CatalogNode[]; boxes?: JobBox[]; members?: JobGroupMember[]; onselect: (keys: string[]) => void } = $props();

	const nodeTypes = { asset: AssetNode, job: JobBoxNode, folder: FolderNode, folderBox: JobBoxNode };
	const edgeTypes = { count: CountEdge };
	let nodes = $state.raw<Node[]>([]);
	let edges = $state.raw<Edge[]>([]);
	let fit = $state<((opts?: { duration?: number }) => void) | undefined>();
	let fitted = $state(false);
	let shape = '';
	let laidOut = '';
	/** Boxes showing only their header. */
	let collapsed = $state<ReadonlySet<string>>(new Set());
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
	const inScope = $derived.by(() => {
		const wanted = new Set(keys);
		return catalog.filter((n) => wanted.has(n.key));
	});
	const tree = $derived(folderTree(inScope));
	const closeFolder = (folder: string) => (open = withFolderClosed(open, folder));
	const openFolder = (folder: string) => (open = new Set([...open, folder]));

	const graph = $derived.by(() => {
		if (canGroup && grouped) return folderGraph(inScope, open, { tree, onclose: closeFolder });
		const next = jobGraph(keys, catalog, boxes, members, collapsed);
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

<div class="graph sb-flow" class:unfitted={!fitted && nodes.length > 0}>
	<SvelteFlow
		bind:nodes
		bind:edges
		{nodeTypes}
		{edgeTypes}
		nodesDraggable={false}
		multiSelectionKey={['Meta', 'Control', 'Shift']}
		minZoom={0.1}
		proOptions={{ hideAttribution: true }}
		onnodeclick={({ node }) => node.type === 'folder' && openFolder(String(node.data.group))}
		onselectionchange={({ nodes: picked }) => onselect(picked.filter((n) => n.type === 'asset').map((n) => n.id))}
	>
		<FlowController bind:fitView={fit} bind:fitted />
		<Background gap={64} bgColor="var(--sb-inset)" patternColor="var(--sb-grid)" />
		<Controls showLock={false} />
		{#if canGroup && keys.length > GROUPED_ABOVE && !tooMany}
			<Panel position="top-left">
				<Button variant={grouped ? 'default' : 'outline'} size="bar" onclick={() => (grouped = !grouped)} title={grouped ? 'Draw every node' : 'Draw each folder as one card; click a folder to open it'}><FolderIcon />Folders</Button>
			</Panel>
		{/if}
	</SvelteFlow>
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
	.graph {
		position: relative;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--sb-inset);
	}
	.unfitted :global(.svelte-flow__viewport),
	.unfitted :global(.svelte-flow__controls) {
		/* Not visibility: Svelte Flow sets each measured card visible, which would override it. */
		opacity: 0;
	}
	.graph :global(.svelte-flow__node-asset),
	.graph :global(.svelte-flow__node-folder) {
		cursor: pointer;
	}
	.graph :global(.svelte-flow__controls-button) {
		background: var(--card);
		border-bottom: 1px solid var(--border);
	}
	.graph :global(.svelte-flow__controls-button svg) {
		fill: var(--muted-foreground);
	}
</style>
