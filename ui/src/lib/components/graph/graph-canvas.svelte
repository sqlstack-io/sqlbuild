<script lang="ts">
	import '@xyflow/svelte/dist/style.css';
	import { Controls, SvelteFlow, ViewportPortal, type Edge, type Node, type Viewport } from '@xyflow/svelte';
	import { setContext, untrack } from 'svelte';
	import { COMPACT_TAILS, compactTitleOf, siblingLeads, siblingTails, type CompactTails } from './compact-tails';
	import { unzoom } from '$lib/lineage/hit-area';
	import { obstacles, shareObstacles } from '$lib/lineage/edge-routes';
	import type { ComponentProps, Snippet } from 'svelte';
	import FlowController from '$lib/lineage/flow-controller.svelte';
	import FitControls from '$lib/lineage/fit-controls.svelte';
	import CountEdge from '$lib/lineage/count-edge.svelte';
	import { LABEL_MIN_ZOOM, READABLE_ZOOM, compactLevel } from '$lib/lineage/fit';

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
		fitFloor = READABLE_ZOOM,
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
		/** The lowest zoom an automatic fit goes to: the readable zoom, unless a page draws a very large graph whole. */
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
	// Every graph's edges merge into a trunk before the card they enter, the library's curves included,
	// and run through the gaps between cards rather than across them. Where the cards are is worked out
	// when the nodes change (a layout, a measure, a drop), never as the view pans or zooms.
	const allEdgeTypes = $derived({ bezier: CountEdge, count: CountEdge, ...edgeTypes });
	const cards = $derived(obstacles(nodes));
	shareObstacles(() => cards);
	// Panning and zooming change only the viewport's transform. Everything that depends on the zoom is
	// worked out once the view settles (SETTLE_MS after the last move), so a gesture restyles and
	// repaints nothing per step: while it moves the viewport is its own layer (`moving`), moved and
	// scaled by the compositor, and redrawn sharp once when it settles.
	const SETTLE_MS = 200;
	let settledZoom = $state(1);
	let moving = $state(false);
	let settleTimer: ReturnType<typeof setTimeout> | undefined;
	/** The view the background grid was last drawn for, in flow coordinates. */
	let gridArea = $state({ x: 0, y: 0, width: 0, height: 0 });
	const GRID = 64;
	function settle() {
		moving = false;
		settledZoom = viewport.zoom;
		// The grid covers the view and half a view around it, on the grid's own lines, so it is still
		// there under a pan until the view settles again; it is redrawn only then.
		const width = (element?.clientWidth ?? 0) / viewport.zoom;
		const height = (element?.clientHeight ?? 0) / viewport.zoom;
		const [left, top] = [-viewport.x / viewport.zoom, -viewport.y / viewport.zoom];
		// Redrawn only when the view has come near the edge of what is drawn, or zoomed far out of it.
		const g = gridArea;
		const covered = left >= g.x && top >= g.y && left + width <= g.x + g.width && top + height <= g.y + g.height && g.width <= 4 * width + 2 * GRID;
		if (covered) return;
		const x = Math.floor((left - width / 2) / GRID) * GRID;
		const y = Math.floor((top - height / 2) / GRID) * GRID;
		gridArea = { x, y, width: 2 * width + 2 * GRID, height: 2 * height + 2 * GRID };
	}
	$effect(() => {
		void viewport.x;
		void viewport.y;
		void viewport.zoom;
		untrack(() => {
			clearTimeout(settleTimer);
			// Before the first fit shows the graph, settle at once, so it appears drawn for its zoom.
			if (!fitted) return settle();
			if (!moving) moving = true;
			settleTimer = setTimeout(settle, SETTLE_MS);
		});
	});
	$effect(() => () => clearTimeout(settleTimer));
	// A drag that pans the graph isn't a click: the browser still sends one on release, over whatever
	// card or box header the drag began on, which opened it at the end of a pan. A pointer that moved
	// more than a few pixels while pressed swallows that click.
	const DRAG_PX = 4;
	let press: { x: number; y: number; dragged: boolean } | null = null;
	function onpointerdown(event: PointerEvent) {
		press = { x: event.clientX, y: event.clientY, dragged: false };
	}
	function onpointermove(event: PointerEvent) {
		if (press && !press.dragged && event.buttons && Math.hypot(event.clientX - press.x, event.clientY - press.y) > DRAG_PX) press.dragged = true;
	}
	function onclick(event: MouseEvent) {
		if (press?.dragged) {
			event.stopPropagation();
			event.preventDefault();
		}
		press = null;
	}
	// Semantic zoom: zoomed out past the point where the cards' lines would no longer read, each card
	// shows its compact form instead (CompactFace: a one-line pill, its state's ring and its name), by
	// one class on the canvas; `data-compact` picks the name's fit for the level of zoom, and
	// `--sb-title-unzoom` keeps the pill's size on screen. Cards keep their size, so edges don't move.
	// Card controls grow with `--sb-unzoom` so they keep their size.
	const level = $derived(compactLevel(settledZoom));
	// Each card's pill name: the part of its title none of its siblings ends with. Worked out from the
	// cards' titles and boxes alone, so moving or selecting cards doesn't change it.
	const cardKey = $derived(nodes.map((n) => `${n.id}\t${n.parentId ?? ''}\t${compactTitleOf(n.data as Record<string, unknown>) ?? ''}`).join('\n'));
	const titled = $derived(cardKey ? cardKey.split('\n').map((line) => line.split('\t')).filter(([, , title]) => title).map(([id, parent, title]) => ({ id, parent, title })) : []);
	const tails = $derived(siblingTails(titled));
	const parents = $derived(new Map(titled.map((c) => [c.id, c.parent])));
	// The prefix each pill name shares with a sibling's, dropped first when it doesn't fit.
	const leads = $derived(siblingLeads(titled, tails));
	setContext<CompactTails>(COMPACT_TAILS, { tail: (id) => tails.get(id), parent: (id) => parents.get(id), lead: (id) => leads.get(id) ?? 0 });
	const compact = $derived(level > 0);
	const titleUnzoom = $derived(compact ? 1 / Math.max(settledZoom, LABEL_MIN_ZOOM) : 1);
	const controlScale = $derived(unzoom(settledZoom));
