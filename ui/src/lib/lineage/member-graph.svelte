<script lang="ts">
	import '@xyflow/svelte/dist/style.css';
	import { Background, SvelteFlow, type Edge, type Node, type Viewport } from '@xyflow/svelte';
	import { tick, untrack } from 'svelte';
	import type { CatalogNode, JobGroupMember } from '$lib/api';
	import Button from '$lib/components/ui/button/button.svelte';
	import AssetNode from '$lib/components/lineage/asset-node.svelte';
	import FolderNode from '$lib/components/lineage/folder-node.svelte';
	import TooManyCard from '$lib/components/lineage/too-many-card.svelte';
	import FlowController from '$lib/lineage/flow-controller.svelte';
	import FitControls from '$lib/lineage/fit-controls.svelte';
	import { layoutFolderGraph, tooManyToDraw, withFolderClosed } from '$lib/lineage/folder-graph';
	import { innerId, memberGraph, memberNodeId, type MemberInfo, type MemberSelection, type NodeLink } from '$lib/lineage/member-graph';
	import { toggled } from '$lib/lineage/member-expand';
	import JobBoxNode from './job-box-node.svelte';
	import MemberNode from './member-node.svelte';
	import MemberBoxNode from './member-box-node.svelte';
	import CountEdge from './count-edge.svelte';

	// A job group's member jobs, one card each. Clicking a card selects it and opens the member in
	// place, a box around its nodes, as a folder opens on Lineage; edges follow what is visible at
	// each end. Clicking a node inside selects that node; clicking the canvas clears the selection.
	let {
		members,
		info,
		catalog = [],
		links = new Map(),
		selection = $bindable(null)
	}: {
		members: JobGroupMember[];
		info: ReadonlyMap<string, MemberInfo>;
		catalog?: readonly CatalogNode[];
		links?: ReadonlyMap<string, NodeLink[]>;
		selection?: MemberSelection | null;
	} = $props();

	const nodeTypes = { member: MemberNode, memberBox: MemberBoxNode, asset: AssetNode, folder: FolderNode, folderBox: JobBoxNode };
	const edgeTypes = { count: CountEdge };
	let expanded = $state<ReadonlySet<string>>(new Set());
	let openFolders = $state<ReadonlyMap<string, ReadonlySet<string>>>(new Map());
	/** Selects a member and opens or closes it, from its card or its box's header. */
	function toggle(jobKey: string) {
		selection = { member: jobKey };
		expanded = toggled(expanded, jobKey);
	}
	function setFolders(jobKey: string, next: ReadonlySet<string>) {
		openFolders = new Map([...openFolders, [jobKey, next]]);
	}
	const closeFolder = (jobKey: string, folder: string) => setFolders(jobKey, withFolderClosed(openFolders.get(jobKey) ?? new Set(), folder));
	const openFolder = (jobKey: string, folder: string) => setFolders(jobKey, new Set([...(openFolders.get(jobKey) ?? []), folder]));

	const graph = $derived(memberGraph(members, info, { catalog, links, expanded, openFolders, ontoggle: toggle, oncloseFolder: closeFolder }));
	const tooMany = $derived(tooManyToDraw(graph));
	let nodes = $state.raw<Node[]>([]);
	let edges = $state.raw<Edge[]>([]);
	let fit = $state<((opts?: { duration?: number }) => void) | undefined>();
	let fitted = $state(false);
	let viewport = $state<Viewport>({ x: 0, y: 0, zoom: 1 });
	let setViewport = $state<(v: Viewport) => void>();
	let panel = $state<HTMLDivElement>();
	// The sidebar opening narrows the graph; the zoom stays, and the view pans only when the selection would end up out of view.
	$effect(() => {
		if (!panel) return;
		const ro = new ResizeObserver(() => untrack(keepSelectionInView));
		ro.observe(panel);
		return () => ro.disconnect();
	});
	function keepSelectionInView() {
		const node = nodes.find((n) => n.selected);
		if (!node || !panel || !setViewport || !fitted) return;
		let { x: px, y: py } = node.position;
		for (let parent = node.parentId; parent; ) {
			const p = nodes.find((n) => n.id === parent);
			if (!p) break;
			px += p.position.x;
			py += p.position.y;
			parent = p.parentId;
		}
		const { width, height } = panel.getBoundingClientRect();
		const { x, y, zoom } = viewport;
		const w = (node.measured?.width ?? node.width ?? 236) * zoom;
		const h = (node.measured?.height ?? node.height ?? 61) * zoom;
		const left = px * zoom + x;
		const top = py * zoom + y;
		if (left >= 0 && left + w <= width && top >= 0 && top + h <= height) return;
		setViewport({ zoom, x: (width - w) / 2 - px * zoom, y: (height - h) / 2 - py * zoom });
	}
	let shape = '';
	let laidOut = '';
	const selectedId = $derived(selection ? (selection.node ? innerId(selection.member, selection.node) : memberNodeId(selection.member)) : null);
	const marked = (list: Node[], id: string | null) => list.map((n) => (!!n.selected === (n.id === id) ? n : { ...n, selected: n.id === id }));
	$effect(() => {
		const id = selectedId;
		const current = untrack(() => nodes);
		if (current.some((n) => !!n.selected !== (n.id === id))) nodes = marked(current, id);
	});

	$effect(() => {
		const next = graph;
		if (tooMany) {
			shape = '';
			nodes = [];
			edges = [];
			return;
		}
		const sig = next.nodes.map((n) => `${n.id}@${n.parentId ?? ''}`).join(',');
		if (sig === shape) {
			// Same shape: keep positions and sizes, refresh what the cards say.
			const data = new Map(next.nodes.map((n) => [n.id, n.data]));
			nodes = untrack(() => nodes).map((n) => ({ ...n, data: data.get(n.id) ?? n.data }));
		} else {
			shape = sig;
			laidOut = '';
			nodes = marked(next.nodes, untrack(() => selectedId));
		}
		edges = next.edges;
	});

	// Cards inside a box are measured before the layout sizes the boxes around them.
	$effect(() => {
		const cards = nodes.filter((n) => n.type === 'asset' || n.type === 'folder');
		if (laidOut === shape || !nodes.length || !cards.every((n) => n.measured?.width && n.measured?.height)) return;
		laidOut = shape;
		nodes = layoutFolderGraph(nodes, edges);
		tick().then(() => requestAnimationFrame(() => fit?.()));
	});

	function onnodeclick({ node, event }: { node: Node; event: MouseEvent | TouchEvent }) {
		if ((event.target as Element | null)?.closest('a, button')) return;
		if (node.type === 'member') {
			if (node.data.expandable) toggle(String(node.data.jobKey));
			else selection = { member: String(node.data.jobKey) };
		} else if (node.type === 'memberBox') selection = { member: String(node.data.jobKey) };
		else if (node.type === 'folder') openFolder(String(node.data.member), String(node.data.group));
		else if (node.type === 'asset') selection = { member: String(node.data.member), node: String(node.data.name) };
	}
</script>

<div class="graph sb-flow" bind:this={panel} class:unfitted={!fitted && nodes.length > 0}>
	<SvelteFlow bind:nodes bind:edges {nodeTypes} {edgeTypes} nodesDraggable={false} nodesConnectable={false} elementsSelectable={false} minZoom={0.1} proOptions={{ hideAttribution: true }} {onnodeclick} onpaneclick={() => (selection = null)}>
		<FlowController bind:fitView={fit} bind:fitted bind:viewport bind:setViewport readable />
		<Background gap={64} bgColor="var(--sb-inset)" patternColor="var(--sb-grid)" />
		<FitControls onfit={() => fit?.()} />
	</SvelteFlow>
	{#if tooMany}
		<TooManyCard count={graph.nodes.length}>
			{#snippet actions()}
				{#if expanded.size}<Button variant="outline" size="bar" onclick={() => (expanded = new Set())}>Close every job</Button>{/if}
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
	.graph :global(.svelte-flow__node-folder),
	.graph :global(.svelte-flow__node-asset) {
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
