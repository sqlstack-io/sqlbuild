<script lang="ts">
	import '@xyflow/svelte/dist/style.css';
	import { Background, Controls, SvelteFlow, type Edge, type Node } from '@xyflow/svelte';
	import { tick, untrack } from 'svelte';
	import type { CatalogNode, JobDetail, RunRef } from '$lib/api';
	import AssetNode from '$lib/components/lineage/asset-node.svelte';
	import FlowController from '$lib/lineage/flow-controller.svelte';
	import { jobGraph, layoutJobGraph, type JobBox } from '$lib/lineage/job-graph';
	import JobBoxNode from './job-box-node.svelte';

	// The job's nodes and the lineage between them; a group draws each member job as a box around its nodes.
	// Clicking a node selects it; Shift, Ctrl or Cmd adds to the selection.
	let {
		job,
		catalog,
		memberNodes,
		lastRuns,
		onselect
	}: { job: JobDetail; catalog: CatalogNode[]; memberNodes: Map<string, string[]>; lastRuns: Map<string, RunRef | null>; onselect: (keys: string[]) => void } = $props();

	const nodeTypes = { asset: AssetNode, job: JobBoxNode };
	let nodes = $state.raw<Node[]>([]);
	let edges = $state.raw<Edge[]>([]);
	let fit = $state<((opts?: { duration?: number }) => void) | undefined>();
	let shape = '';
	let laidOut = '';

	/** Clears the selection, as clicking the empty canvas does. */
	export function clear() {
		nodes = nodes.map((n) => (n.selected ? { ...n, selected: false } : n));
		onselect([]);
	}

	$effect(() => {
		const boxes: JobBox[] = job.kind === 'group' ? [...(job.groupMembers ?? [])].sort((a, b) => a.stage - b.stage).map((m) => ({ jobKey: m.jobKey, nodes: memberNodes.get(m.jobKey) ?? [], lastRun: lastRuns.get(m.jobKey) })) : [];
		const next = jobGraph(job.resolvedNodes, catalog, boxes, job.groupMembers ?? []);
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

<div class="graph sb-flow">
	<SvelteFlow
		bind:nodes
		bind:edges
		{nodeTypes}
		nodesDraggable={false}
		multiSelectionKey={['Meta', 'Control', 'Shift']}
		minZoom={0.3}
		proOptions={{ hideAttribution: true }}
		onselectionchange={({ nodes: picked }) => onselect(picked.filter((n) => n.type === 'asset').map((n) => n.id))}
	>
		<FlowController bind:fitView={fit} />
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
