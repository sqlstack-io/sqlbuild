<script lang="ts">
	import '@xyflow/svelte/dist/style.css';
	import { SvelteFlow, Background, Controls, type Node, type Edge } from '@xyflow/svelte';
	import AssetNode from '$lib/components/lineage/asset-node.svelte';
	import NodeFieldsPopover from '$lib/components/lineage/node-fields-popover.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import FocusIcon from '@lucide/svelte/icons/focus';
	import PanelLeftIcon from '@lucide/svelte/icons/panel-left';
	import PanelRightIcon from '@lucide/svelte/icons/panel-right';
	import SearchIcon from '@lucide/svelte/icons/search';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import RotateCcwIcon from '@lucide/svelte/icons/rotate-ccw';
	import { layoutGraph } from '$lib/lineage/layout';
	import { nodeFields } from '$lib/lineage/node-fields.svelte';
	import { neighbourEdgeIds, tracePath } from '$lib/lineage/trace';
	import { outlineSql } from '$lib/sql/outline';
	import GitCompareIcon from '@lucide/svelte/icons/git-compare';
	import SqlViewer from '$lib/components/code/sql-viewer.svelte';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import InspectorSection from '$lib/components/lineage/inspector-section.svelte';
	import { tick, untrack } from 'svelte';
	import FlowController from '$lib/lineage/flow-controller.svelte';
	import RunPanel from '$lib/run/run-panel.svelte';
	import { page } from '$app/state';
	import { scaleGraph } from '$lib/mock/scale-graph';
	import { layoutDag } from '$lib/lineage/graph-layout';
	import { drawFar, hitTest } from '$lib/lineage/graph-draw';
	import type { Viewport } from '@xyflow/svelte';
	import type { CatalogNode } from '$lib/api';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import BuildSource from '$lib/components/build-area/build-source.svelte';
	import OutcomeStrip from '$lib/components/build-area/outcome-strip.svelte';
	import StaleBadges from '$lib/components/build-area/stale-badges.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { checkOutcomeTone, nodeOutcomeLabel, nodeOutcomeTone, staleReasonDetail, staleReasonLabel } from '$lib/status';
	import { NODE_KINDS, formatCredits, formatMs, formatNumber, kindLine, nodeKindPlural, relationName, runScopePhaseLabel, shortRunId, sqlVersions } from '$lib/build-area/labels';
	import { catalogGraph, cardData } from '$lib/lineage/catalog-graph';

	let runOpen = $state(false);
	let browserOpen = $state(false);
	let inspectorOpen = $state(true);
	let neighbourhoodOnly = $state(false);
	let modelQuery = $state('');

	const nodeTypes = { asset: AssetNode };

	// The graph is the catalog: every node in the current snapshot with its
	// build state. Later refreshes update card data in place so positions,
	// selection and focus survive.
	const catalog = createResource((client, scope) => client.listCatalogNodes(scope), ['runs', 'checks', 'snapshot', 'node_status']);
	const catalogByKey = $derived(new Map((catalog.data ?? []).map((n) => [n.key, n])));
	const initialFocus = page.url.searchParams.get('select');
	let nodes = $state.raw<Node[]>([]);
	let edges = $state.raw<Edge[]>([]);
	$effect(() => {
		const data = catalog.data;
		if (!data || scaleMode) return;
		untrack(() => {
			if (!nodes.length) {
				const graph = catalogGraph(data);
				const focus = data.some((n) => n.key === initialFocus) ? initialFocus : 'fact_orders';
				nodes = graph.nodes.map((n) => (n.id === focus ? { ...n, selected: true } : n));
				edges = graph.edges;
				return;
			}
			const byKey = new Map(data.map((n) => [n.key, n]));
			nodes = nodes.map((n) => {
				const fresh = byKey.get(n.id);
				return fresh ? { ...n, data: { ...cardData(fresh), dimmed: n.data.dimmed } } : n;
			});
		});
	});

	// Scale testing: /lineage?models=2000 swaps in a synthetic warehouse-shaped graph.
	// Positions are precomputed (stand-in for `sqb compile` output) instead of
	// running dagre in the browser, only on-screen cards are rendered, and when
	// zoomed out past the point where cards are legible a canvas draws the graph.
	const scaleModels = Number(page.url.searchParams.get('models') ?? 0);
	const scaleMode = scaleModels > 0;
	const FAR_ZOOM = 0.5;
	let scaleStartId = 'fact_orders';
	const scaleLayout = scaleMode ? (() => {
		const synthetic = scaleGraph(scaleModels);
		const t = performance.now();
		const layout = layoutDag(synthetic.nodes, synthetic.edges);
		const w = window as unknown as { __sbLayouts?: number[] };
		(w.__sbLayouts ??= []).push(performance.now() - t);
		nodes = synthetic.nodes.map((n) => {
			const g = layout.byId.get(n.id)!;
			return { ...n, position: { x: g.x, y: g.y }, hidden: true, initialWidth: 236, initialHeight: 61 };
		});
		scaleStartId =
			(nodes.find((n) => n.data.stale === 'changed' && /^(fct|dim|mart)/.test(String(n.data.name))) ??
				nodes.find((n) => n.data.stale === 'changed') ??
				nodes[nodes.length - 1]).id;
		nodes = nodes.map((n) => (n.id === scaleStartId ? { ...n, selected: true } : n));
		edges = synthetic.edges.map((e) => ({ ...e, hidden: true }));
		return layout;
	})() : null;

	let viewport = $state<Viewport>({ x: 0, y: 0, zoom: 1 });
	let farCanvas = $state<HTMLCanvasElement>();
	let graphPanel = $state<HTMLDivElement>();
	let panelSize = $state({ w: 0, h: 0 });
	const far = $derived(scaleMode && viewport.zoom < FAR_ZOOM);

	// Which cards exist in the page. Zoomed out: none (the canvas draws the graph).
	// Zoomed in: only cards within a quarter screen of the viewport, so cards far away
	// are never mounted (Svelte Flow force-renders any card it has not measured).
	let focusKeep = $state<Set<string> | null>(null);
	$effect(() => {
		if (!scaleLayout) return;
		const isFar = far;
		const { x, y, zoom } = viewport;
		const { w, h } = panelSize;
		const keep = focusKeep;
		untrack(() => {
			const mx = (w * 0.25) / zoom;
			const my = (h * 0.25) / zoom;
			const left = -x / zoom - mx;
			const right = (w - x) / zoom + mx;
			const top = -y / zoom - my;
			const bottom = (h - y) / zoom + my;
			const shown = new Set<string>();
			if (!isFar) {
				for (const g of scaleLayout.nodes) {
					if (keep && !keep.has(g.id)) continue;
					if (g.x + 236 >= left && g.x <= right && g.y + 61 >= top && g.y <= bottom) shown.add(g.id);
				}
			}
			if (nodes.some((n) => !!n.hidden === shown.has(n.id))) {
				nodes = nodes.map((n) => (!!n.hidden === shown.has(n.id) ? { ...n, hidden: !shown.has(n.id) } : n));
			}
			const edgeHidden = (e: Edge) => !(shown.has(e.source) && shown.has(e.target));
			if (edges.some((e) => !!e.hidden !== edgeHidden(e))) {
				edges = edges.map((e) => (!!e.hidden !== edgeHidden(e) ? { ...e, hidden: edgeHidden(e) } : e));
			}
		});
	});

	$effect(() => {
		if (!scaleLayout || !graphPanel) return;
		const ro = new ResizeObserver(([entry]) => {
			panelSize = { w: entry.contentRect.width, h: entry.contentRect.height };
		});
		ro.observe(graphPanel);
		return () => ro.disconnect();
	});

	$effect(() => {
		if (!scaleLayout || !farCanvas) return;
		const dpr = window.devicePixelRatio || 1;
		const { w, h } = panelSize;
		if (farCanvas.width !== Math.round(w * dpr)) farCanvas.width = Math.round(w * dpr);
		if (farCanvas.height !== Math.round(h * dpr)) farCanvas.height = Math.round(h * dpr);
		const ctx = farCanvas.getContext('2d');
		if (!ctx) return;
		if (!far) {
			ctx.clearRect(0, 0, farCanvas.width, farCanvas.height);
			return;
		}
		drawFar(ctx, { layout: scaleLayout, view: viewport, width: w, height: h, dpr, lit: lit?.nodes ?? null, selected: selectedIds });
	});

	// open on the whole graph: Svelte Flow's own initial fit waits for measured cards
	let setFlowViewport = $state<(v: Viewport) => void>();
	let didScaleFit = false;
	function fitScaleGraph() {
		if (!scaleLayout || !setFlowViewport || !panelSize.w) return;
		const pad = 24;
		const zoom = Math.min((panelSize.w - pad * 2) / scaleLayout.width, (panelSize.h - pad * 2) / scaleLayout.height);
		setFlowViewport({ zoom, x: (panelSize.w - scaleLayout.width * zoom) / 2, y: pad });
	}
	$effect(() => {
		if (!scaleMode || didScaleFit || !setFlowViewport || !panelSize.w) return;
		didScaleFit = true;
		fitScaleGraph();
	});

	function onFarClick({ event }: { event: MouseEvent }) {
		if (!far || !scaleLayout || !graphPanel) return;
		const rect = graphPanel.getBoundingClientRect();
		const hit = hitTest(event.clientX - rect.left, event.clientY - rect.top, scaleLayout, viewport);
		nodes = nodes.map((n) => (n.selected === (n.id === hit?.id) ? n : { ...n, selected: n.id === hit?.id }));
	}

	// --- selection + hover driven highlighting ---
	let hoveredId = $state<string | null>(null);

	// ids of currently-selected nodes (Svelte Flow sets node.selected)
	const selectedIds = $derived(new Set(nodes.filter((n) => n.selected).map((n) => n.id)));

	// the lit set: full upstream+downstream path of the selection, or
	// immediate neighbours of the hovered node when nothing is selected.
	const lit = $derived.by(() => {
		if (selectedIds.size > 0) {
			return tracePath(selectedIds, edges);
		}
		if (hoveredId) {
			return { nodes: new Set([hoveredId]), edges: neighbourEdgeIds(hoveredId, edges) };
		}
		return null;
	});

	const EDGE_NEUTRAL = 'stroke: var(--sb-edge); stroke-width: 1.5;';
	const EDGE_ON = 'stroke: var(--primary); stroke-width: 2;';
	const EDGE_DIM = 'stroke: var(--sb-edge-dim); stroke-width: 1.2;';

	// Apply highlight styling back into the bound arrays whenever the lit set
	// changes. Guarded so it doesn't fight drag/selection (only touches style
	// + a dimmed data flag, and only when those actually change).
	$effect(() => {
		const l = lit;
		let edgesChanged = false;
		const nextEdges = edges.map((e) => {
			const style = !l ? EDGE_NEUTRAL : l.edges.has(e.id) ? EDGE_ON : EDGE_DIM;
			const animated = !!l && l.edges.has(e.id) && selectedIds.size > 0;
			if (e.style !== style || e.animated !== animated) edgesChanged = true;
			return { ...e, style, animated };
		});
		if (edgesChanged) edges = nextEdges;

		let nodesChanged = false;
		const filtering = filterActive;
		const nextNodes = nodes.map((n) => {
			const dimmed = (!!l && !l.nodes.has(n.id)) || (filtering && !matchesFilter(n));
			if (n.data.dimmed !== dimmed) nodesChanged = true;
			return dimmed === n.data.dimmed ? n : { ...n, data: { ...n.data, dimmed } };
		});
		if (nodesChanged) nodes = nextNodes;
	});

	// the inspector reflects the current selection
	const selectedNodes = $derived(nodes.filter((n) => n.selected));
	// model names selected in the graph (falls back to the inspector's primary)
	// realistically nobody hand-picks >50; cap the manual selection (use a query beyond that)
	const runSelection = $derived(selectedNodes.slice(0, 50).map((n) => n.data.name as string));
	const primary = $derived(selectedNodes[0] ?? nodes.find((n) => n.id === scaleStartId));
	const primarySlug = $derived(primary?.data.name ?? '');
	// Inspector detail for the primary selection; the scale-test graph is synthetic, so it has none.
	const detail = createResource(
		(client, scope) => (scaleMode || !primarySlug ? Promise.resolve(null) : client.getCatalogNode(scope, String(primarySlug))),
		['runs', 'checks', 'snapshot', 'node_status']
	);
	const d = $derived(detail.data && detail.data.key === primarySlug ? detail.data : null);
	const isSource = $derived(d?.kind === 'source');
	const hasSql = $derived(!!d?.sql);

	// ---- toolbar filters: non-matching nodes are dimmed ----
	let kindFilter = $state('');
	let groupFilter = $state('');
	let ownerFilter = $state('');
	let statusFilter = $state<'' | 'stale' | 'changed' | 'behind' | 'failed'>('');
	const filterActive = $derived(!!(kindFilter || groupFilter || ownerFilter || statusFilter));
	function matchesFilter(n: Node): boolean {
		const c = catalogByKey.get(n.id);
		if (!c) return !filterActive;
		if (kindFilter && c.kind !== kindFilter) return false;
		if (groupFilter && c.group !== groupFilter) return false;
		if (ownerFilter && !c.owner.includes(ownerFilter)) return false;
		return matchesFilterStatus(c, statusFilter);
	}
	function matchesFilterStatus(c: CatalogNode, f: typeof statusFilter): boolean {
		if (f === 'stale') return (c.stale?.length ?? 0) > 0;
		if (f === 'changed') return !!c.stale?.some((r) => r.reason !== 'behind');
		if (f === 'behind') return !!c.stale?.some((r) => r.reason === 'behind');
		if (f === 'failed') return c.lastOutcome === 'failed';
		return true;
	}
	const catalogNodes = $derived<CatalogNode[]>(catalog.data ?? []);
	const kindOptions = $derived(NODE_KINDS.filter((k) => catalogNodes.some((n) => n.kind === k)).map((k) => ({ value: k, label: nodeKindPlural[k], count: catalogNodes.filter((n) => n.kind === k).length })));
	const groupOptions = $derived([...new Set(catalogNodes.flatMap((n) => (n.group === null ? [] : [n.group])))].map((g) => ({ value: g, label: g, count: catalogNodes.filter((n) => n.group === g).length })));
	const ownerOptions = $derived([...new Set(catalogNodes.flatMap((n) => n.owner))].sort().map((o) => ({ value: o, label: o, count: catalogNodes.filter((n) => n.owner.includes(o)).length })));
	const statusOptions = [
		{ value: 'stale' as const, label: 'Stale' },
		{ value: 'changed' as const, label: 'Stale · changed' },
		{ value: 'behind' as const, label: 'Stale · behind' },
		{ value: 'failed' as const, label: 'Last build failed' }
	];

	const groupOf = (node: Node) => String(node.data.group ?? (String(node.data.kind).startsWith('source') ? 'sources' : 'models'));
	let collapsedGroups = $state<Set<string>>(new Set());
	function toggleGroup(g: string) {
		const next = new Set(collapsedGroups);
		next.has(g) ? next.delete(g) : next.add(g);
		collapsedGroups = next;
	}
	const browserNodes = $derived(
		[...nodes]
			.filter((node) => String(node.data.name ?? '').toLowerCase().includes(modelQuery.trim().toLowerCase()))
			.sort((left, right) => String(left.data.name).localeCompare(String(right.data.name)))
	);

	const browserGroups = $derived(
		[...new Set(browserNodes.map(groupOf))]
			.map((g) => ({ name: g, nodes: browserNodes.filter((n) => groupOf(n) === g) }))
			.filter((g) => g.nodes.length)
	);

	// immediate upstream/downstream of the selected model (for the Lineage tab)
	const nameOf = (id: string) => nodes.find((n) => n.id === id)?.data.name ?? id;
	const upstream = $derived(primary ? edges.filter((e) => e.target === primary.id).map((e) => nameOf(e.source)) : []);
	const downstream = $derived(primary ? edges.filter((e) => e.source === primary.id).map((e) => nameOf(e.target)) : []);

	let sqlMode = $state<'raw' | 'compiled'>('raw');
	const sqlRaw = $derived(d?.sql?.raw ?? '');
	let sqlOpen = $state(false);
	let viewerMode = $state<'raw' | 'compiled' | 'diff' | 'drift'>('raw');
	const changed = $derived(d?.stale.find((r) => r.reason === 'changed'));
	const primaryDrift = $derived(
		changed && changed.reason === 'changed' ? (changed.change === 'compiled' ? 'compiled' : 'direct') : d?.stale.some((r) => r.reason === 'changed_upstream') ? 'upstream' : undefined
	);
	const versions = $derived(d ? sqlVersions(d.builds, app.now) : []);
	const upstreamDriftNote = $derived.by(() => {
		const r = d?.stale.find((s) => s.reason === 'changed_upstream');
		return r && r.reason === 'changed_upstream' ? `${r.upstream.join(', ')} changed since this model’s last build.` : undefined;
	});
	const driftLabel: Record<string, string> = {
		direct: 'Definition changed',
		compiled: 'Compiled SQL differs',
		upstream: 'Upstream definition changed'
	};
	const sqlOutline = $derived(outlineSql(sqlRaw));
	function openSql(mode: 'raw' | 'compiled' | 'diff' | 'drift' = sqlMode) {
		viewerMode = mode;
		sqlOpen = true;
	}
	function selectByName(name: string) {
		const id = nodes.find((node) => node.data.name === name)?.id;
		if (id) selectModel(id);
	}

	async function selectModel(id: string) {
		neighbourhoodOnly = false;
		if (scaleLayout) {
			focusKeep = null;
			nodes = nodes.map((node) => (node.selected === (node.id === id) ? node : { ...node, selected: node.id === id }));
			const g = scaleLayout.byId.get(id);
			if (g && setFlowViewport) setFlowViewport({ zoom: 1, x: panelSize.w / 2 - (g.x + 118), y: panelSize.h / 2 - (g.y + 30) });
			return;
		}
		nodes = nodes.map((node) => ({ ...node, hidden: false, selected: node.id === id }));
		edges = edges.map((edge) => ({ ...edge, hidden: false }));
		await tick();
		fitFlow?.({ duration: 220 });
	}

	async function toggleNeighbourhood() {
		const focus = selectedNodes[0];
		if (!focus && !neighbourhoodOnly) return;
		neighbourhoodOnly = !neighbourhoodOnly;
		const keep = new Set<string>(focus ? [focus.id] : []);
		if (neighbourhoodOnly) {
			for (const edge of edges) {
				if (edge.source === focus?.id || edge.target === focus?.id) {
					keep.add(edge.source);
					keep.add(edge.target);
				}
			}
		}
		if (scaleMode) {
			focusKeep = neighbourhoodOnly ? keep : null;
			await tick();
			if (neighbourhoodOnly) fitFlow?.({ duration: 220 });
			return;
		}
		nodes = nodes.map((node) => ({ ...node, hidden: neighbourhoodOnly && !keep.has(node.id) }));
		edges = edges.map((edge) => ({
			...edge,
			hidden: neighbourhoodOnly && (!keep.has(edge.source) || !keep.has(edge.target))
		}));
		await tick();
		fitFlow?.({ duration: 220 });
	}

	// ---- resizable inspector width (transient, like the run-detail gutter) ----
	// Dagster-like: the inspector scales with the viewport (~34%), within bounds.
	const inspectorDefault = () =>
		typeof window === 'undefined' ? 560 : Math.round(Math.min(720, Math.max(460, window.innerWidth * 0.34)));
	let inspectorW = $state(inspectorDefault());
	let inspectorDragging = $state(false);
	function startInspector(e: PointerEvent) {
		inspectorDragging = true;
		(e.target as HTMLElement).setPointerCapture(e.pointerId);
	}
	function onInspector(e: PointerEvent) {
		if (!inspectorDragging) return;
		// dragging the left edge: width grows as the pointer moves left
		inspectorW = Math.min(Math.max(480, window.innerWidth * 0.55), Math.max(380, window.innerWidth - e.clientX));
	}
	function endInspector(e: PointerEvent) {
		if (inspectorDragging) tick().then(() => fitFlow?.({ duration: 200 }));
		inspectorDragging = false;
		try {
			(e.target as HTMLElement).releasePointerCapture(e.pointerId);
		} catch {}
	}

	// Auto-layout runs only when a *layout-relevant* signal changes:
	// Node fields toggling, or node heights changing (re-measure). It must NOT
	// run on plain position changes, otherwise dragging a node would be
	// instantly snapped back by dagre.
	let layoutKey = $derived(
		JSON.stringify(nodeFields.value) +
			'|' +
			nodes.map((n) => `${n.id}:${Math.round(n.measured?.height ?? 0)}`).join(',')
	);

	let lastLayoutKey = '';

	$effect(() => {
		const key = layoutKey; // track only field state + measured heights
		if (scaleMode || key === lastLayoutKey) return;
		const measured = nodes.every((n) => n.measured?.width && n.measured?.height);
		if (!measured) return;
		lastLayoutKey = key;
		const layoutStart = performance.now();
		nodes = layoutGraph(nodes, edges);
		const w = window as unknown as { __sbLayouts?: number[] };
		(w.__sbLayouts ??= []).push(performance.now() - layoutStart);
		tick().then(() => fitFlow?.({ duration: 0 }));
	});

	let fitFlow = $state<((opts?: { duration?: number }) => void) | undefined>();
	// refit the graph when side panels open/close so nodes never hide behind them
	let panelKey = '';
	$effect(() => {
		const key = `${browserOpen}|${inspectorOpen}|${sqlOpen}`;
		if (key === panelKey) return;
		const first = panelKey === '';
		panelKey = key;
		if (!first) tick().then(() => fitFlow?.({ duration: 200 }));
	});
	async function resetLayout() {
		neighbourhoodOnly = false;
		if (scaleLayout) {
			focusKeep = null;
			inspectorW = inspectorDefault();
			fitScaleGraph();
			return;
		}
		nodes = nodes.map((node) => ({ ...node, hidden: false }));
		edges = edges.map((edge) => ({ ...edge, hidden: false }));
		nodes = layoutGraph(nodes, edges);
		inspectorW = inspectorDefault();
		await tick();
		fitFlow?.();
	}
