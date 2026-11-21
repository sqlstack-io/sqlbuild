<script lang="ts">
	import type { Edge, Node, Viewport } from '@xyflow/svelte';
	import { tick, untrack } from 'svelte';
	import type { CatalogNode, JobGroupMember } from '$lib/api';
	import Button from '$lib/components/ui/button/button.svelte';
	import AssetNode from '$lib/components/lineage/asset-node.svelte';
	import FolderNode from '$lib/components/lineage/folder-node.svelte';
	import TooManyCard from '$lib/components/lineage/too-many-card.svelte';
	import GraphCanvas from '$lib/components/graph/graph-canvas.svelte';
	import GraphToolbar from '$lib/components/graph/graph-toolbar.svelte';
	import { MAX_DRAWN, folderBoxId, folderCardId, tooManyToDraw, withFolderClosed } from '$lib/lineage/folder-graph';
	import { absolutePosition as positionOn, anchoredViewport } from '$lib/lineage/fit';
	import { innerId, layoutMemberGraph, litEdges, memberFolderBoxId, memberFolderCardId, memberGraph, memberNodeId, memberReach, type MemberInfo, type MemberSelection, type NodeLink } from '$lib/lineage/member-graph';
	import { toggled } from '$lib/lineage/member-expand';
	import { MEMBER_FOLDERS_CLOSED_ABOVE, declarationFolder, memberFolderTree } from '$lib/lineage/member-folders';
	import JobBoxNode from './job-box-node.svelte';
	import MemberNode from './member-node.svelte';
	import MemberBoxNode from './member-box-node.svelte';
	import CountEdge from './count-edge.svelte';
	import { edgeStyle } from '$lib/lineage/visible-edges';

	// A job group's member jobs, one card each. Clicking a card selects it and opens the member in
	// place, a box around its nodes, as a folder opens on Lineage; edges follow what is visible at
	// each end. Clicking a node inside selects that node; clicking the canvas clears the selection.
	let {
		members,
		info,
		catalog = [],
		links = new Map(),
		selection = $bindable(null),
		details,
		initialExpanded = [],
		fitWhole = false
	}: {
		members: JobGroupMember[];
		info: ReadonlyMap<string, MemberInfo>;
		catalog?: readonly CatalogNode[];
		links?: ReadonlyMap<string, NodeLink[]>;
		selection?: MemberSelection | null;
		/** The page's details panel, toggled from the toolbar as on Lineage. */
		details?: { open: boolean; toggle: () => void };
		/** Members open in place when the graph first shows, such as a run's failed member. */
		initialExpanded?: readonly string[];
		/** Open on the whole graph, even when its cards turn compact (a run's graph). */
		fitWhole?: boolean;
	} = $props();

	const nodeTypes = { member: MemberNode, memberBox: MemberBoxNode, asset: AssetNode, folder: FolderNode, folderBox: JobBoxNode };
	const edgeTypes = { count: CountEdge };
	// svelte-ignore state_referenced_locally
	let expanded = $state<ReadonlySet<string>>(new Set(initialExpanded));
	let openFolders = $state<ReadonlyMap<string, ReadonlySet<string>>>(new Map());
	/** Selects a member and opens or closes it, from its card or its box's header. */
	function toggle(jobKey: string) {
		selection = { member: jobKey };
		anchorOn(memberNodeId(jobKey), memberNodeId(jobKey));
		expanded = toggled(expanded, jobKey);
	}
	// Opening or closing a member or a folder keeps the clicked card or box where it is on screen;
	// the view fits only when the graph first shows and on the Fit button.
	let anchor: { id: string; before: { x: number; y: number }; view: Viewport } | null = null;
	function anchorOn(from: string, to: string) {
		const before = positionOn(nodes, from);
		anchor = before ? { id: to, before, view: viewport } : null;
	}
	function setFolders(jobKey: string, next: ReadonlySet<string>) {
		openFolders = new Map([...openFolders, [jobKey, next]]);
	}
	function closeFolder(jobKey: string, folder: string) {
		anchorOn(innerId(jobKey, folderBoxId(folder)), innerId(jobKey, folderCardId(folder)));
		setFolders(jobKey, withFolderClosed(openFolders.get(jobKey) ?? new Set(), folder));
	}
	function openFolder(jobKey: string, folder: string) {
		anchorOn(innerId(jobKey, folderCardId(folder)), innerId(jobKey, folderBoxId(folder)));
		setFolders(jobKey, new Set([...(openFolders.get(jobKey) ?? []), folder]));
	}

	// Focus: only the selected member and the members it waits for or that wait for it.
	let focusOn = $state<string | null>(null);
	const shownMembers = $derived.by(() => {
		if (!focusOn) return members;
		const keep = new Set([focusOn, ...(reach.up.get(focusOn) ?? []), ...(reach.down.get(focusOn) ?? [])]);
		return members.filter((m) => keep.has(m.jobKey));
	});
	function toggleFocus() {
		focusOn = focusOn ? null : (selection?.member ?? null);
	}
	// Members by the folder they are declared in, as Lineage draws folders: a large group opens with its
	// folders closed, a small one with them open.
	const tree = $derived(memberFolderTree(shownMembers.map((m) => m.jobKey), (k) => declarationFolder(info.get(k)?.sourceFile)));
	const hasFolders = $derived(tree.folders.size > 0);
	let foldersOn = $state(true);
	/** Drawing by folder or not changes the whole graph, so the view fits it again, as Lineage's Folders does. */
	let refitNext = false;
	function toggleFolders() {
		foldersOn = !foldersOn;
		refitNext = true;
	}
	let openMemberFolders = $state<ReadonlySet<string>>(new Set());
	let foldersReady = false;
	const initialFolders = () => (members.length > MEMBER_FOLDERS_CLOSED_ABOVE ? new Set<string>() : new Set(tree.folders.keys()));
	$effect(() => {
		if (foldersReady || !hasFolders) return;
		foldersReady = true;
		openMemberFolders = untrack(initialFolders);
	});
	function openMemberFolder(folder: string) {
		anchorOn(memberFolderCardId(folder), memberFolderBoxId(folder));
		openMemberFolders = new Set([...openMemberFolders, folder]);
	}
	function closeMemberFolder(folder: string) {
		anchorOn(memberFolderBoxId(folder), memberFolderCardId(folder));
		openMemberFolders = new Set([...openMemberFolders].filter((f) => f !== folder && !f.startsWith(`${folder}/`)));
	}
	const folderOption = $derived(foldersOn && hasFolders ? { tree, open: openMemberFolders, onclose: closeMemberFolder } : undefined);

	let expandNote = $state('');
	/** Opens every folder, then every member whose nodes are known, in order, stopping before the graph draws too many cards. */
	function expandEverything() {
		openMemberFolders = new Set(tree.folders.keys());
		let next = new Set(expanded);
		const openable = shownMembers.filter((m) => (info.get(m.jobKey)?.nodes?.length ?? 0) > 0 && !next.has(m.jobKey));
		let opened = 0;
		for (const m of openable) {
			const trial = new Set([...next, m.jobKey]);
			if (memberGraph(shownMembers, info, { catalog, links, expanded: trial, openFolders, folders: folderOption && { ...folderOption, open: openMemberFolders } }).nodes.length > MAX_DRAWN) break;
			next = trial;
			opened++;
		}
		expandNote = opened < openable.length ? `Opened ${opened} of ${openable.length} jobs: the graph draws at most ${MAX_DRAWN.toLocaleString('en')} cards` : '';
		expanded = next;
	}
	function collapseEverything() {
		expanded = new Set();
		openFolders = new Map();
		openMemberFolders = new Set();
		expandNote = '';
	}
	/** Back to how the graph opened: every member closed, folders as they opened, no focus, fitted again. */
	async function reset() {
		focusOn = null;
		collapseEverything();
		foldersOn = true;
		openMemberFolders = initialFolders();
		await tick();
		// When the reset changed what is drawn, the new cards are measured and laid out first, and the
		// layout fits them; fitting now would fit the cards being replaced.
		if (laidOut !== shape) {
			refitNext = true;
			return;
		}
		if (nodes.length) nodes = layoutMemberGraph(nodes, edges);
		await tick();
		requestAnimationFrame(() => fit?.());
	}

	const graph = $derived(memberGraph(shownMembers, info, { catalog, links, expanded, openFolders, ontoggle: toggle, oncloseFolder: closeFolder, folders: folderOption }));
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
	// Hovering or selecting a card or box lights its edges and dims the rest, as on Lineage; a member
	// lights its whole upstream and downstream, including the waits the drawing leaves implied.
	let hoveredId = $state<string | null>(null);
	const reach = $derived(memberReach(members));
	const focusId = $derived(hoveredId ?? selectedId);
	$effect(() => {
		const id = focusId;
		// Tracks the edges too, so a redrawn graph is lit again; it writes only when a style changes.
		const current = edges;
		const lit = id ? litEdges(id, current, untrack(() => nodes), reach) : null;
		const styled = current.map((e) => {
			const count = (e.data as { count?: number } | undefined)?.count ?? 1;
			const style = edgeStyle(Math.max(1, count), !lit ? 'neutral' : lit.has(e.id) ? 'on' : 'dim', count === 0);
			return e.style === style ? e : { ...e, style };
		});
		if (styled.some((e, i) => e !== current[i])) edges = styled;
	});
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
		nodes = layoutMemberGraph(nodes, edges);
		const held = anchor;
		anchor = null;
		tick().then(() =>
			requestAnimationFrame(() => {
				const after = held && positionOn(nodes, held.id);
				if (held && after && setViewport && fitted) setViewport(anchoredViewport(held.view, held.before, after));
				else if (!fitted || refitNext) {
					refitNext = false;
					fit?.();
				}
			})
		);
	});

	function onnodeclick({ node, event }: { node: Node; event: MouseEvent | TouchEvent }) {
		if ((event.target as Element | null)?.closest('a, button')) return;
		if (node.type === 'member') {
			if (node.data.expandable) toggle(String(node.data.jobKey));
			else selection = { member: String(node.data.jobKey) };
		} else if (node.type === 'memberBox') selection = { member: String(node.data.jobKey) };
		else if (node.type === 'folder' && node.data.memberFolder) openMemberFolder(String(node.data.memberFolder));
		else if (node.type === 'folder') openFolder(String(node.data.member), String(node.data.group));
		else if (node.type === 'asset') selection = { member: String(node.data.member), node: String(node.data.name) };
	}
