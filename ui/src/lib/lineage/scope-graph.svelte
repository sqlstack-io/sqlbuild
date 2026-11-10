<script lang="ts">
	import '@xyflow/svelte/dist/style.css';
	import { Background, Controls, SvelteFlow, type Edge, type Node } from '@xyflow/svelte';
	import { tick, untrack } from 'svelte';
	import type { CatalogNode, JobGroupMember } from '$lib/api';
	import AssetNode from '$lib/components/lineage/asset-node.svelte';
	import FlowController from '$lib/lineage/flow-controller.svelte';
	import { jobGraph, layoutJobGraph, type JobBox } from '$lib/lineage/job-graph';
	import JobBoxNode from './job-box-node.svelte';

	// Nodes and the lineage between them; each box, such as a group's member job, is drawn around its nodes.
	// Clicking a node selects it; Shift, Ctrl or Cmd adds to the selection.
	let {
		keys,
		catalog,
		boxes = [],
		members = [],
		onselect
	}: { keys: string[]; catalog: CatalogNode[]; boxes?: JobBox[]; members?: JobGroupMember[]; onselect: (keys: string[]) => void } = $props();

	const nodeTypes = { asset: AssetNode, job: JobBoxNode };
	let nodes = $state.raw<Node[]>([]);
	let edges = $state.raw<Edge[]>([]);
	let fit = $state<((opts?: { duration?: number }) => void) | undefined>();
	let fitted = $state(false);
	let shape = '';
	let laidOut = '';

	/** Clears the selection, as clicking the empty canvas does. */
	export function clear() {
		nodes = nodes.map((n) => (n.selected ? { ...n, selected: false } : n));
		onselect([]);
	}

	$effect(() => {
		const next = jobGraph(keys, catalog, boxes, members);
		const sig = next.nodes.map((n) => `${n.id}@${n.parentId ?? ''}`).join(',');
		if (sig === shape) {
			// Same shape: keep positions and sizes, refresh build states.
			const data = new Map(next.nodes.map((n) => [n.id, n.data]));
			nodes = untrack(() => nodes).map((n) => ({ ...n, data: data.get(n.id) ?? n.data }));
		} else {
			shape = sig;
			laidOut = '';
			nodes = next.nodes;
		}
		edges = next.edges;
	});

	$effect(() => {
		const cards = nodes.filter((n) => n.type !== 'job');
		if (laidOut === shape || !nodes.length || !cards.every((n) => n.measured?.width && n.measured?.height)) return;
		laidOut = shape;
		nodes = layoutJobGraph(nodes, edges);
		// Fit once Svelte Flow has taken the new box sizes.
		tick().then(() => requestAnimationFrame(() => fit?.({ duration: 0 })));
	});
</script>

<div class="graph sb-flow" class:unfitted={!fitted && nodes.length > 0}>
	<SvelteFlow
		bind:nodes
		bind:edges
		{nodeTypes}
		nodesDraggable={false}
		multiSelectionKey={['Meta', 'Control', 'Shift']}
		minZoom={0.1}
		proOptions={{ hideAttribution: true }}
		onselectionchange={({ nodes: picked }) => onselect(picked.filter((n) => n.type === 'asset').map((n) => n.id))}
	>
		<FlowController bind:fitView={fit} bind:fitted />
		<Background gap={64} bgColor="var(--sb-inset)" patternColor="var(--sb-grid)" />
		<Controls showLock={false} />
	</SvelteFlow>
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
