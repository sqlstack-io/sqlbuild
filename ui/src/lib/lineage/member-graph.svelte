<script lang="ts">
	import '@xyflow/svelte/dist/style.css';
	import { Background, Controls, SvelteFlow, type Edge, type Node } from '@xyflow/svelte';
	import { tick } from 'svelte';
	import type { JobGroupMember } from '$lib/api';
	import FlowController from '$lib/lineage/flow-controller.svelte';
	import TooManyCard from '$lib/components/lineage/too-many-card.svelte';
	import { MAX_DRAWN } from '$lib/lineage/folder-graph';
	import { memberGraph, type MemberInfo } from '$lib/lineage/member-graph';
	import MemberNode from './member-node.svelte';

	// A job group's member jobs, one card each, with an edge where a member waits for another.
	// Clicking a card opens that job's page, where its own nodes are drawn.
	let { members, info }: { members: JobGroupMember[]; info: ReadonlyMap<string, MemberInfo> } = $props();

	const nodeTypes = { member: MemberNode };
	const tooMany = $derived(members.length > MAX_DRAWN);
	let nodes = $state.raw<Node[]>([]);
	let edges = $state.raw<Edge[]>([]);
	let fit = $state<((opts?: { duration?: number }) => void) | undefined>();
	let fitted = $state(false);
	let shape = '';

	$effect(() => {
		if (tooMany) {
			nodes = [];
			edges = [];
			return;
		}
		const next = memberGraph(members, info);
		nodes = next.nodes;
		edges = next.edges;
		// Cards have a fixed size, so the graph is placed already; fit only when its members change.
		const sig = members.map((m) => m.jobKey).join(',');
		if (sig === shape) return;
		shape = sig;
		tick().then(() => requestAnimationFrame(() => fit?.({ duration: 0 })));
	});
</script>

<div class="graph sb-flow" class:unfitted={!fitted && nodes.length > 0}>
	<SvelteFlow bind:nodes bind:edges {nodeTypes} nodesDraggable={false} nodesConnectable={false} elementsSelectable={false} minZoom={0.1} proOptions={{ hideAttribution: true }}>
		<FlowController bind:fitView={fit} bind:fitted fitFloor={0.3} />
		<Background gap={64} bgColor="var(--sb-inset)" patternColor="var(--sb-grid)" />
		<Controls showLock={false} />
	</SvelteFlow>
	{#if tooMany}<TooManyCard count={members.length} />{/if}
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
	.graph :global(.svelte-flow__node-member) {
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
