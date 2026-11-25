<script lang="ts">
	import GraphCanvas from '$lib/components/graph/graph-canvas.svelte';
	import GraphToolbar from '$lib/components/graph/graph-toolbar.svelte';
	import { countBy } from '$lib/list/count-by';
	import SelectionBar from '$lib/components/selection/selection-bar.svelte';
	import { createSelection } from '$lib/state/selection.svelte';
	import { readSelectionParams } from '$lib/selection/command';
	import { isKnownType } from '$lib/selection/syntax';
	import { plural } from '$lib/format/plural';
	import Loading from '$lib/components/loading.svelte';
	import PlayIcon from '@lucide/svelte/icons/play';
	import { to } from '$lib/paths';
	import { goto } from '$app/navigation';
	import '@xyflow/svelte/dist/style.css';
	import { type Node, type Edge } from '@xyflow/svelte';
	import AssetNode from '$lib/components/lineage/asset-node.svelte';
	import FolderNode from '$lib/components/lineage/folder-node.svelte';
	import StubNode from '$lib/components/lineage/stub-node.svelte';
	import XIcon from '@lucide/svelte/icons/x';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import HammerIcon from '@lucide/svelte/icons/hammer';
	import NodeFieldsPopover from '$lib/components/lineage/node-fields-popover.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';
	import PanelLeftIcon from '@lucide/svelte/icons/panel-left';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import { layoutGraph } from '$lib/lineage/layout';
	import { nodeFields } from '$lib/lineage/node-fields.svelte';
	import { tracePath } from '$lib/lineage/trace';
	import { edgeStyle, edgesTouching } from '$lib/lineage/visible-edges';
	import CountEdge from '$lib/lineage/count-edge.svelte';
	import { outlineSql } from '$lib/sql/outline';
	import GitCompareIcon from '@lucide/svelte/icons/git-compare';
	import SqlViewer from '$lib/components/code/sql-viewer.svelte';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import InspectorSection from '$lib/components/lineage/inspector-section.svelte';
	import InspectorHead from '$lib/components/lineage/inspector-head.svelte';
	import ViewSummaryPanel from '$lib/components/lineage/view-summary.svelte';
	import { createDetailsPanel } from '$lib/state/details-panel.svelte';
	import { viewSummary } from '$lib/lineage/view-summary';
	import { tick, untrack } from 'svelte';
	import { READABLE_ZOOM, anchoredViewport, absolutePosition as positionOn } from '$lib/lineage/fit';
	import { CHANGED_PLAN_HREF, changedBuildCounts, changedBuildLabel, planHref } from '$lib/run/command';
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
	import { catalogNodes as sharedCatalog, useCatalogNodes } from '$lib/state/catalog.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { checkOutcomeTone, nodeOutcomeLabel, nodeOutcomeTone, staleReasonDetail, staleReasonLabel } from '$lib/status';
	import { statusLabel } from '$lib/build-area/node-status';
	import { formatCredits, formatMs, formatNumber, kindLine, relationName, runScopePhaseHelp, runScopePhaseLabel, shortName, shortRunId, sqlVersions } from '$lib/build-area/labels';
	import { GROUPED_ABOVE, cardData, cardStatus, folderOf } from '$lib/lineage/catalog-graph';
	import { MAX_DRAWN, expandAll, folderBoxId, folderCardId, opensByFolder, tooManyToDraw, folderTree, layoutFolderGraph, withFolderClosed, withFoldersOpen, type FolderTree } from '$lib/lineage/folder-graph';
	import { FOLDERS_PARAM, NODES_PARAM, foldersParam, foldersToOpen, inSelection, lineageGraph, parseFolders, parseNodes, shownNodes, withFolderAdded, withFolderToggled } from '$lib/lineage/folder-selection';
	import JobBoxNode from '$lib/lineage/job-box-node.svelte';

	let browserOpen = $state(false);
	// Closed until something is selected, then as the person last left it (remembered in this browser).
	const details = createDetailsPanel('lineage', () => selectedNodes.length > 0);
	const inspectorOpen = $derived(details.open);
	let neighbourhoodOnly = $state(false);

	const nodeTypes = { asset: AssetNode, folder: FolderNode, folderBox: JobBoxNode, stub: StubNode };
	const edgeTypes = { count: CountEdge };
	// Automatic fits stop at a zoom where cards stay readable; the fit button and the wheel go further, to the whole graph.
	const FIT_FLOOR = READABLE_ZOOM;

	// The graph is the catalog: every node in the current snapshot with its
	// build state. Later refreshes update card data in place so positions,
	// selection and focus survive.
	const catalog = useCatalogNodes();
	const catalogByKey = $derived(new Map((catalog.data ?? []).map((n) => [n.key, n])));
	// `?focus=key` opens on one node. Older links put the node's key in `select`: a key such as
	// `model:orders` is no selector, so it is read as the focus rather than as a selection.
	const linked = readSelectionParams(page.url.searchParams);
	const legacyFocus = /^[a-z_]+:\S+$/.test(linked.select) && !isKnownType(linked.select.slice(0, linked.select.indexOf(':'))) ? linked.select : null;
	const initialFocus = page.url.searchParams.get('focus') ?? legacyFocus ?? (/^[^\s,~+*:/]+$/.test(linked.select) ? linked.select : null);
	// The selection bar: what `sqb build -s … --exclude …` selects; the rest of the graph is dimmed.
	const selection = createSelection({ url: true, initial: legacyFocus ? { ...linked, select: '' } : linked });
	/** Node names for the selection bar, as a bare name selects them, with their folder. */
	const selectionNames = $derived((catalog.data ?? []).map((n) => ({ value: shortName(n.key), hint: folderOf(n) })));
	let nodes = $state.raw<Node[]>([]);
	let edges = $state.raw<Edge[]>([]);
	// The first graph is built a frame after the listing arrives, so the page and its loading state
	// paint before the cards are created and laid out, which takes a moment on a large project.
	let building = false;
	let initialised = false;
	let fitted = $state(false);
	// A large project opens with each folder as one card; opening a folder draws a box around what it
	// holds. The side list shares the open folders.
	let grouped = $state(false);
	let expanded = $state<ReadonlySet<string>>(new Set());
	const tree = $derived<FolderTree>(folderTree(catalog.data ?? []));
	// The side list's folder tree opens and closes on its own; its chevrons never change what is drawn.
	let treeOpen = $state<ReadonlySet<string>>(new Set());
	// Folders picked in the side list: the canvas draws only them, with stubs for lineage outside them.
	// Kept in the URL so a reload or a shared link shows the same folders.
	let selectedFolders = $state<string[]>([]);
	// Nodes picked elsewhere, such as a provider's code from its Used by tab, drawn instead of folders.
	// Also kept in the URL; picking folders replaces them.
	let pickedNodes = $state<string[]>([]);
	// "Show the N out of date" from the too-many card: only nodes that are out of date.
	let staleOnly = $state(false);
	// "Pick folders" highlights the side list until a folder is picked.
	let pickHighlight = $state(false);
	// Laying out thousands of cards in the browser would hang the page, so past the cap nothing is drawn.
	let tooMany = $state(false);
	let tooManyCount = $state(0);
	const graphOf = (data: CatalogNode[]) =>
		lineageGraph(data, { grouped, open: expanded, selected: selectedFolders, picked: new Set(pickedNodes), staleOnly, tree: folderTree(data), onclose: closeFolder, onadd: addFolder });
	/** Shows the graph, or the cap's message instead of a graph too large to lay out. */
	function draw(graph: { nodes: Node[]; edges: Edge[] }, selected: ReadonlySet<string>) {
		tooMany = tooManyToDraw(graph);
		tooManyCount = graph.nodes.length;
		if (tooMany) {
			pendingCentre = null;
			nodes = [];
			edges = [];
			// The side list can still open and close folders.
			browserOpen = true;
			return;
		}
		nodes = graph.nodes.map((n) => (selected.has(n.id) ? { ...n, selected: true } : n));
		edges = graph.edges;
	}
	$effect(() => {
		const data = catalog.data;
		if (!data || scaleMode) return;
		untrack(() => {
			if (!initialised) {
				if (building) return;
				building = true;
				requestAnimationFrame(() =>
					setTimeout(() => {
						building = false;
						const latest = catalog.data;
						if (!latest || initialised) return;
						initialised = true;
						const focus = latest.find((n) => n.key === initialFocus) ?? null;
						const latestTree = folderTree(latest);
						pickedNodes = parseNodes(page.url.searchParams.get(NODES_PARAM), new Set(latest.map((n) => n.key)));
						selectedFolders = pickedNodes.length ? [] : parseFolders(page.url.searchParams.get(FOLDERS_PARAM), latestTree);
						// The same rule at every scope: drawn by folder when the scope is large.
						grouped = opensByFolder(shownNodes(latest, { selected: selectedFolders, picked: new Set(pickedNodes), tree: latestTree }).length);
						if (selectedFolders.length) {
							treeOpen = new Set(selectedFolders.flatMap((f) => foldersToOpen(latestTree, [f]).filter((open) => open !== f)));
						}
						if (focus) {
							expanded = withFoldersOpen(latestTree, focus.key, expanded);
							treeOpen = withFoldersOpen(latestTree, focus.key, treeOpen);
						}
						draw(graphOf(latest), new Set(focus ? [focus.key] : []));
					})
				);
				return;
			}
			if (tooMany) return;
			const fresh = new Map(graphOf(data).nodes.map((n) => [n.id, n.data]));
			nodes = nodes.map((n) => {
				const card = fresh.get(n.id);
				return card ? { ...n, data: { ...card, dimmed: n.data.dimmed } } : n;
			});
		});
	});

	/** Redraws the graph after folders open or close, keeping the selection; the layout effect then places it. */
	// The view fits the graph only on first load, a scope or filter change, and the Fit button. Opening or
	// closing a folder keeps the clicked box where it was on screen instead (`anchor`).
	let pendingFit = true;
	let anchor: { id: string; before: { x: number; y: number }; view: Viewport } | null = null;
	function regroup(next: { grouped?: boolean; expanded?: ReadonlySet<string>; fit?: boolean }) {
		if (next.fit) pendingFit = true;
		const data = catalog.data;
		if (!data) return;
		grouped = next.grouped ?? grouped;
		expanded = next.expanded ?? expanded;
		const selected = new Set(nodes.filter((n) => n.selected).map((n) => n.id));
		neighbourhoodOnly = false;
		// The card or box under the pointer may be gone, and it never reports the pointer leaving.
		clearPointerState();
		lastLayoutKey = '';
		draw(graphOf(data), selected);
	}
	/** Opens or closes a folder in the graph and the side list; drawing every node leaves the graph as it is. */
	function setOpen(next: ReadonlySet<string>) {
		if (grouped) regroup({ expanded: next });
		else expanded = next;
	}
	/** Keeps the clicked card or box, `from`, on screen where it is once it is redrawn as `to`. */
	function anchorOn(from: string, to: string) {
		const before = positionOn(nodes, from);
		anchor = before ? { id: to, before, view: viewport } : null;
	}
	function openFolder(folder: string) {
		anchorOn(folderCardId(folder), folderBoxId(folder));
		setOpen(new Set([...expanded, folder]));
	}
	function closeFolder(folder: string) {
		anchorOn(folderBoxId(folder), folderCardId(folder));
		setOpen(withFolderClosed(expanded, folder));
	}
	/** Opens or closes a folder in the side list only. */
	function toggleTreeFolder(folder: string) {
		treeOpen = treeOpen.has(folder) ? withFolderClosed(treeOpen, folder) : new Set([...treeOpen, folder]);
	}

	/** Draws only the given folders, or every folder when empty, and keeps the choice in the URL. Picked nodes give way to folders. */
	function setFolderSelection(next: string[]) {
		selectedFolders = next;
		pickedNodes = [];
		pickHighlight = false;
		expandNote = '';
		const url = new URL(page.url);
		const param = foldersParam(next);
		if (param) url.searchParams.set(FOLDERS_PARAM, param);
		else url.searchParams.delete(FOLDERS_PARAM);
		url.searchParams.delete(NODES_PARAM);
		if (url.search !== page.url.search) void goto(url, { replaceState: true, noScroll: true, keepFocus: true });
		// A folder filter is the default view scoped to the folders: their subfolders start closed.
		regroup({ grouped: scopeOpensByFolder(next), expanded: new Set(), fit: true });
	}
	/** Whether the scope draws by folder, as the default view decides for the whole project. */
	function scopeOpensByFolder(selected: string[] = selectedFolders): boolean {
		return opensByFolder(shownNodes(catalogNodes, { selected, picked: new Set(pickedNodes), staleOnly, tree }).length);
	}
	// "Expand all" opens every folder in the scope up to the drawing cap; this says where it stopped.
	let expandNote = $state('');
	function expandEverything() {
		const result = expandAll(scopeTree(), expanded, pickedNodes.length ? [] : selectedFolders);
		expandNote = result.complete ? '' : `Opened ${result.opened} of ${result.total} folders; opening more would draw over ${MAX_DRAWN.toLocaleString('en')} cards`;
		regroup({ expanded: result.open });
	}
	function collapseEverything() {
		expandNote = '';
		regroup({ expanded: new Set() });
	}
	/** The folder tree of what the scope draws, as the graph builds it. */
	function scopeTree(): FolderTree {
		const shown = shownNodes(catalogNodes, { selected: selectedFolders, picked: new Set(pickedNodes), staleOnly, tree });
		return shown.length === catalogNodes.length ? tree : folderTree(shown);
	}
	/** Draws the picked nodes and keeps them in the URL. */
	function setPickedNodes(next: string[]) {
		pickedNodes = next;
		const url = new URL(page.url);
		url.searchParams.set(NODES_PARAM, next.join(','));
		if (url.search !== page.url.search) void goto(url, { replaceState: true, noScroll: true, keepFocus: true });
		regroup({ fit: true });
	}
	/** Adds a folder to what is drawn: a stub's click, Ctrl/⌘-click or a tree checkbox. With picked nodes, a stub adds its folder's nodes to them. */
	function addFolder(folder: string) {
		if (pickedNodes.length) {
			const add = catalogNodes.filter((n) => inSelection(tree, n.key, [folder])).map((n) => n.key);
			setPickedNodes([...new Set([...pickedNodes, ...add])]);
			return;
		}
		setFolderSelection(withFolderAdded(selectedFolders, folder));
	}
	/** A click on a tree folder draws only it; Ctrl/⌘-click adds or removes it. */
	function pickFolder(folder: string, event: MouseEvent) {
		setFolderSelection(event.metaKey || event.ctrlKey ? withFolderToggled(selectedFolders, folder) : [folder]);
	}
	function clearSelection() {
		staleOnly = false;
		setFolderSelection([]);
	}
	function showOutOfDate() {
		staleOnly = true;
		regroup({ grouped: scopeOpensByFolder(), expanded: new Set(), fit: true });
	}
	function pickFolders() {
		browserOpen = true;
		pickHighlight = true;
	}

	// Scale testing: /lineage?models=2000 swaps in a synthetic warehouse-shaped graph.
	// Positions are precomputed (stand-in for `sqb compile` output) instead of
	// running dagre in the browser, only on-screen cards are rendered, and when
	// zoomed out past the point where cards are legible a canvas draws the graph.
	const scaleModels = Number(page.url.searchParams.get('models') ?? 0);
	const scaleMode = scaleModels > 0;
	const FAR_ZOOM = 0.5;
	let scaleStartId = '';
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
	// A card or a closed folder lights its edges once the pointer rests on it, so sweeping across the
	// canvas doesn't flash them; leaving clears at once. An open box only outlines on hover: lighting
	// everything inside a big box was noise. Clicking a box's body selects it and lights its edges.
	const HOVER_REST_MS = 250;
	let hoveredId = $state<string | null>(null);
	let hoverTimer: ReturnType<typeof setTimeout> | null = null;
	/** The open box under the pointer, outlined. */
	let hoveredBox = $state<string | null>(null);
	/** The open box selected by a click on its body. */
	let pickedBox = $state<string | null>(null);

	function clearHoverTimer() {
		if (hoverTimer) clearTimeout(hoverTimer);
		hoverTimer = null;
	}
	/** Forget the pointer and the selected box, as when the graph is redrawn. */
	function clearPointerState() {
		clearHoverTimer();
		hoveredId = null;
		hoveredBox = null;
		pickedBox = null;
	}
	function enterNode(node: Node) {
		clearHoverTimer();
		hoveredId = null;
		if (node.type === 'folderBox') {
			hoveredBox = node.id;
			return;
		}
		hoveredBox = null;
		hoverTimer = setTimeout(() => {
			hoverTimer = null;
			hoveredId = node.id;
		}, HOVER_REST_MS);
	}
	function leaveNode() {
		clearHoverTimer();
		hoveredId = null;
		hoveredBox = null;
	}
	/** A click on an open box's body selects it; its header opens and closes it, and a node a click just removed is ignored. */
	function clickNode({ node, event }: { node: Node; event: MouseEvent | TouchEvent }) {
		if (!nodes.some((n) => n.id === node.id)) return;
		if (node.type === 'folder') {
			pickedBox = null;
			openFolder(String(node.data.group));
			return;
		}
		if (node.type !== 'folderBox') {
			pickedBox = null;
			return;
		}
		if ((event.target as Element | null)?.closest('.folder-head, .head-hit, a, button')) return;
		pickedBox = pickedBox === node.id ? null : node.id;
		if (pickedBox) nodes = nodes.map((n) => (n.selected ? { ...n, selected: false } : n));
	}
	// Whatever replaces the nodes (a folder opening or closing, expand or collapse all, a reset, a new
	// listing) may take the hovered or selected box with it; a stale id would dim the whole graph.
	$effect(() => {
		const ids = new Set(nodes.map((n) => n.id));
		untrack(() => {
			if (hoveredId && !ids.has(hoveredId)) leaveNode();
			if (hoveredBox && !ids.has(hoveredBox)) hoveredBox = null;
			if (pickedBox && !ids.has(pickedBox)) pickedBox = null;
		});
	});
	// Outline the hovered box and mark the selected one on their elements, leaving the node array alone.
	$effect(() => {
		const panel = graphPanel;
		const marks: [string | null, string][] = [
			[hoveredBox, 'box-hovered'],
			[pickedBox, 'box-picked']
		];
		if (!panel) return;
		const undo = marks.map(([id, cls]) => {
			const el = id ? panel.querySelector(`.svelte-flow__node[data-id="${CSS.escape(id)}"]`) : null;
			el?.classList.add(cls);
			return () => el?.classList.remove(cls);
		});
		return () => undo.forEach((u) => u());
	});

	// ids of currently-selected nodes (Svelte Flow sets node.selected)
	const selectedIds = $derived(new Set(nodes.filter((n) => n.selected).map((n) => n.id)));

	// the lit set: full upstream+downstream path of the selection, or
	// immediate neighbours of the hovered node when nothing is selected.
	const lit = $derived.by(() => {
		const l = litSet();
		// A lit set that names nothing drawn would only dim everything.
		const shown = new Set(nodes.filter((n) => !n.hidden).map((n) => n.id));
		return l && [...l.nodes].some((id) => shown.has(id)) ? l : null;
	});
	function litSet(): { nodes: Set<string>; edges: Set<string> } | null {
		if (selectedIds.size > 0) return tracePath(selectedIds, edges);
		if (pickedBox) {
			// A selected box lights every edge of its contents, and both ends of each.
			const touching = edgesTouching(pickedBox, edges, nodes);
			const ends = new Set<string>([pickedBox]);
			for (const e of edges) if (touching.has(e.id)) ends.add(e.source).add(e.target);
			return { nodes: ends, edges: touching };
		}
		if (hoveredId) return { nodes: new Set([hoveredId]), edges: edgesTouching(hoveredId, edges, nodes) };
		return null;
	}


	// Apply highlight styling back into the bound arrays whenever the lit set
	// changes. Guarded so it doesn't fight drag/selection (only touches style
	// + a dimmed data flag, and only when those actually change).
	$effect(() => {
		const l = lit;
		let edgesChanged = false;
		const nextEdges = edges.map((e) => {
			// Thickness keeps saying how many node links an edge stands for, lit or dimmed.
			const count = (e.data as { count?: number } | undefined)?.count ?? 1;
			const style = edgeStyle(count, !l ? 'neutral' : l.edges.has(e.id) ? 'on' : 'dim');
			const animated = !!l && l.edges.has(e.id) && (selectedIds.size > 0 || !!pickedBox);
			if (e.style !== style || e.animated !== animated) edgesChanged = true;
			return { ...e, style, animated };
		});
		if (edgesChanged) edges = nextEdges;

		let nodesChanged = false;
		const filtering = filterActive;
		const nextNodes = nodes.map((n) => {
			// A box only frames its cards, which dim on their own.
			const dimmed = n.type !== 'folderBox' && ((!!l && !l.nodes.has(n.id)) || (filtering && !matchesFilter(n)));
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
	// Without a detail read the inspector shows what the catalog listing reports.
	const detailSupported = app.client.supports('getCatalogNode');
	// The page already holds the listing, so a click reads only the node's own facts. Reloading with
	// each new listing keeps both from one moment, so a change signal reloads each once.
	const detail = createResource((client, scope) => {
		const listing = catalog.data;
		return scaleMode || !primarySlug || !detailSupported || !listing ? Promise.resolve(null) : client.getCatalogNode(scope, String(primarySlug), listing);
	});
	const fresh = $derived(detail.data && detail.data.key === primarySlug ? detail.data : null);
	// A reload of the node already shown keeps its facts until the new answer arrives; another node
	// shows the loading state until its own facts arrive, never the previous node's.
	let held = $state.raw<typeof fresh>(null);
	$effect(() => {
		if (fresh) held = fresh;
	});
	const d = $derived(fresh ?? (held && held.key === primarySlug && !detail.error && !catalog.error ? held : null));
	const listed = $derived(detailSupported ? null : (catalogByKey.get(String(primarySlug)) ?? null));
	const isSource = $derived(d?.kind === 'source');
	const hasSql = $derived(!!d?.sql);
	// Drift compares the current SQL with the SQL of the last build, so it needs both.
	const hasDrift = $derived(hasSql && !!d?.builds.length);

	// ---- toolbar filters: non-matching nodes are dimmed ----
	type StatusFilter = 'stale' | 'changed' | 'behind' | 'failed';
	// Kind and folder are part of the selection (`kind:`, `path:`); Status only changes the view.
	let statusFilter = $state<StatusFilter[]>([]);
	const filterActive = $derived(!!(statusFilter.length || selection.active));
	function matchesFilter(n: Node): boolean {
		if (n.type === 'folder') return catalogNodes.some((c) => `${folderOf(c)}/`.startsWith(`${n.data.group}/`) && matchesCatalog(c));
		const c = catalogByKey.get(n.id);
		return c ? matchesCatalog(c) : !filterActive;
	}
	function matchesCatalog(c: CatalogNode): boolean {
		if (!selection.has(c.key)) return false;
		return !statusFilter.length || statusFilter.some((f) => matchesFilterStatus(c, f));
	}
	function matchesFilterStatus(c: CatalogNode, f: StatusFilter): boolean {
		if (f === 'stale') return c.status === 'changed' || c.status === 'behind';
		if (f === 'changed') return c.status === 'changed';
		if (f === 'behind') return c.status === 'behind';
		return c.status === 'failed';
	}
	const catalogNodes = $derived<CatalogNode[]>(catalog.data ?? []);
	/** Nodes whose own definition changed, which `sqb build --changed` starts from. */
	const changedCount = $derived(catalogNodes.filter((c) => c.stale?.some((r) => r.reason === 'changed')).length);
	/** The location's plan of what changed when it has one, else the definition changes with the downstream count unknown. */
	const changedCounts = $derived(changedBuildCounts(changedCount, sharedCatalog.view.changedBuild));
	const changedLabel = $derived(changedBuildLabel(changedCounts.changed, changedCounts.downstream, 'bar', changedCounts.failing));
	const outOfDateCount = $derived(catalogNodes.filter((c) => (c.stale?.length ?? 0) > 0).length);
	/** The toolbar chip's text, e.g. "orders, inventory"; empty when every folder is drawn. */
	const selectionLabel = $derived.by(() => {
		if (pickedNodes.length) return plural(pickedNodes.length, 'picked node');
		const parts = selectedFolders.length > 3 ? [...selectedFolders.slice(0, 3), `${selectedFolders.length - 3} more`] : [...selectedFolders];
		if (staleOnly) parts.push(`${outOfDateCount.toLocaleString('en')} out of date`);
		return parts.join(', ');
	});
	/** Whether a tree folder is drawn because a folder around it is selected. */
	const inSelectedFolder = (folder: string) => selectedFolders.some((s) => folder.startsWith(`${s}/`));
	const statusOptions: { value: StatusFilter; label: string }[] = [
		{ value: 'stale', label: 'Stale' },
		{ value: 'changed', label: 'Stale · changed' },
		{ value: 'behind', label: 'Stale · behind' },
		{ value: 'failed', label: 'Last build failed' }
	];
	const byStatus = $derived(countBy(catalogNodes, (c) => statusOptions.filter((o) => matchesFilterStatus(c, o.value)).map((o) => o.value)));
	// The sidebar's summary with nothing selected: the nodes drawn, and what narrows or dims them.
	const inView = $derived(viewSummary(shownNodes(catalogNodes, { selected: selectedFolders, picked: new Set(pickedNodes), staleOnly, tree })));
	const activeFilters = $derived(
		[
			pickedNodes.length ? plural(pickedNodes.length, 'picked node') : '',
			!pickedNodes.length && selectedFolders.length ? `Folders: ${selectedFolders.join(', ')}` : '',
			staleOnly ? 'Only nodes out of date' : '',
			selection.active ? `Selection: ${selection.select || 'every node'}${selection.exclude ? ` except ${selection.exclude}` : ''} (others dimmed)` : '',
			statusFilter.length ? `Status: ${statusFilter.map((f) => statusOptions.find((o) => o.value === f)?.label ?? f).join(', ')} (others dimmed)` : ''
		].filter(Boolean)
	);

	// The list names every node, including those inside closed folders.
	const listNodes = $derived<Pick<Node, 'id' | 'data'>[]>(scaleMode ? nodes : catalogNodes.map((n) => ({ id: n.key, data: cardData(n) })));
	const browserNodes = $derived(
		[...listNodes]
			.filter((node) => selection.has(node.id))
			.sort((left, right) => String(left.data.name).localeCompare(String(right.data.name)))
	);

	// The side list is the folder tree, opened and closed together with the graph's folders; with a
	// selection it lists the selected nodes flat, up to a limit, as a project can hold thousands.
	const LIST_LIMIT = 500;
	type BrowserRow =
		| { type: 'folder'; id: string; label: string; depth: number; count: number; open: boolean }
		| { type: 'node'; id: string; depth: number; node: Pick<Node, 'id' | 'data'> };
	const folderCounts = $derived.by(() => {
		const counts = new Map<string, number>();
		const count = (id: string): number => {
			const f = tree.folders.get(id)!;
			const total = f.nodes.length + f.folders.reduce((sum, sub) => sum + count(sub), 0);
			counts.set(id, total);
			return total;
		};
		tree.roots.forEach(count);
		return counts;
	});
	const browserRows = $derived.by<BrowserRow[]>(() => {
		if (scaleMode || selection.active) return browserNodes.slice(0, LIST_LIMIT).map((node) => ({ type: 'node' as const, id: node.id, depth: 0, node }));
		const rows: BrowserRow[] = [];
		const visit = (id: string, depth: number) => {
			const f = tree.folders.get(id)!;
			const open = treeOpen.has(id);
			rows.push({ type: 'folder', id, label: f.label, depth, count: folderCounts.get(id) ?? 0, open });
			if (!open) return;
			for (const sub of f.folders) visit(sub, depth + 1);
			const listed = f.nodes.flatMap((key) => catalogByKey.get(key) ?? []).sort((a, b) => a.key.localeCompare(b.key));
			for (const n of listed) rows.push({ type: 'node', id: n.key, depth: depth + 1, node: { id: n.key, data: cardData(n) } });
		};
		tree.roots.forEach((root) => visit(root, 0));
		return rows;
	});

	// immediate upstream/downstream of the selected model (for the Lineage tab); with folders drawn,
	// from the catalog, as an edge to a closed folder doesn't name the node it reaches
	const nameOf = (id: string) => nodes.find((n) => n.id === id)?.data.name ?? id;
	// A narrowed graph leaves out neighbours and draws stubs in their place, so it lists from the catalog too.
	const fromCatalog = $derived(grouped || selectedFolders.length > 0 || pickedNodes.length > 0 || staleOnly);
	const upstream = $derived(
		!primary ? [] : fromCatalog ? (catalogByKey.get(primary.id)?.upstream ?? []) : edges.filter((e) => e.target === primary.id).map((e) => nameOf(e.source))
	);
	const downstream = $derived(
		!primary
			? []
			: fromCatalog
				? catalogNodes.filter((n) => n.upstream.includes(primary.id)).map((n) => n.key)
				: edges.filter((e) => e.source === primary.id).map((e) => nameOf(e.target))
	);

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
		// A node inside a closed folder isn't drawn; selecting it opens the folders around it.
		const id = nodes.find((node) => node.data.name === name)?.id ?? (grouped && catalogByKey.has(name) ? name : undefined);
		if (id) selectModel(id);
	}

	/**
	 * A node picked in the side list: drawn if it is outside what is shown, then selected and centred,
	 * as Focus does. The node's own folder joins the selection; "out of date only" ends if it is current.
	 */
	async function revealNode(id: string) {
		const listedNode = catalogByKey.get(id);
		if (!scaleMode && listedNode) {
			treeOpen = withFoldersOpen(tree, id, treeOpen);
			const outsideFolders = selectedFolders.length > 0 && !inSelection(tree, id, selectedFolders);
			const outsideStale = staleOnly && !listedNode.stale?.length;
			if (outsideStale) staleOnly = false;
			const own = tree.chains.get(id)?.at(-1);
			if (pickedNodes.length && !pickedNodes.includes(id)) setPickedNodes([...pickedNodes, id]);
			else if (outsideFolders && own) setFolderSelection(withFolderAdded(selectedFolders, own));
			else if (outsideStale) regroup({ fit: true });
		}
		await selectModel(id, true);
	}

	/** Where a card sits on the canvas; a card inside a folder box is placed relative to the box. */
	function absolutePosition(id: string): { x: number; y: number } | null {
		let node = nodes.find((n) => n.id === id);
		if (!node) return null;
		let { x, y } = node.position;
		while (node?.parentId) {
			node = nodes.find((n) => n.id === node!.parentId);
			if (node) {
				x += node.position.x;
				y += node.position.y;
			}
		}
		return { x, y };
	}
	/** A node to centre on once the layout under way has placed it. */
	let pendingCentre: string | null = null;
	function centreOn(id: string): boolean {
		const node = nodes.find((n) => n.id === id);
		const at = absolutePosition(id);
		if (!node || !at || !graphPanel || !setFlowViewport || !node.measured?.width) return false;
		const { width, height } = graphPanel.getBoundingClientRect();
		const zoom = Math.max(viewport.zoom, 0.75);
		const w = node.measured.width;
		const h = node.measured.height ?? 61;
		setFlowViewport({ zoom, x: width / 2 - (at.x + w / 2) * zoom, y: height / 2 - (at.y + h / 2) * zoom });
		return true;
	}

	async function selectModel(id: string, centre = false) {
		neighbourhoodOnly = false;
		if (scaleLayout) {
			focusKeep = null;
			nodes = nodes.map((node) => (node.selected === (node.id === id) ? node : { ...node, selected: node.id === id }));
			const g = scaleLayout.byId.get(id);
			if (g && setFlowViewport) setFlowViewport({ zoom: 1, x: panelSize.w / 2 - (g.x + 118), y: panelSize.h / 2 - (g.y + 30) });
			return;
		}
		const listed = catalogByKey.get(id);
		if (grouped && listed && !nodes.some((n) => n.id === id)) regroup({ expanded: withFoldersOpen(tree, id, expanded) });
		nodes = nodes.map((node) => ({ ...node, hidden: false, selected: node.id === id }));
		edges = edges.map((edge) => ({ ...edge, hidden: false }));
		await tick();
		if (!centre) {
			fitFlow?.({ duration: 220 });
			return;
		}
		// A redrawn graph is still to be laid out; the layout centres on the node once it has placed it.
		if (lastLayoutKey === '' || !centreOn(id)) pendingCentre = id;
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
			// A box's size comes from the layout itself, so measuring it must not start another.
			nodes.flatMap((n) => (n.type === 'folderBox' ? [] : [`${n.id}:${Math.round(n.measured?.height ?? 0)}`])).join(',')
	);

	let lastLayoutKey = '';

	$effect(() => {
		const key = layoutKey; // track only field state + measured heights
		if (scaleMode || !nodes.length || key === lastLayoutKey) return;
		const measured = nodes.every((n) => n.type === 'folderBox' || (n.measured?.width && n.measured?.height));
		if (!measured) return;
		lastLayoutKey = key;
		const layoutStart = performance.now();
		nodes = layout(nodes, edges);
		const w = window as unknown as { __sbLayouts?: number[] };
		(w.__sbLayouts ??= []).push(performance.now() - layoutStart);
		tick().then(() => {
			const centre = pendingCentre;
			pendingCentre = null;
			const held = anchor;
			anchor = null;
			const after = held && positionOn(nodes, held.id);
			if (held && after && setFlowViewport && fitted) setFlowViewport(anchoredViewport(held.view, held.before, after));
			else if (centre && centreOn(centre)) return;
			else if (pendingFit || !fitted) {
				pendingFit = false;
				fitFlow?.({ duration: 0 });
			}
		});
	});

	const layout = (n: Node[], e: Edge[]) => (grouped ? layoutFolderGraph(n, e) : layoutGraph(n, e));
	let fitFlow = $state<((opts?: { duration?: number }) => void) | undefined>();
	// Side panels change the graph's width. The zoom stays where the person left it; the view only pans when
	// the selected node would otherwise end up behind a panel.
	let panelKey = '';
	$effect(() => {
		const key = `${browserOpen}|${inspectorOpen}|${sqlOpen}`;
		if (key === panelKey) return;
		const first = panelKey === '';
		panelKey = key;
		if (!first) tick().then(keepSelectionInView);
	});
	function keepSelectionInView() {
		const node = nodes.find((n) => n.selected);
		if (!node || !graphPanel || !setFlowViewport) return;
		// The SQL drawer lies over the graph's right side, so the visible graph ends where it starts.
		const panel = graphPanel.getBoundingClientRect();
		const drawer = graphPanel.parentElement?.querySelector('.sql-drawer')?.getBoundingClientRect();
		const width = Math.min(panel.right, drawer?.left ?? Infinity) - panel.left;
		const height = panel.height;
		const { x, y, zoom } = viewport;
		const w = (node.measured?.width ?? 236) * zoom;
		const h = (node.measured?.height ?? 61) * zoom;
		const left = node.position.x * zoom + x;
		const top = node.position.y * zoom + y;
		if (left >= 0 && left + w <= width && top >= 0 && top + h <= height) return;
		setFlowViewport({ zoom, x: (width - w) / 2 - node.position.x * zoom, y: (height - h) / 2 - node.position.y * zoom });
	}
	async function resetLayout() {
		neighbourhoodOnly = false;
		if (scaleLayout) {
			focusKeep = null;
			inspectorW = inspectorDefault();
			fitScaleGraph();
			return;
		}
		if (grouped && expanded.size) regroup({ expanded: new Set() });
		nodes = nodes.map((node) => ({ ...node, hidden: false }));
		edges = edges.map((edge) => ({ ...edge, hidden: false }));
		nodes = layout(nodes, edges);
		inspectorW = inspectorDefault();
		await tick();
		fitFlow?.();
	}
</script>

<AppTopbar title="Lineage">
	<RefreshButton onclick={() => catalog.refresh()} />
	{#if app.client.supports('launchAdhocRun')}
		{#if changedCounts.changed}<GatedButton size="bar" requires="run.adhoc" variant="stale" hint={changedLabel.tooltip} onclick={() => goto(to(CHANGED_PLAN_HREF))}
				><HammerIcon /><span>{changedLabel.label}{#if changedLabel.extra}<span class="label-extra">{changedLabel.extra}</span>{/if}</span></GatedButton
			>{/if}
		<GatedButton size="bar" requires="run.adhoc" variant="default" onclick={() => goto(to(planHref(runSelection)))}><PlayIcon class="size-3.5 fill-current stroke-none" />Execute{runSelection.length ? ` ${runSelection.length}` : ''}</GatedButton>
	{/if}
</AppTopbar>


<!-- filter toolbar -->
<div class="lineage-page">
	<section class="lineage-workbench">
<SelectionBar {selection} names={selectionNames} total={catalogNodes.length}>
	{#snippet leading()}
		<Button variant="ghost" size="icon-bar" aria-pressed={browserOpen} onclick={() => (browserOpen = !browserOpen)} title={browserOpen ? 'Hide model list' : 'Show model list'} aria-label="Toggle model list"><PanelLeftIcon /></Button>
	{/snippet}
	{#snippet view()}
		{#if !scaleMode}<ChoiceFilter multiple label="Status" bind:values={statusFilter} options={statusOptions.map((o) => ({ ...o, count: byStatus.get(o.value) ?? 0 }))} />{/if}
	{/snippet}
</SelectionBar>
<GraphToolbar
	folders={scaleMode ? undefined : { on: grouped, toggle: () => regroup({ grouped: !grouped, expanded: new Set(), fit: true }) }}
	expand={!scaleMode && grouped && !tooMany
		? { all: expandEverything, collapse: collapseEverything, canCollapse: expanded.size > 0, allTitle: `Open every folder in view, up to ${MAX_DRAWN.toLocaleString('en')} cards`, collapseTitle: 'Close every folder in view', note: expandNote }
		: undefined}
	focus={{ on: neighbourhoodOnly, can: selectedNodes.length > 0, toggle: toggleNeighbourhood, title: 'Show only the selected model and its immediate dependencies' }}
	onreset={resetLayout}
	details={{ open: inspectorOpen, toggle: details.toggle }}
>
	{#snippet middle()}
		{#if !scaleMode}
			{#if selectionLabel}
				<span class="selection-chip" title="Showing {selectionLabel}">
					<span>Showing: {selectionLabel}</span>
					<Button variant="ghost" size="icon-xs" class="rounded-full" onclick={clearSelection} aria-label="Show every folder" title="Show every folder"><XIcon /></Button>
				</span>
			{/if}
		{/if}
		<NodeFieldsPopover />
	{/snippet}
	{#snippet end()}
		<div class="status-summary">
			{#each [['ok', 'var(--sb-success)', 'last build succeeded'], ['warn', 'var(--sb-warning)', 'last build warned'], ['stale', 'var(--sb-stale)', 'stale'], ['fail', 'var(--sb-error)', 'last build failed']] as [st, c, label] (st)}
				{@const n = scaleMode ? nodes.filter((x) => (st === 'stale' ? !!x.data.stale : x.data.status === st)).length : catalogNodes.filter((c) => (st === 'stale' ? !!c.stale?.length : cardStatus(c) === st)).length}
				{#if n}<span title="{n} {label}"><i style:background={c}></i>{n}</span>{/if}
			{/each}
		</div>
	{/snippet}
</GraphToolbar>

<div class="lineage-stage relative flex min-h-0 flex-1">
	{#if browserOpen}
		<aside class="model-browser" class:pick={pickHighlight}>
			<div class="browser-heading">
				<span>Nodes</span>
				<b>{browserNodes.length}</b>
			</div>
			<div class="browser-list sb-scroll">
				{#each browserRows as row (`${row.type}:${row.id}`)}
					{#if row.type === 'folder'}
						{@const picked = selectedFolders.includes(row.id)}
						<div class="browser-group" class:picked class:shown={!picked && inSelectedFolder(row.id)} style:padding-left="{2 + row.depth * 14}px">
							<!-- raw-button: the tree's expand control; it only opens or closes the folder -->
							<button class="tree-chevron" onclick={() => toggleTreeFolder(row.id)} aria-expanded={row.open} aria-label="{row.open ? 'Collapse' : 'Expand'} {row.id}"><ChevronRightIcon size={13} class={row.open ? 'rotate-90' : ''} /></button>
							<!-- raw-button: a folder's row in the tree, clickable as a whole to show that folder -->
							<button class="tree-label" onclick={(event) => pickFolder(row.id, event)} title="Show only {row.id}; Ctrl/⌘-click to add it"><span>{row.label}</span><b>{row.count}</b></button>
							<input type="checkbox" class="sb-check tree-check" checked={picked} onchange={() => setFolderSelection(withFolderToggled(selectedFolders, row.id))} aria-label="Add {row.id} to the folders shown" />
						</div>
					{:else}
						<!-- raw-button: a node's row in the tree, clickable as a whole to select it -->
						<button
							class="browser-row"
							style:padding-left="{24 + row.depth * 14}px"
							class:selected={selectedIds.has(row.id)}
							onclick={() => revealNode(row.id)}
							title={String(row.node.data.name)}
						>
							<i class="browser-status {String(row.node.data.status)}"></i>
							<span><strong>{row.node.data.label ?? row.node.data.name}</strong></span>
						</button>
					{/if}
				{/each}
				{#if browserNodes.length === 0}
					<div class="browser-empty">{selection.active ? 'No nodes in the selection.' : 'No nodes.'}</div>
				{:else if selection.active && browserNodes.length > LIST_LIMIT}
					<div class="browser-empty">First {LIST_LIMIT} of {browserNodes.length} matches</div>
				{/if}
			</div>
		</aside>
	{/if}
	<!-- graph canvas -->
	<div class="graph-panel sb-flow relative min-w-0 flex-1 overflow-hidden" style="background: var(--sb-inset)" bind:this={graphPanel}>
		<GraphCanvas
			bind:nodes
			bind:edges
			bind:fit={fitFlow}
			fitOnInit
			bind:viewport
			bind:setViewport={setFlowViewport}
			bind:fitted
			fitFloor={scaleMode ? undefined : FIT_FLOOR}
			readable={!scaleMode}
			controls={scaleMode ? 'full' : 'fit'}
			showLock
			{nodeTypes}
			{edgeTypes}
			fitViewOptions={{ padding: 0.08, maxZoom: 1, minZoom: scaleMode ? undefined : FIT_FLOOR, includeHiddenNodes: scaleMode }}
			minZoom={scaleMode ? 0.02 : 0.05}
			onlyRenderVisibleElements={scaleMode}
			onpaneclick={(event) => {
				pickedBox = null;
				onFarClick(event);
			}}
			elementsSelectable
			nodesDraggable
			multiSelectionKey={['Shift', 'Meta', 'Control']}
			selectionKey="Shift"
			onnodeclick={clickNode}
			onnodepointerenter={({ node }) => enterNode(node)}
			onnodepointerleave={leaveNode}
		/>
		{#if tooMany}
			<div class="graph-too-many">
				<div class="too-many-card" role="status">
					<strong>Too many nodes to draw at once</strong>
					<p>{tooManyCount.toLocaleString('en')} cards · limit {MAX_DRAWN.toLocaleString('en')}</p>
					<div class="too-many-actions">
						{#if !grouped}<Button variant="outline" size="bar" onclick={() => regroup({ grouped: true, expanded: new Set(), fit: true })}><FolderIcon size={13} />Show folders</Button>{/if}
						{#if !staleOnly && outOfDateCount > 0 && outOfDateCount <= MAX_DRAWN}<Button variant="outline" size="bar" onclick={showOutOfDate}>Show the {outOfDateCount.toLocaleString('en')} out of date</Button>{/if}
						<Button variant="outline" size="bar" onclick={pickFolders}>Pick folders</Button>
					</div>
				</div>
			</div>
		{:else if !scaleMode && !fitted && !catalog.error && catalog.data?.length !== 0}
			<div class="graph-loading"><div><Loading lines={4} /></div></div>
		{/if}
		{#if scaleMode}
			<canvas class="far-canvas" bind:this={farCanvas} style:width="{panelSize.w}px" style:height="{panelSize.h}px" style:display={far ? 'block' : 'none'}></canvas>
			<div class="scale-hint">{scaleModels.toLocaleString()} models · {far ? 'zoomed out: status overview' : 'cards'} · zoom {Math.round(viewport.zoom * 100)}%</div>
		{/if}
	</div>

	{#if sqlOpen && d?.sql}
		<div class="sql-drawer">
			<SqlViewer name={shortName(d.key)} raw={d.sql.raw} compiled={d.sql.compiled} bind:mode={viewerMode} {versions} upstreamNote={upstreamDriftNote} onRef={selectByName} onClose={() => (sqlOpen = false)} />
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
		{:else if primary}
			<InspectorHead title={String(primary?.data.label ?? primary?.data.name ?? '')} hint={String(primary?.data.name ?? '')} href={d ? to(`/catalog/${primarySlug}`) : undefined} linkLabel="Catalog ↗" linkTitle="Open in Catalog">
				{#snippet meta()}
					<!-- From the selected card, so the header names the node at once, before its detail arrives. -->
					<span>{primary?.data.kind}</span>
					{#if primary?.data.group}<span>{primary.data.group}</span>{/if}
				{/snippet}
			</InspectorHead>
		{/if}
		<div class="insp-body">
			{#if !scaleMode && !primary}
				<ViewSummaryPanel summary={inView} filters={activeFilters} />
			{:else}
			{#if scaleMode}
				<InspectorSection title="Scale test">
					<p class="insp-description">A synthetic graph for layout and rendering at scale; nodes have no catalog detail.</p>
				</InspectorSection>
			{:else if listed}
				<InspectorSection title="Details">
					<dl class="kv">
						<div><dt>Kind</dt><dd>{kindLine(listed)}</dd></div>
						{#if listed.relation}<div><dt>Relation</dt><dd class="mono">{relationName(listed.relation)}</dd></div>{/if}
						{#if listed.kind !== 'source'}<div><dt>Last built</dt><dd>{#if listed.lastBuild}<BuildSource build={listed.lastBuild} />{:else if listed.statusAt}{formatStamp(listed.statusAt)}{:else}No build recorded{/if}</dd></div>{/if}
						{#if listed.tags.length}<div><dt>Tags</dt><dd>{listed.tags.join(', ')}</dd></div>{/if}
						{#if listed.path}<div><dt>File</dt><dd class="mono">{listed.path}</dd></div>{/if}
					</dl>
				</InspectorSection>
			{:else if !d}
				<!-- Titled like the section the facts fill, so it does not switch title when they arrive. -->
				<InspectorSection title={catalogByKey.get(String(primarySlug))?.kind === 'source' ? 'Source' : 'Last build'}>
					{#snippet meta()}{#if !(detail.error ?? catalog.error) && detail.loading}<span class="loading-meta"><LoaderIcon size={13} class="spin" />Loading</span>{/if}{/snippet}
					{#if detail.error ?? catalog.error}<p class="insp-description">{detail.error ?? catalog.error}</p>
					{:else if !detail.loading}<p class="insp-description">This node is no longer in the project.</p>
					{:else}
						<!-- Shaped like the facts it stands in for, so they replace it without the panel jumping. -->
						<div class="status-stats" role="status" aria-label="Loading">
							{#each [0, 1, 2] as i (i)}<div><span class="sb-skeleton stat-label"></span><span class="sb-skeleton stat-value"></span></div>{/each}
						</div>
						<span class="sb-skeleton strip-bar"></span>
						<dl class="kv">
							{#each [64, 48, 56] as w (w)}<div><dt><span class="sb-skeleton" style:width="70%"></span></dt><dd><span class="sb-skeleton" style:width="{w}%"></span></dd></div>{/each}
						</dl>
					{/if}
				</InspectorSection>
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
					{#snippet meta()}{#if d.lastBuild?.runId}<a class="section-link" href={to(`/runs/${d.lastBuild.runId}`)}>View run →</a>{/if}{/snippet}
					<div class="status-top">
						{#if statusLabel(d)}{@const s = statusLabel(d)!}<StatusLabel tone={s.tone} label={s.label} muted={d.status === 'not_built'} />{/if}
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
						<!-- raw-button: a stale notice that is clickable as a whole, opening its drift -->
						<button class="drift-hint {r.reason === 'changed_upstream' ? 'upstream' : r.reason}" onclick={() => (r.reason === 'behind' ? undefined : openSql('drift'))} disabled={r.reason === 'behind' || !hasDrift}>
							<GitCompareIcon size={14} />
							<span><b>Stale · {staleReasonLabel(r)}</b> · {staleReasonDetail(r)}</span>
							{#if r.reason !== 'behind' && hasDrift}<em>View drift →</em>{/if}
						</button>
					{/each}
					<dl class="kv">
						<div><dt>Last built</dt><dd><BuildSource build={d.lastBuild} /></dd></div>
						{#if d.lastBuild?.jobKey}<div><dt>Job</dt><dd><a class="link" href={to(`/jobs/${d.lastBuild.jobKey}`)}>{d.lastBuild.jobKey}</a></dd></div>{/if}
						{#if d.lastBuild}<div><dt>Started</dt><dd>{formatStamp(d.lastBuild.builtAt)}</dd></div>{/if}
						<div><dt>Relation</dt><dd class="mono">{relationName(d.relation)}</dd></div>
					</dl>
				</InspectorSection>
			{/if}

			{#if (d ?? listed)?.description}
				<InspectorSection title="Description">
					<p class="insp-description">{(d ?? listed)?.description}</p>
				</InspectorSection>
			{/if}

			{#if d?.sql}
				<InspectorSection title="SQL">
					{#snippet meta()}<span>{sqlRaw.split('\n').length} lines</span><Button variant="link" size="inline" class="ml-auto text-[12px]" onclick={() => openSql()}>Open →</Button>{/snippet}
					<CodePreview file="{shortName(d.key)}.sql" code={d.sql.raw} compiled={d.sql.compiled} bind:mode={sqlMode} maxLines={14} onOpen={() => openSql()} />
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
						<a class="check-row" href={to(`/checks/${c.id}`)}>
							<i style:background={checkOutcomeTone[c.latest?.outcome ?? 'skipped']}></i><code>{c.name}</code>
							<span title={runScopePhaseHelp[c.runScopePhase]}>{runScopePhaseLabel[c.runScopePhase]}</span>
						</a>
					{/each}
				</InspectorSection>
			{/if}

			{#if d?.columns.length}
				<InspectorSection title="Columns">
					{#snippet meta()}<a class="section-link" href={to(`/catalog/${primarySlug}?tab=columns`)}>All {d.columns.length} →</a>{/snippet}
					<dl class="kv">
						{#each d.columns.slice(0, 6) as c (c.name)}
							<div><dt class="mono fg">{c.name}</dt><dd class="muted">{c.description}</dd><dd class="mono muted type">{c.type}</dd></div>
						{/each}
					</dl>
				</InspectorSection>
			{/if}

			{#if d && !isSource && d.runs}
				<InspectorSection title="Recent runs">
					{#snippet meta()}<a class="section-link" href={to(`/catalog/${primarySlug}?tab=runs`)}>All →</a>{/snippet}
					{#each d.runs.slice(0, 3) as run, i (i)}
						<a class="mini-run" href={run.runId ? to(`/runs/${run.runId}`) : undefined}>
							<i style:background={nodeOutcomeTone[run.outcome]}></i>
							<span class="mini-run-main"><b>{nodeOutcomeLabel[run.outcome]}</b><span>{run.jobKey ?? (run.runId ? 'Manual' : `CLI · ${shortRunId(run.fingerprintRunId ?? '')}`)}</span></span>
							<span class="mini-run-meta">{formatMs(run.durationMs)} · {formatAgo(run.startedAt, app.now)}</span>
							{#if run.message}<em>{run.message}</em>{/if}
						</a>
					{:else}
						<span class="section-meta">No builds yet.</span>
					{/each}
				</InspectorSection>
			{/if}

			<!-- Waits while the node's facts load, so the whole panel fills at once instead of sections arriving above it. -->
			{#if d || listed || scaleMode || !detail.loading || detail.error || catalog.error}
			<InspectorSection title="Dependencies">
				{#snippet meta()}<span>{upstream.length} up · {downstream.length} down</span>{/snippet}
				<div class="deps">
					<div>
						<div class="dep-head">Upstream</div>
						{#each upstream as model (model)}
							<!-- raw-button: a row of the dependency list, clickable as a whole to select that node -->
							<button class="dependency-row" onclick={() => selectByName(String(model))}>{model}</button>
						{:else}<span class="section-meta">None</span>{/each}
					</div>
					<div>
						<div class="dep-head">Downstream</div>
						{#each downstream as model (model)}
							<!-- raw-button: a row of the dependency list, clickable as a whole to select that node -->
							<button class="dependency-row" onclick={() => selectByName(String(model))}>{model}</button>
						{:else}<span class="section-meta">None</span>{/each}
					</div>
				</div>
			</InspectorSection>
			{/if}

			{#if d}
				<InspectorSection title="Definition">
					<dl class="kv">
						<div><dt>Kind</dt><dd>{kindLine(d)}</dd></div>
						<div><dt>Unique key</dt><dd class="mono">{sqlOutline.config.find((c) => c.key === 'unique_key')?.value ?? '—'}</dd></div>
						<div><dt>Group</dt><dd>{d.group ?? '—'}</dd></div>
						<div><dt>Tags</dt><dd>{d.tags.join(', ') || '—'}</dd></div>
						<div><dt>Build version</dt><dd class="mono" title="Fingerprint of the last build in this location">{d.lastBuild?.fingerprint.slice(0, 12) ?? '—'}</dd></div>
					</dl>
				</InspectorSection>
			{/if}
			{/if}
		</div>
	</aside>
	</div>
	{/if}
</div>
	</section>
</div>

<style>
	:global(.svelte-flow__node.box-hovered > .box) {
		border-color: color-mix(in srgb, var(--primary) 55%, var(--border-strong));
	}
	:global(.svelte-flow__node.box-picked > .box) {
		border-color: var(--primary);
		box-shadow: 0 0 0 1px var(--primary);
	}
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
	.panel-toggle.active {
		border-color: color-mix(in srgb, var(--primary) 28%, var(--border));
		background: var(--sidebar-accent);
		color: var(--primary);
	}
	.toolbar-filter {
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
	.toolbar-filter:hover {
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
	.graph-loading {
		position: absolute;
		inset: 0;
		z-index: 5;
		background: var(--sb-inset);
		padding: 24px;
	}
	.graph-loading > div {
		width: 240px;
	}
	.graph-too-many {
		position: absolute;
		inset: 0;
		z-index: 5;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px;
		background: var(--sb-inset);
		color: var(--muted-foreground);
		font-size: 13px;
		text-align: center;
	}
	.inspector-panel {
		height: 100%;
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
	/* Toolbar buttons match the top bar's: 30 px high, 13 px medium text, 14 px icons. */
	.toolbar-filter {
		height: 30px;
		gap: 6px;
		padding: 0 9px;
		border: 0;
		border-radius: 2px;
		font-size: 13px;
		font-weight: 500;
	}
	.toolbar-filter :global(svg) {
		color: var(--sb-text-faint);
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
		transition: opacity 120ms;
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
		gap: 2px;
		margin-top: 6px;
		padding: 1px 6px 1px 2px;
		border-radius: 2px;
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 600;
		text-transform: capitalize;
	}
	.browser-group.picked {
		background: var(--sidebar-accent);
		color: var(--primary);
	}
	.browser-group.shown {
		color: var(--foreground);
	}
	.tree-chevron {
		display: grid;
		width: 20px;
		height: 22px;
		flex: 0 0 20px;
		place-items: center;
		border-radius: 2px;
	}
	.tree-chevron:hover {
		background: var(--sb-hover);
	}
	.tree-label {
		display: flex;
		min-width: 0;
		flex: 1;
		align-items: center;
		gap: 6px;
		padding: 4px 0;
		text-align: left;
		text-transform: inherit;
	}
	/* The checkbox adds a folder to the selection; it shows on hover, and stays while checked. */
	.tree-check {
		flex: 0 0 auto;
		margin-left: 6px;
		opacity: 0;
	}
	.browser-group:hover .tree-check,
	.tree-check:checked,
	.tree-check:focus-visible {
		opacity: 1;
	}
	.model-browser.pick {
		box-shadow: inset 0 0 0 2px var(--primary);
		animation: pick-pulse 1.2s ease-in-out 2;
	}
	@keyframes pick-pulse {
		50% {
			box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--primary) 30%, transparent);
		}
	}
	.selection-chip {
		display: inline-flex;
		max-width: 280px;
		height: 25px;
		align-items: center;
		gap: 4px;
		padding: 0 4px 0 9px;
		border: 1px solid color-mix(in srgb, var(--primary) 28%, var(--border));
		border-radius: 13px;
		background: var(--sidebar-accent);
		color: var(--primary);
		font-size: 12px;
	}
	.selection-chip > span {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.too-many-card {
		max-width: 420px;
		padding: 20px 22px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--card);
		box-shadow: var(--sb-elev);
		color: var(--muted-foreground);
	}
	.too-many-card strong {
		display: block;
		color: var(--foreground);
		font-size: 14px;
		font-weight: 600;
	}
	.too-many-card p {
		margin: 6px 0 14px;
		font-size: 12.5px;
	}
	.too-many-actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: 8px;
	}
	.browser-group:first-child {
		margin-top: 0;
	}
	.browser-group:not(.picked):hover {
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
	.browser-group span {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
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
		border-color: color-mix(in srgb, var(--sb-behind) 35%, var(--border));
	}
	.drift-hint.behind :global(svg) {
		color: var(--sb-behind);
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
	.loading-meta {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.loading-meta :global(.spin) {
		animation: spin 1s linear infinite;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
	.status-stats .stat-label {
		width: 50%;
		height: 8px;
	}
	.status-stats .stat-value {
		width: 40%;
		height: 16px;
		margin-top: 6px;
	}
	.strip-bar {
		height: 14px;
		margin: 16px 0 14px;
	}
	@media (prefers-reduced-motion: reduce) {
		.loading-meta :global(.spin) {
			animation: none;
		}
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
</style>
