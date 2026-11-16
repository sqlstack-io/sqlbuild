<script lang="ts">
	import '@xyflow/svelte/dist/style.css';
	import { Background, Controls, SvelteFlow, type Edge, type Node, type Viewport } from '@xyflow/svelte';
	import type { ComponentProps, Snippet } from 'svelte';
	import FlowController from '$lib/lineage/flow-controller.svelte';
	import FitControls from '$lib/lineage/fit-controls.svelte';
	import CountEdge from '$lib/lineage/count-edge.svelte';

	// Every graph in the UI is drawn here, so they share one canvas: the same background, fit controls
	// and readable fit (a large graph opens on its cards at a zoom where they read, never as dots), and
	// the same hiding of a graph until its first fit, so cards never jump into place. Pages bring their
	// own cards, edges and handlers; a check (no-raw-graph.test.ts) keeps them from drawing the
	// library themselves.
	type FlowProps = Omit<ComponentProps<typeof SvelteFlow>, 'nodes' | 'edges' | 'children' | 'fitView'>;
	let {
		nodes = $bindable([]),
		edges = $bindable([]),
		fit = $bindable(),
		fitOnInit,
		fitted = $bindable(false),
		viewport = $bindable({ x: 0, y: 0, zoom: 1 }),
		setViewport = $bindable(),
		readable = true,
		fitFloor,
		controls = 'fit',
		showLock = false,
		framed = false,
		element = $bindable(),
		edgeTypes = {},
		children,
		...flow
	}: FlowProps & {
		nodes?: Node[];
		edges?: Edge[];
		/** Fits the graph as the fit button does; bound for the page to call after a re-layout. */
		fit?: (opts?: { duration?: number }) => void;
		/** Let the library fit once as it mounts, before the page's own fit. */
		fitOnInit?: boolean;
		fitted?: boolean;
		viewport?: Viewport;
		setViewport?: (v: Viewport) => void;
		/** The readable fit: a graph too large to show whole opens at the readable zoom on its cards. */
		readable?: boolean;
		/** The lowest zoom an automatic fit goes to; the readable zoom by default. */
		fitFloor?: number;
		/** `fit`: zoom buttons with a readable fit; `full`: the library's own controls, for a very large graph drawn whole. */
		controls?: 'fit' | 'full' | 'none';
		showLock?: boolean;
		/** A border around the canvas, for a graph inside a page section. */
		framed?: boolean;
		element?: HTMLDivElement;
		/** Overlays drawn inside the canvas, such as a panel. */
		children?: Snippet;
	} = $props();
	// Every graph's edges merge into a trunk before the card they enter, the library's curves included.
	const allEdgeTypes = $derived({ bezier: CountEdge, count: CountEdge, ...edgeTypes });
</script>

<div class="graph-canvas sb-flow" class:framed class:unfitted={!fitted && nodes.length > 0} bind:this={element}>
	<SvelteFlow bind:nodes bind:edges edgeTypes={allEdgeTypes} fitView={fitOnInit || undefined} minZoom={0.1} proOptions={{ hideAttribution: true }} {...flow}>
		<FlowController bind:fitView={fit} bind:fitted bind:viewport bind:setViewport {fitFloor} {readable} />
		<Background gap={64} bgColor="var(--sb-inset)" patternColor="var(--sb-grid)" />
		{#if controls === 'fit'}<FitControls onfit={() => fit?.()} {showLock} />{:else if controls === 'full'}<Controls {showLock} />{/if}
		{@render children?.()}
	</SvelteFlow>
</div>

<style>
	.graph-canvas {
		position: relative;
		width: 100%;
		height: 100%;
		/* Under a toolbar in a column, the canvas takes the rest of the height. */
		flex: 1 1 0;
		min-height: 0;
		overflow: hidden;
		background: var(--sb-inset);
	}
	.framed {
		border: 1px solid var(--border);
		border-radius: 2px;
	}
	.graph-canvas :global(.svelte-flow__controls-button) {
		background: var(--card);
		border-bottom: 1px solid var(--border);
	}
	.graph-canvas :global(.svelte-flow__controls-button svg) {
		fill: var(--muted-foreground);
	}
	.unfitted :global(.svelte-flow__viewport),
	.unfitted :global(.svelte-flow__controls) {
		/* Not visibility: Svelte Flow sets each measured card visible, which would override it. */
		opacity: 0;
	}
</style>