</script>

<AppTopbar title="Lineage">
	<Button variant="outline" size="sm" onclick={() => catalog.refresh()}>↻ Refresh</Button>
	<GatedButton requires="run.adhoc" variant="default" onclick={() => (runOpen = true)}>▷ Execute{runSelection.length ? ` ${runSelection.length}` : ''}</GatedButton>
</AppTopbar>

<RunPanel bind:open={runOpen} selection={runSelection} />

<!-- filter toolbar -->
<div class="lineage-page">
	<section class="lineage-workbench">
<div class="lineage-toolbar">
	<button
		class="icon-toggle"
		class:active={browserOpen}
		onclick={() => (browserOpen = !browserOpen)}
		title={browserOpen ? 'Hide model list' : 'Show model list'}
		aria-label="Toggle model list"
	><PanelLeftIcon size={15} /></button>
	<div class="search-wrap">
		<label class="model-search">
			<SearchIcon size={14} />
			<input bind:value={modelQuery} placeholder="Search models" />
		</label>
		{#if modelQuery.trim() && !browserOpen}
			<div class="search-results">
				{#each browserNodes.slice(0, 8) as node (node.id)}
					<button onclick={() => { selectModel(node.id); modelQuery = ''; }}>
						<i class="browser-status {String(node.data.status)}"></i>
						<span>{node.data.name}</span>
						<small>{String(node.data.kind).split(' · ')[0]}</small>
					</button>
				{:else}
					<div class="browser-empty">No matching models</div>
				{/each}
			</div>
		{/if}
	</div>
	{#if !scaleMode}
		<div class="toolbar-group">
			<ChoiceFilter label="Kind" bind:value={kindFilter} options={kindOptions} />
			<ChoiceFilter label="Group" bind:value={groupFilter} options={groupOptions} />
			<ChoiceFilter label="Owner" bind:value={ownerFilter} options={ownerOptions} />
			<ChoiceFilter label="Status" bind:value={statusFilter} options={statusOptions.map((o) => ({ ...o, count: catalogNodes.filter((c) => matchesFilterStatus(c, o.value)).length }))} />
		</div>
	{/if}
	<span class="toolbar-sep"></span>
	<NodeFieldsPopover />
	<button
		onclick={toggleNeighbourhood}
		class="toolbar-action"
		class:focus-active={neighbourhoodOnly}
		disabled={!neighbourhoodOnly && selectedNodes.length === 0}
		title={!neighbourhoodOnly && selectedNodes.length === 0 ? 'Select a model to focus on it' : 'Show only the selected model and its immediate dependencies'}
	>
		<FocusIcon size={14} /> {neighbourhoodOnly ? 'Show all' : 'Focus'}
	</button>
	<button onclick={resetLayout} class="toolbar-action" title="Reset layout"><RotateCcwIcon size={13} /> Reset</button>
	<div class="status-summary ml-auto">
		{#each [['ok', 'var(--sb-success)', 'last build succeeded'], ['warn', 'var(--sb-warning)', 'last build warned'], ['stale', 'var(--sb-stale)', 'stale'], ['fail', 'var(--sb-error)', 'last build failed']] as [st, c, label] (st)}
			{@const n = nodes.filter((x) => (st === 'stale' ? !!x.data.stale : x.data.status === st)).length}
			{#if n}<span title="{n} {label}"><i style:background={c}></i>{n}</span>{/if}
		{/each}
	</div>
	<button
		class="icon-toggle"
		class:active={inspectorOpen}
		onclick={() => (inspectorOpen = !inspectorOpen)}
		title={inspectorOpen ? 'Hide details' : 'Show details'}
		aria-label="Toggle details"
	><PanelRightIcon size={15} /></button>
</div>

<div class="lineage-stage relative flex min-h-0 flex-1">
	{#if browserOpen}
		<aside class="model-browser">
			<div class="browser-heading">
				<span>Models</span>
				<b>{browserNodes.length}</b>
			</div>
			<div class="browser-list sb-scroll">
				{#each browserGroups as g (g.name)}
					<button class="browser-group" onclick={() => toggleGroup(g.name)} aria-expanded={!collapsedGroups.has(g.name)}>
						<ChevronRightIcon size={13} class={collapsedGroups.has(g.name) ? '' : 'rotate-90'} />
						<span>{g.name}</span><b>{g.nodes.length}</b>
					</button>
					{#if !collapsedGroups.has(g.name)}
						{#each g.nodes as node (node.id)}
							<button
								class="browser-row"
								class:selected={selectedIds.has(node.id)}
								onclick={() => selectModel(node.id)}
								title={String(node.data.name)}
							>
								<i class="browser-status {String(node.data.status)}"></i>
								<span><strong>{node.data.name}</strong></span>
							</button>
						{/each}
					{/if}
				{/each}
				{#if browserNodes.length === 0}
					<div class="browser-empty">No models match “{modelQuery}”.</div>
				{/if}
			</div>
		</aside>
	{/if}
	<!-- graph canvas -->
	<div class="graph-panel sb-flow relative min-w-0 flex-1 overflow-hidden" style="background: var(--sb-inset)" bind:this={graphPanel}>
		<SvelteFlow
			bind:nodes
			bind:edges
			{nodeTypes}
			fitView
			fitViewOptions={{ padding: 0.08, maxZoom: 1, includeHiddenNodes: scaleMode }}
			minZoom={scaleMode ? 0.02 : 0.3}
			onlyRenderVisibleElements={scaleMode}
			onpaneclick={onFarClick}
			elementsSelectable
			nodesDraggable
			multiSelectionKey={['Shift', 'Meta', 'Control']}
			selectionKey="Shift"
			proOptions={{ hideAttribution: true }}
			onnodepointerenter={({ node }) => (hoveredId = node.id)}
			onnodepointerleave={() => (hoveredId = null)}
		>
			<FlowController bind:fitView={fitFlow} bind:viewport bind:setViewport={setFlowViewport} />
			<Background gap={64} bgColor="var(--sb-inset)" patternColor="var(--sb-grid)" />
			<Controls />
		</SvelteFlow>
		{#if scaleMode}
			<canvas class="far-canvas" bind:this={farCanvas} style:width="{panelSize.w}px" style:height="{panelSize.h}px" style:display={far ? 'block' : 'none'}></canvas>
			<div class="scale-hint">{scaleModels.toLocaleString()} models · {far ? 'zoomed out: status overview' : 'cards'} · zoom {Math.round(viewport.zoom * 100)}%</div>
		{/if}
	</div>

	{#if sqlOpen && d?.sql}
		<div class="sql-drawer">
			<SqlViewer name={d.key} raw={d.sql.raw} compiled={d.sql.compiled} bind:mode={viewerMode} {versions} upstreamNote={upstreamDriftNote} onRef={selectByName} onClose={() => (sqlOpen = false)} />
		</div>
	{/if}

	<!-- inspector (resizable) -->
	{#if inspectorOpen}
	<div class="inspector-panel relative shrink-0" style:width="{inspectorW}px" class:select-none={inspectorDragging}>
	<!-- drag handle on the left edge -->
	<div
		class="sb-inspector-handle absolute left-0 top-0 bottom-0 z-30 -ml-1 w-2 cursor-col-resize"
		onpointerdown={startInspector}
		onpointermove={onInspector}
		onpointerup={endInspector}
		role="separator"
		aria-label="Resize inspector"
		title="Drag to resize"
		tabindex="-1"
	></div>
	<aside class="sb-scroll flex h-full w-full flex-col overflow-x-hidden overflow-y-auto border-l border-border bg-card">
		{#if selectedNodes.length > 1}
			<div class="border-b border-border p-4">
				<div class="text-[15px] font-semibold">{selectedNodes.length} models selected</div>
				<div class="text-[var(--sb-text-faint)] mt-1 text-[11.5px]">
					{selectedNodes.map((n) => n.data.name).slice(0, 3).join(', ')}{selectedNodes.length > 3 ? '…' : ''}
				</div>
			</div>
		{:else}
			<div class="inspector-head">
				<div class="flex items-center gap-2">
					<span class="truncate text-[15px] font-semibold">{primary?.data.name}</span>
					{#if d}<a href="/catalog/{primarySlug}" class="head-link ml-auto" title="Open in Catalog">Catalog ↗</a>{/if}
				</div>
				<div class="inspector-meta">
					<span>{d ? kindLine(d) : primary?.data.kind}</span>
					{#if d}<span>{d.group}</span>{#if d.owner.length}<span>{d.owner.join(', ')}</span>{/if}{/if}
				</div>
			</div>
		{/if}
		<div class="insp-body">
			{#if scaleMode}
				<InspectorSection title="Scale test">
					<p class="insp-description">A synthetic graph for layout and rendering at scale; nodes have no catalog detail.</p>
				</InspectorSection>
			{:else if !d}
				<InspectorSection title="Details"><p class="insp-description">{detail.error ?? 'Loading…'}</p></InspectorSection>
			{:else if isSource}
				<InspectorSection title="Source">
					<dl class="kv">
						<div><dt>Relation</dt><dd class="mono">{relationName(d.relation)}</dd></div>
						<div><dt>Freshness</dt><dd>{d.sourceFreshness ? d.sourceFreshness.status : 'no check'}</dd></div>
						<div><dt>Newest data</dt><dd>{d.sourceFreshness?.loadedAt ? `loaded ${formatAgo(d.sourceFreshness.loadedAt, app.now)}` : '—'}</dd></div>
						<div><dt>Thresholds</dt><dd>{d.sourceFreshness ? `warn ${Math.round((d.sourceFreshness.warnAfterSeconds ?? 0) / 60)}m · error ${Math.round((d.sourceFreshness.errorAfterSeconds ?? 0) / 60)}m` : '—'}</dd></div>
					</dl>
				</InspectorSection>
			{:else}
				<InspectorSection title="Last build">
					{#snippet meta()}{#if d.lastBuild?.runId}<a class="section-link" href="/runs/{d.lastBuild.runId}">View run →</a>{/if}{/snippet}
					<div class="status-top">
						{#if d.lastOutcome}<StatusLabel tone={nodeOutcomeTone[d.lastOutcome]} label={nodeOutcomeLabel[d.lastOutcome]} />{:else}<StatusLabel tone="var(--sb-text-faint)" label="Not built" muted />{/if}
						<StaleBadges reasons={d.stale} />
					</div>
					<div class="status-stats">
						<div><span>Duration</span><b>{formatMs(d.lastDurationMs)}</b></div>
						<div><span>Rows</span><b>{formatNumber(d.lastRows)}</b>{#if d.rowsDelta}<em class:up={d.rowsDelta > 0}>{d.rowsDelta > 0 ? '+' : ''}{formatNumber(d.rowsDelta)}</em>{/if}</div>
						<div><span>Checks</span><b>{d.checks.total ? `${d.checks.passing}/${d.checks.total}` : '—'}</b></div>
						{#if d.cost}<div><span>Cost</span><b>{formatCredits(d.cost.perBuild)}</b></div>{/if}
					</div>
					{#if d.recentOutcomes.length}
						<div class="strip-wrap"><OutcomeStrip outcomes={d.recentOutcomes} tones={nodeOutcomeTone} caption="Last {d.recentOutcomes.length} builds" /></div>
					{/if}
					{#each d.stale as r, i (i)}
						<button class="drift-hint {r.reason === 'changed_upstream' ? 'upstream' : r.reason}" onclick={() => (r.reason === 'behind' ? undefined : openSql('drift'))} disabled={r.reason === 'behind' || !hasSql}>
							<GitCompareIcon size={14} />
							<span><b>Stale · {staleReasonLabel(r)}</b> · {staleReasonDetail(r)}</span>
							{#if r.reason !== 'behind' && hasSql}<em>View drift →</em>{/if}
						</button>
					{/each}
					<dl class="kv">
						<div><dt>Last built</dt><dd><BuildSource build={d.lastBuild} /></dd></div>
						{#if d.lastBuild?.jobKey}<div><dt>Job</dt><dd><a class="link" href="/jobs/{d.lastBuild.jobKey}">{d.lastBuild.jobKey}</a></dd></div>{/if}
						{#if d.lastBuild}<div><dt>Started</dt><dd>{formatStamp(d.lastBuild.builtAt)}</dd></div>{/if}
						<div><dt>Relation</dt><dd class="mono">{relationName(d.relation)}</dd></div>
					</dl>
				</InspectorSection>
			{/if}

			{#if d?.description}
				<InspectorSection title="Description">
					<p class="insp-description">{d.description}</p>
				</InspectorSection>
			{/if}

			{#if d?.sql}
				<InspectorSection title="SQL">
					{#snippet meta()}<span>{sqlRaw.split('\n').length} lines</span><button class="section-link" onclick={() => openSql()}>Open →</button>{/snippet}
					<CodePreview file="{primarySlug}.sql" code={d.sql.raw} compiled={d.sql.compiled} bind:mode={sqlMode} maxLines={14} onOpen={() => openSql()} />
				</InspectorSection>
			{:else if d?.code}
				<InspectorSection title="Code">
					<CodePreview file={d.code.file} code={d.code.source} language="text" maxLines={14} />
				</InspectorSection>
			{/if}

			{#if d?.checkResults.length}
				<InspectorSection title="Checks">
					{#snippet meta()}<span>{d.checks.passing} of {d.checks.total} passing</span>{/snippet}
					{#each d.checkResults as c (c.id)}
						<a class="check-row" href="/checks/{c.id}">
							<i style:background={checkOutcomeTone[c.latest?.outcome ?? 'skipped']}></i><code>{c.name}</code>
							<span>{runScopePhaseLabel[c.runScopePhase]}</span>
						</a>
					{/each}
				</InspectorSection>
			{/if}

			{#if d?.columns.length}
				<InspectorSection title="Columns">
					{#snippet meta()}<a class="section-link" href="/catalog/{primarySlug}?tab=columns">All {d.columns.length} →</a>{/snippet}
					<dl class="kv">
						{#each d.columns.slice(0, 6) as c (c.name)}
							<div><dt class="mono fg">{c.name}</dt><dd class="muted">{c.description}</dd><dd class="mono muted type">{c.type}</dd></div>
						{/each}
					</dl>
				</InspectorSection>
			{/if}

			{#if d && !isSource}
				<InspectorSection title="Recent runs">
					{#snippet meta()}<a class="section-link" href="/catalog/{primarySlug}?tab=runs">All →</a>{/snippet}
					{#each d.runs.slice(0, 3) as run, i (i)}
						<a class="mini-run" href={run.runId ? `/runs/${run.runId}` : undefined}>
							<i style:background={nodeOutcomeTone[run.outcome]}></i>
							<span class="mini-run-main"><b>{nodeOutcomeLabel[run.outcome]}</b><span>{run.jobKey ?? (run.runId ? 'started by hand' : `from the command line · ${shortRunId(run.fingerprintRunId ?? '')}`)}</span></span>
							<span class="mini-run-meta">{formatMs(run.durationMs)} · {formatAgo(run.startedAt, app.now)}</span>
							{#if run.message}<em>{run.message}</em>{/if}
						</a>
					{:else}
						<span class="section-meta">No builds recorded in this location.</span>
					{/each}
				</InspectorSection>
			{/if}

			<InspectorSection title="Dependencies">
				{#snippet meta()}<span>{upstream.length} up · {downstream.length} down</span>{/snippet}
				<div class="deps">
					<div>
						<div class="dep-head">Upstream</div>
						{#each upstream as model (model)}
							<button class="dependency-row" onclick={() => selectByName(String(model))}>{model}</button>
						{:else}<span class="section-meta">None</span>{/each}
					</div>
					<div>
						<div class="dep-head">Downstream</div>
						{#each downstream as model (model)}
							<button class="dependency-row" onclick={() => selectByName(String(model))}>{model}</button>
						{:else}<span class="section-meta">None</span>{/each}
					</div>
				</div>
			</InspectorSection>

			{#if d}
				<InspectorSection title="Definition">
					<dl class="kv">
						<div><dt>Kind</dt><dd>{kindLine(d)}</dd></div>
						<div><dt>Unique key</dt><dd class="mono">{sqlOutline.config.find((c) => c.key === 'unique_key')?.value ?? '—'}</dd></div>
						<div><dt>Group</dt><dd>{d.group}</dd></div>
						<div><dt>Owner</dt><dd>{d.owner.join(', ') || '—'}</dd></div>
						<div><dt>Tags</dt><dd>{d.tags.join(', ') || '—'}</dd></div>
						<div><dt>Build version</dt><dd class="mono" title="Fingerprint of the last build in this location">{d.lastBuild?.fingerprint.slice(0, 12) ?? '—'}</dd></div>
					</dl>
				</InspectorSection>
			{/if}
		</div>
	</aside>
	</div>
	{/if}
</div>
	</section>
</div>

<style>
	.far-canvas {
		position: absolute;
		inset: 0;
		z-index: 4;
		pointer-events: none;
	}
	.scale-hint {
		position: absolute;
		right: 12px;
		bottom: 10px;
		z-index: 5;
		font-size: 11px;
		color: var(--sb-text-faint);
		pointer-events: none;
	}
	.lineage-page {
		display: flex;
		min-height: 0;
		flex: 1;
		flex-direction: column;
	}
	.lineage-workbench {
		display: flex;
		min-height: 0;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
		background: var(--card);
	}
	.lineage-stage {
		min-height: 0;
		flex: 1;
		overflow: hidden;
	}
	.lineage-toolbar {
		flex: 0 0 auto;
		gap: 5px;
		padding-top: 7px;
		padding-bottom: 7px;
	}
	.model-search {
		display: flex;
		width: 230px;
		height: 31px;
		align-items: center;
		gap: 7px;
		padding: 0 10px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--sb-text-faint);
	}
	.model-search input {
		min-width: 0;
		flex: 1;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--foreground);
		font-size: 12px;
	}
	.model-search input::placeholder {
		color: var(--sb-text-faint);
	}
	.panel-toggle {
		display: flex;
		height: 31px;
		align-items: center;
		gap: 7px;
		padding: 0 10px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--card);
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.panel-toggle:hover {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	.panel-toggle.active,
	.focus-active {
		border-color: color-mix(in srgb, var(--primary) 28%, var(--border));
		background: var(--sidebar-accent);
		color: var(--primary);
	}
	.toolbar-filter,
	.toolbar-action {
		display: flex;
		height: 29px;
		align-items: center;
		gap: 5px;
		padding: 0 8px;
		border: 1px solid transparent;
		border-radius: 2px;
		background: transparent;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.toolbar-filter:hover,
	.toolbar-action:hover {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	.status-summary {
		display: flex;
		align-items: center;
		gap: 12px;
		font: 11.5px var(--font-sans);
	}
	.status-summary span {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		white-space: nowrap;
	}
	.status-summary i {
		width: 6px;
		height: 6px;
		border-radius: 50%;
	}
	.model-browser {
		display: flex;
		width: 244px;
		flex: 0 0 244px;
		flex-direction: column;
		border-right: 1px solid var(--border);
		background: var(--card);
	}
	.browser-heading {
		display: flex;
		height: 37px;
		align-items: center;
		padding: 0 12px;
		border-bottom: 1px solid var(--border);
		color: var(--sb-text-faint);
		font: 500 11px var(--font-sans);
	}
	.browser-heading b {
		margin-left: auto;
		color: var(--muted-foreground);
		font-weight: 500;
	}
	.browser-list {
		min-height: 0;
		flex: 1;
		overflow-y: auto;
		padding: 6px;
	}
	.browser-row {
		display: flex;
		width: 100%;
		min-height: 43px;
		align-items: center;
		gap: 8px;
		padding: 6px 8px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		text-align: left;
	}
	.browser-row:hover {
		background: var(--sb-hover);
	}
	.browser-row.selected {
		background: var(--sidebar-accent);
	}
	.browser-row > span {
		min-width: 0;
	}
	.browser-row strong,
	.browser-row strong {
		color: var(--foreground);
		font: 500 12.5px var(--font-sans);
	}
	.browser-status {
		width: 7px;
		height: 7px;
		flex: 0 0 7px;
		border-radius: 50%;
		background: var(--sb-text-faint);
	}
	.browser-status.ok { background: var(--sb-success); }
	.browser-status.warn { background: var(--sb-warning); }
	.browser-status.stale { background: var(--sb-stale); }
	.browser-status.fail { background: var(--sb-error); }
	.browser-empty {
		padding: 20px 10px;
		color: var(--sb-text-faint);
		font: 11.5px var(--font-sans);
		text-align: center;
	}
	.inspector-panel {
		height: 100%;
	}
	/* thin styled scrollbars */
	.sb-scroll {
		scrollbar-width: thin;
		scrollbar-color: var(--border-strong) transparent;
	}
	.sb-scroll::-webkit-scrollbar {
		width: 9px;
		height: 9px;
	}
	.sb-scroll::-webkit-scrollbar-track {
		background: transparent;
	}
	.sb-scroll::-webkit-scrollbar-thumb {
		background: var(--border-strong);
		border-radius: 2px;
		border: 2px solid transparent;
		background-clip: content-box;
	}
	.sb-scroll::-webkit-scrollbar-thumb:hover {
		background: var(--muted-foreground);
		background-clip: content-box;
	}
	/* inspector resize handle */
	.sb-inspector-handle::after {
		content: '';
		position: absolute;
		top: 0;
		bottom: 0;
		left: 50%;
		width: 1px;
		transform: translateX(-0.5px);
		background: transparent;
		transition: background 0.12s;
	}
	.sb-inspector-handle:hover::after,
	.sb-inspector-handle:active::after {
		background: var(--primary);
	}
	/* Dark theme for Svelte Flow chrome (controls/minimap) which ship light by default */
	.sb-flow :global(.svelte-flow__controls) {
		box-shadow: var(--sb-elev);
		border: 1px solid var(--border);
		border-radius: 2px;
		overflow: hidden;
	}
	.sb-flow :global(.svelte-flow__controls-button) {
		background: var(--card);
		border-bottom: 1px solid var(--border);
		color: var(--foreground);
		width: 30px;
		height: 30px;
	}
	.sb-flow :global(.svelte-flow__controls-button:hover) {
		background: var(--sb-hover, #1f2532);
	}
	.sb-flow :global(.svelte-flow__controls-button svg) {
		fill: var(--muted-foreground, #8d96ab);
	}
	.sb-flow :global(.svelte-flow__controls-button:hover svg) {
		fill: var(--foreground, #eaecf2);
	}
	.sb-flow :global(.svelte-flow__attribution) {
		display: none;
	}
	/* ---- Dagster-inspired refinement ---- */
	.lineage-page {
		--sb-edge: color-mix(in srgb, var(--foreground) 30%, transparent);
		--sb-edge-dim: color-mix(in srgb, var(--foreground) 9%, transparent);
		font-family: var(--font-sans);
	}
	.lineage-toolbar {
		display: flex;
		height: 46px;
		flex: 0 0 46px;
		align-items: center;
		gap: 4px;
		padding: 0 10px;
		border-bottom: 1px solid var(--border);
		background: var(--card);
	}
	.icon-toggle {
		display: grid;
		width: 30px;
		height: 30px;
		place-items: center;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--muted-foreground);
	}
	.icon-toggle:hover {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	.icon-toggle.active {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	.search-wrap {
		position: relative;
		margin: 0 6px 0 2px;
	}
	.model-search {
		width: 260px;
		height: 30px;
		border-radius: 2px;
		background: var(--background);
	}
	.model-search input {
		font-size: 12.5px;
	}
	.search-results {
		position: absolute;
		z-index: 40;
		top: calc(100% + 4px);
		left: 0;
		width: 320px;
		padding: 4px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--popover, var(--card));
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
	}
	.search-results button {
		display: flex;
		width: 100%;
		align-items: center;
		gap: 8px;
		padding: 6px 8px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--foreground);
		font-size: 12.5px;
		text-align: left;
	}
	.search-results button:hover {
		background: var(--sb-hover);
	}
	.search-results small {
		margin-left: auto;
		color: var(--sb-text-faint);
		font-size: 11px;
	}
	.toolbar-group {
		display: flex;
		gap: 2px;
	}
	.toolbar-sep {
		width: 1px;
		height: 18px;
		margin: 0 6px;
		background: var(--border);
	}
	.toolbar-filter,
	.toolbar-action {
		height: 30px;
		gap: 5px;
		padding: 0 9px;
		border: 0;
		border-radius: 2px;
		font-size: 12.5px;
	}
	.toolbar-filter :global(svg) {
		color: var(--sb-text-faint);
	}
	.focus-active {
		border: 0;
		background: var(--sb-hover);
		color: var(--foreground);
	}
	.status-summary {
		gap: 14px;
		margin-right: 8px;
		color: var(--muted-foreground);
		font: 500 12px var(--font-sans);
		font-variant-numeric: tabular-nums;
	}
	.status-summary i {
		width: 7px;
		height: 7px;
	}
	.model-browser {
		width: 232px;
		flex-basis: 232px;
	}
	.browser-heading {
		font: 600 12px var(--font-sans);
		color: var(--muted-foreground);
	}
	.browser-row {
		min-height: 32px;
		border-radius: 2px;
	}
	.browser-row strong {
		font: 500 12.5px var(--font-sans);
	}
	.inspector-head {
		padding: 14px 16px 12px;
		border-bottom: 1px solid var(--border);
	}
	.inspector-meta span + span::before {
		content: '·';
		margin-right: 12px;
		color: var(--sb-text-faint);
	}
	.inspector-meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 4px 12px;
		margin-top: 6px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.head-link {
		flex: 0 0 auto;
		padding: 3px 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.head-link:hover {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	.check-row {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 5px 0;
		border-bottom: 1px solid var(--border-subtle);
		font-size: 12.5px;
	}
	.check-row:last-child {
		border-bottom: 0;
	}
	.check-row i {
		width: 7px;
		height: 7px;
		flex: 0 0 7px;
		border-radius: 50%;
	}
	.check-row code {
		font: 12px var(--font-mono);
	}
	.check-row span {
		margin-left: auto;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.sb-flow :global(.svelte-flow__controls) {
		box-shadow: none;
		border-radius: 2px;
	}
	.sb-flow :global(.svelte-flow__controls-button) {
		width: 28px;
		height: 28px;
	}
	.insp-body {
		display: flex;
		flex-direction: column;
	}
	.mini-run {
		display: grid;
		grid-template-columns: 8px 1fr auto;
		align-items: center;
		column-gap: 10px;
		padding: 7px 0;
		border-bottom: 1px solid var(--border-subtle);
		font-size: 12.5px;
	}
	.mini-run:last-child {
		border-bottom: 0;
	}
	.mini-run:hover .mini-run-main b {
		color: var(--primary);
	}
	.mini-run i {
		width: 8px;
		height: 8px;
		border-radius: 50%;
	}
	.mini-run-main b {
		margin-right: 8px;
		font-weight: 500;
	}
	.mini-run-main span,
	.mini-run-meta {
		color: var(--muted-foreground);
	}
	.mini-run-meta {
		font-size: 12px;
		font-variant-numeric: tabular-nums;
	}
	.mini-run em {
		grid-column: 2 / 4;
		margin-top: 2px;
		overflow: hidden;
		color: var(--sb-error);
		font-size: 12px;
		font-style: normal;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	/* ---- inspector summary v2 ---- */
	.insp-section {
		padding: 14px 16px;
		border-bottom: 1px solid var(--border);
	}
	.insp-description {
		margin: 0;
		color: var(--muted-foreground);
		font-size: 13px;
		line-height: 1.5;
	}
	.section-meta {
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 400;
	}
	.section-link {
		margin-left: auto;
	}
	.section-link {
		border: 0;
		background: transparent;
		color: var(--primary);
		font-size: 12px;
	}
	.section-link:hover {
		text-decoration: underline;
	}
	.status-top {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 12.5px;
	}
	.status-stats {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(80px, 1fr));
		gap: 12px;
		margin-top: 14px;
	}
	.status-stats span {
		display: block;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.status-stats b {
		font-size: 16px;
		font-weight: 600;
		font-variant-numeric: tabular-nums;
	}
	.status-stats em {
		margin-left: 6px;
		font-size: 11.5px;
		font-style: normal;
	}
	.status-stats em.up {
		color: var(--sb-success);
	}
	.deps {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 16px;
	}
	.dependency-row {
		display: block;
		width: 100%;
		overflow: hidden;
		border: 0;
		background: transparent;
		font-size: 12.5px;
		text-align: left;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.deps .dependency-row {
		padding: 3px 0;
		color: var(--foreground);
	}
	.deps .dependency-row:hover {
		background: transparent;
		color: var(--primary);
	}
	.check-row span {
		font-size: 11.5px;
	}
	/* ---- SQL viewer drawer + SQL tab outline ---- */
	.sql-drawer {
		position: absolute;
		z-index: 30;
		top: 0;
		right: 0;
		bottom: 0;
		width: min(1040px, 66%);
		border-left: 1px solid var(--border);
		box-shadow: -18px 0 40px rgba(0, 0, 0, 0.22);
		animation: drawer-in 0.16s ease-out;
	}
	@keyframes drawer-in {
		from {
			transform: translateX(24px);
			opacity: 0;
		}
	}
	/* ---- inspector v3: collapsible sections, key/value rows ---- */
	.kv {
		margin: 12px 0 0;
	}
	:global(.insp-sec-body > .kv:first-child) {
		margin-top: 0;
	}
	.kv > div {
		display: grid;
		grid-template-columns: 110px minmax(0, 1fr) auto;
		align-items: baseline;
		gap: 12px;
		padding: 6px 0;
		border-top: 1px solid var(--border-subtle);
		font-size: 12.5px;
	}
	.kv > div:first-child {
		border-top: 0;
	}
	.kv dt {
		overflow: hidden;
		color: var(--muted-foreground);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.kv dd {
		min-width: 0;
		margin: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.kv .mono {
		font: 12px var(--font-mono);
	}
	.kv .fg {
		color: var(--foreground);
	}
	.kv .muted,
	.insp-body .muted {
		color: var(--muted-foreground);
	}
	.kv .type {
		text-align: right;
	}
	.kv:has(.type) > div {
		grid-template-columns: 130px minmax(0, 1fr) auto;
	}
	.insp-body .link {
		color: var(--primary);
	}
	.insp-body .link:hover {
		text-decoration: underline;
	}
	.dep-head {
		margin-bottom: 4px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.browser-group {
		display: flex;
		width: 100%;
		align-items: center;
		gap: 6px;
		margin-top: 6px;
		padding: 5px 6px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 600;
		text-transform: capitalize;
	}
	.browser-group:first-child {
		margin-top: 0;
	}
	.browser-group:hover {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	.browser-group b {
		margin-left: auto;
		font-weight: 400;
	}
	.browser-group :global(svg) {
		transition: transform 0.15s;
	}
	.browser-group + .browser-row,
	.browser-row {
		padding-left: 24px;
	}
	.drift-hint {
		display: flex;
		width: 100%;
		align-items: center;
		gap: 9px;
		margin-top: 12px;
		padding: 8px 10px;
		border: 1px solid color-mix(in srgb, var(--sb-stale) 35%, var(--border));
		border-radius: 2px;
		background: transparent;
		color: var(--foreground);
		font-size: 12.5px;
		text-align: left;
	}
	.drift-hint:hover {
		background: var(--sb-hover);
	}
	.drift-hint :global(svg) {
		flex: 0 0 auto;
		color: var(--sb-stale);
	}
	.drift-hint.upstream {
		border-style: dashed;
	}
	.drift-hint.behind {
		border-color: color-mix(in srgb, var(--sb-warning) 35%, var(--border));
	}
	.drift-hint.behind :global(svg) {
		color: var(--sb-warning);
	}
	.drift-hint:disabled {
		cursor: default;
	}
	.drift-hint:disabled:hover {
		background: transparent;
	}
	.strip-wrap {
		margin-top: 14px;
	}
	.status-top {
		flex-wrap: wrap;
	}
	.drift-hint b {
		font-weight: 600;
	}
	.drift-hint em {
		margin-left: auto;
		color: var(--primary);
		font-style: normal;
		white-space: nowrap;
	}
	.toolbar-action:disabled {
		opacity: 0.45;
		pointer-events: none;
	}
	@media (max-width: 1450px) {
		.model-search {
			width: 200px;
		}
	}
</style>
