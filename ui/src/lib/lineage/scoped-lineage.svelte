<script lang="ts">
	// Focused, model-scoped lineage graph for the catalog detail page.
	// Shows the selected node's neighborhood with adjustable upstream/downstream
	// depth, lets you select nodes, and Execute acts on the current selection.
	import '@xyflow/svelte/dist/style.css';
	import { SvelteFlow, Background, Controls, type Node, type Edge } from '@xyflow/svelte';
	import AssetNode from '$lib/components/lineage/asset-node.svelte';
	import { layoutGraph } from '$lib/lineage/layout';
	import FlowController from '$lib/lineage/flow-controller.svelte';
	import { tick } from 'svelte';

	import { app } from '$lib/state/app.svelte';

	/** A lineage card: `data` is the asset card's data (see catalog-graph.ts). */
	type GraphNode = { id: string; name: string; data: Record<string, unknown> };

	let {
		focal,
		graphNodes,
		graphEdges,
		onExecute
	}: {
		focal: string;
		graphNodes: GraphNode[];
		graphEdges: [string, string][];
		/** Opens the run panel on the selection. */
		onExecute?: (selection: string[]) => void;
	} = $props();

	const nodeTypes = { asset: AssetNode };

	// adjacency (rebuilt if edges change)
	const adjacency = $derived.by(() => {
		const up = new Map<string, string[]>();
		const down = new Map<string, string[]>();
		for (const [s, t] of graphEdges) {
			if (!down.has(s)) down.set(s, []);
			down.get(s)!.push(t);
			if (!up.has(t)) up.set(t, []);
			up.get(t)!.push(s);
		}
		return { up, down };
	});

	let upDepth = $state(1);
	let downDepth = $state(1);

	// how many levels actually exist in each direction, so the steppers stop there
	const levels = (adj: Map<string, string[]>) => {
		const seen = new Set([focal]);
		let frontier = [focal];
		let depth = 0;
		while (true) {
			const next = frontier.flatMap((id) => adj.get(id) ?? []).filter((n) => !seen.has(n) && (seen.add(n), true));
			if (!next.length) return depth;
			depth++;
			frontier = next;
		}
	};
	const maxUp = $derived(levels(adjacency.up));
	const maxDown = $derived(levels(adjacency.down));
	$effect(() => {
		if (upDepth > maxUp) upDepth = maxUp;
		if (downDepth > maxDown) downDepth = maxDown;
	});

	// BFS outward to the configured depth in each direction
	const visible = $derived.by(() => {
		const keep = new Set<string>([focal]);
		const walk = (start: string, adj: Map<string, string[]>, depth: number) => {
			let frontier = [start];
			for (let d = 0; d < depth; d++) {
				const next: string[] = [];
				for (const id of frontier)
					for (const n of adj.get(id) ?? [])
						if (!keep.has(n)) {
							keep.add(n);
							next.push(n);
						}
				frontier = next;
			}
		};
		walk(focal, adjacency.up, upDepth);
		walk(focal, adjacency.down, downDepth);
		return keep;
	});


	let nodes = $state.raw<Node[]>([]);
	let edges = $state.raw<Edge[]>([]);
	let fitFn = $state<((opts?: { duration?: number }) => void) | undefined>();

	// (1) Rebuild the node/edge set ONLY when the visible neighbourhood changes
	// (depth steppers / focal). New nodes start at 0,0; dagre places them in (2).
	let builtKey = '';
	$effect(() => {
		const key = [...visible].sort().join(',');
		if (key === builtKey) return;
		builtKey = key;
		const keep = visible;
		nodes = graphNodes
			.filter((n) => keep.has(n.id))
			.map((n) => ({
				id: n.id,
				type: 'asset',
				position: { x: 0, y: 0 },
				selected: n.id === focal,
				data: { ...n.data }
			}));
		edges = graphEdges
			.filter(([s, t]) => keep.has(s) && keep.has(t))
			.map(([s, t]) => ({ id: `${s}-${t}`, source: s, target: t, type: 'bezier', style: 'stroke: var(--sb-edge); stroke-width: 1.5;' }));
		layoutDone = ''; // force a fresh layout for the new set
	});

	// (2) Same reactive layout pattern as the main /lineage page: re-lay out only
	// when measured heights settle for the current set, guarded by a key so it
	// can't loop and doesn't fight dragging.
	let layoutDone = $state('');
	const layoutKey = $derived(builtKey + '|' + nodes.map((n) => `${n.id}:${Math.round(n.measured?.height ?? 0)}`).join(','));
	$effect(() => {
		const key = layoutKey;
		if (key === layoutDone) return;
		if (nodes.length === 0 || !nodes.every((n) => n.measured?.width && n.measured?.height)) return;
		layoutDone = key;
		nodes = layoutGraph(nodes, edges);
		tick().then(() => fitFn?.({ duration: 200 }));
	});

	// Reset: re-run dagre layout and re-fit (clears manual drags).
	function resetLayout() {
		nodes = layoutGraph(nodes, edges);
		tick().then(() => fitFn?.({ duration: 200 }));
	}

	// Selection is owned by Svelte Flow (node.selected), like the main /lineage
	// page. Derive the selected set + --select expression from the live nodes;
	// fall back to the focal node when nothing is selected.
	const selectedFromGraph = $derived(nodes.filter((n) => n.selected).map((n) => n.id));
	const effectiveSelection = $derived(selectedFromGraph.length ? selectedFromGraph : [focal]);
	const selectExpr = $derived(
		effectiveSelection
			.map((id) => graphNodes.find((n) => n.id === id)?.name ?? id)
			.sort()
			.join(' ')
	);