</script>

<div class="graph-canvas sb-flow" class:framed class:compact class:moving class:unfitted={!fitted && nodes.length > 0} data-compact={level || undefined} style:--sb-title-unzoom={titleUnzoom} style:--sb-unzoom={controlScale} bind:this={element} onpointerdowncapture={onpointerdown} onpointermovecapture={onpointermove} onclickcapture={onclick}>
	<SvelteFlow bind:nodes bind:edges edgeTypes={allEdgeTypes} fitView={fitOnInit || undefined} elevateEdgesOnSelect={false} minZoom={0.1} proOptions={{ hideAttribution: true }} {...flow}>
		<FlowController bind:fitView={fit} bind:fitted bind:viewport bind:setViewport {fitFloor} {readable} />
		<!-- The dot grid lives in the viewport, so it moves and scales with the graph's layer. -->
		<ViewportPortal target="back">
			<div class="grid" style:transform="translate({gridArea.x}px, {gridArea.y}px)" style:width="{gridArea.width}px" style:height="{gridArea.height}px"></div>
		</ViewportPortal>
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
	/* Compact cards: the card's lines keep their room, hidden, under its compact face. */
	.compact :global(:is(.svelte-flow__node-asset, .svelte-flow__node-folder, .svelte-flow__node-member) > * > :not(.cface)) {
		visibility: hidden;
	}
	/* Edges, plain, highlighted or selected, are drawn beneath every card, so they never take a card's
	   click. The library raises edges from inside an open box to the box's contents' level, above the
	   cards outside it; cards are put above every edge (and selected nodes are raised by at most about
	   1000 per level of boxes). Open boxes (`parent`) keep the library's order, beneath the edges inside
	   them. An edge is hit only on its stroke. */
	.graph-canvas :global(.svelte-flow__node:not(.parent)) {
		z-index: 100000 !important;
	}
	.graph-canvas :global(.svelte-flow__edges svg) {
		pointer-events: none;
	}
	.graph-canvas :global(.svelte-flow__edge),
	.graph-canvas :global(.svelte-flow__edge path) {
		pointer-events: visibleStroke;
	}
	.moving :global(.svelte-flow__viewport) {
		will-change: transform;
	}
	.grid {
		position: absolute;
		top: 0;
		left: 0;
		background-image: radial-gradient(circle, var(--sb-grid) 1.2px, transparent 1.6px);
		background-position: 32px 32px;
		background-size: 64px 64px;
		pointer-events: none;
	}
	.unfitted :global(.svelte-flow__viewport),
	.unfitted :global(.svelte-flow__controls) {
		/* Not visibility: Svelte Flow sets each measured card visible, which would override it. */
		opacity: 0;
	}
</style>