</script>

<div class="member-graph">
	<GraphToolbar
		folders={hasFolders ? { on: foldersOn, toggle: toggleFolders } : undefined}
		expand={{ all: expandEverything, collapse: collapseEverything, canCollapse: expanded.size > 0 || (foldersOn && openMemberFolders.size > 0), allTitle: 'Open every job in place, up to the graph’s card limit', collapseTitle: 'Close every job', note: expandNote }}
		focus={{ on: !!focusOn, can: !!selection, toggle: toggleFocus, title: 'Show only the selected job and the jobs it waits for or that wait for it' }}
		onreset={reset}
		{details}
	/>
	<GraphCanvas
		bind:nodes
		bind:edges
		bind:fit
		bind:fitted
		bind:viewport
		bind:setViewport
		bind:element={panel}
		readable={!fitWhole}
		{...fitWhole ? { fitFloor: 0.1 } : {}}
		{...fitWhole ? { fitViewOptions: { padding: 0.06, maxZoom: 1, minZoom: 0.1 } } : {}}
		{nodeTypes}
		{edgeTypes}
		nodesDraggable={false}
		nodesConnectable={false}
		elementsSelectable={false}
		{onnodeclick}
		onpaneclick={() => (selection = null)}
		onnodepointerenter={({ node }) => (hoveredId = node.id)}
		onnodepointerleave={() => (hoveredId = null)}
	/>
	{#if tooMany}
		<TooManyCard count={graph.nodes.length}>
			{#snippet actions()}
				{#if expanded.size}<Button variant="outline" size="bar" onclick={collapseEverything}>Close every job</Button>{/if}
			{/snippet}
		</TooManyCard>
	{/if}
</div>

<style>
	.member-graph {
		position: relative;
		display: flex;
		height: 100%;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--sb-inset);
	}
	.member-graph :global(.svelte-flow__node-folder),
	.member-graph :global(.svelte-flow__node-asset) {
		cursor: pointer;
	}
</style>