</script>

<div class="flex h-full min-h-0 flex-col">
	<!-- controls -->
	<div class="flex items-center gap-3 border-b border-border px-4 py-2">
		<div class="flex items-center gap-1.5">
			<span class="text-[var(--sb-text-faint)] text-[12px] font-medium">Upstream</span>
			<div class="bg-secondary flex items-center overflow-hidden rounded-[2px] border border-border">
				<button class="text-muted-foreground hover:text-foreground hover:bg-[var(--sb-hover)] h-6 w-6 text-[13px] disabled:opacity-40 disabled:pointer-events-none" disabled={upDepth <= 0} onclick={() => (upDepth = Math.max(0, upDepth - 1))}>−</button>
				<span class="w-5 text-center text-[11px]">{upDepth}</span>
				<button class="text-muted-foreground hover:text-foreground hover:bg-[var(--sb-hover)] h-6 w-6 text-[13px] disabled:opacity-40 disabled:pointer-events-none" disabled={upDepth >= maxUp} title={upDepth >= maxUp ? 'No more upstream models' : undefined} onclick={() => upDepth++}>+</button>
			</div>
		</div>
		<div class="flex items-center gap-1.5">
			<span class="text-[var(--sb-text-faint)] text-[12px] font-medium">Downstream</span>
			<div class="bg-secondary flex items-center overflow-hidden rounded-[2px] border border-border">
				<button class="text-muted-foreground hover:text-foreground hover:bg-[var(--sb-hover)] h-6 w-6 text-[13px] disabled:opacity-40 disabled:pointer-events-none" disabled={downDepth <= 0} onclick={() => (downDepth = Math.max(0, downDepth - 1))}>−</button>
				<span class="w-5 text-center text-[11px]">{downDepth}</span>
				<button class="text-muted-foreground hover:text-foreground hover:bg-[var(--sb-hover)] h-6 w-6 text-[13px] disabled:opacity-40 disabled:pointer-events-none" disabled={downDepth >= maxDown} title={downDepth >= maxDown ? 'No more downstream models' : undefined} onclick={() => downDepth++}>+</button>
			</div>
		</div>
		<button
			class="text-muted-foreground hover:text-foreground hover:bg-[var(--sb-hover)] rounded-[2px] border border-border px-2 py-1 text-[11px] disabled:opacity-40 disabled:pointer-events-none"
			disabled={upDepth >= maxUp && downDepth >= maxDown}
			onclick={() => {
				upDepth = maxUp;
				downDepth = maxDown;
			}}>All</button
		>
		<button
			class="text-muted-foreground hover:text-foreground hover:bg-[var(--sb-hover)] rounded-[2px] border border-border px-2 py-1 text-[11px]"
			onclick={resetLayout}
			title="Reset layout">⤢ Reset</button
		>

		<div class="ml-auto flex items-center gap-2.5">
			<span class="text-[var(--sb-text-faint)] text-[12px]">--select <span class="text-muted-foreground">{selectExpr}</span></span>
			<button
				class="bg-primary rounded-[2px] px-3 py-1 text-[12px] font-medium text-white disabled:opacity-50"
				disabled={!app.can('run.adhoc')}
				title={app.can('run.adhoc') ? undefined : 'Requires run.adhoc'}
				onclick={() => onExecute?.(effectiveSelection.map((id) => graphNodes.find((n) => n.id === id)?.name ?? id))}>▷ Execute {effectiveSelection.length > 1 ? `(${effectiveSelection.length})` : ''}</button
			>
		</div>
	</div>

	<!-- graph -->
	<div class="sb-flow relative min-h-0 flex-1" style="background: var(--sb-inset)">
		<SvelteFlow
			bind:nodes
			bind:edges
			{nodeTypes}
			fitView
			elementsSelectable
			nodesDraggable
			multiSelectionKey={['Shift', 'Meta', 'Control']}
			selectionKey="Shift"
			proOptions={{ hideAttribution: true }}
		>
			<FlowController bind:fitView={fitFn} />
			<Background gap={64} bgColor="var(--sb-inset)" patternColor="var(--sb-grid)" />
			<Controls />
		</SvelteFlow>
	</div>
</div>

<style>
	.sb-flow :global(.svelte-flow__controls-button) {
		background: var(--card);
		border-bottom: 1px solid var(--border);
	}
	.sb-flow :global(.svelte-flow__controls-button svg) {
		fill: var(--muted-foreground);
	}
	.sb-flow :global(.svelte-flow__attribution) {
		display: none;
	}
</style>
