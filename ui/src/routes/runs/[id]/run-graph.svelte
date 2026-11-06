<script lang="ts">
	import '@xyflow/svelte/dist/style.css';
	import { Background, Controls, SvelteFlow, type Edge, type Node } from '@xyflow/svelte';
	import { untrack } from 'svelte';
	import type { RunEdge, RunNode } from '$lib/api';
	import { layoutGraph } from '$lib/lineage/layout';
	import RunGraphNode from './run-graph-node.svelte';

	let { nodes: runNodes, edges: runEdges, ownRunId }: { nodes: RunNode[]; edges: RunEdge[]; ownRunId: string } = $props();

	const nodeTypes = { run: RunGraphNode };
	const edgeStyle = 'stroke: var(--sb-edge); stroke-width: 1.5;';
	const skipStyle = 'stroke: var(--sb-error); stroke-width: 1.5; stroke-dasharray: 4 3;';

	// Skipped nodes stay in the graph, so the path from a failure to what it blocked is visible.
	function build(): { nodes: Node[]; edges: Edge[] } {
		const byKey = new Map(runNodes.map((n) => [n.nodeKey, n]));
		return {
			nodes: runNodes.map((n) => ({ id: n.nodeKey, type: 'run', position: { x: 0, y: 0 }, data: { node: n, ownRunId } })),
			edges: runEdges.map((e) => {
				const target = byKey.get(e.to);
				const blocked = target?.status === 'skipped' && (byKey.get(e.from)?.status === 'failed' || byKey.get(e.from)?.status === 'skipped');
				return { id: `${e.from}-${e.to}`, source: e.from, target: e.to, type: 'bezier', style: blocked ? skipStyle : edgeStyle };
			})
		};
	}

	let flowNodes = $state.raw<Node[]>([]);
	let flowEdges = $state.raw<Edge[]>([]);
	let signature = '';
	let laidOut = false;

	$effect(() => {
		const next = build();
		const sig = runNodes.map((n) => n.nodeKey).join(',');
		if (sig !== signature) {
			signature = sig;
			laidOut = false;
			flowNodes = next.nodes;
		} else {
			// Same shape: keep positions, refresh statuses.
			const data = new Map(next.nodes.map((n) => [n.id, n.data]));
			flowNodes = untrack(() => flowNodes).map((n) => ({ ...n, data: data.get(n.id) ?? n.data }));
		}
		flowEdges = next.edges;
	});

	$effect(() => {
		if (laidOut || !flowNodes.length) return;
		if (!flowNodes.every((n) => n.measured?.width && n.measured?.height)) return;
		flowNodes = layoutGraph(flowNodes, flowEdges);
		laidOut = true;
	});
</script>

<div class="sb-flow relative flex-1" style="background: var(--sb-inset)">
	<SvelteFlow bind:nodes={flowNodes} bind:edges={flowEdges} {nodeTypes} fitView nodesDraggable proOptions={{ hideAttribution: true }}>
		<Background gap={64} bgColor="var(--sb-inset)" patternColor="var(--sb-grid)" />
		<Controls />
	</SvelteFlow>
</div>

<style>
	.sb-flow :global(.svelte-flow__controls-button) {
		background: var(--card);
		border-bottom: 1px solid var(--border);
	}
	.sb-flow :global(.svelte-flow__controls-button svg) {
		fill: var(--muted-foreground);
	}
</style>
