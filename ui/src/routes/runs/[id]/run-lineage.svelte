<script lang="ts">
	import '@xyflow/svelte/dist/style.css';
	import type { Edge, Node } from '@xyflow/svelte';
	import { untrack } from 'svelte';
	import GraphCanvas from '$lib/components/graph/graph-canvas.svelte';
	import AssetNode from '$lib/components/lineage/asset-node.svelte';
	import FolderNode from '$lib/components/lineage/folder-node.svelte';
	import StubNode from '$lib/components/lineage/stub-node.svelte';
	import JobBoxNode from '$lib/lineage/job-box-node.svelte';
	import MemberGraph from '$lib/lineage/member-graph.svelte';
	import type { MemberInfo, MemberSelection } from '$lib/lineage/member-graph';
	import { folderTree, layoutFolderGraph, withFolderClosed } from '$lib/lineage/folder-graph';
	import { lineageGraph } from '$lib/lineage/folder-selection';
	import { folderOf } from '$lib/lineage/catalog-graph';
	import type { CatalogNode, RunNode } from '$lib/api';
	import { useCatalogNodes } from '$lib/state/catalog.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import type { RunDetailState } from './state.svelte';

	// The run's Graph (CHI-1591, change 2): the Lineage page's own graph, by folder, with each card coloured
	// by what happened to it in this run. A group run draws the group's lineage instead, its member jobs as
	// boxes, as the group's job page does, coloured by this run's member runs and nodes.
	let { view }: { view: RunDetailState } = $props();

	const catalog = useCatalogNodes();
	const LIVE: Record<RunNode['status'], string> = { success: 'ok', warn: 'warn', failed: 'fail', running: 'run', skipped: 'skip', pending: 'wait', not_run: 'notrun', cancelled: 'notrun' };
	/** The run's nodes as catalog nodes, with this run's outcome as their status. */
	const runCatalog = $derived.by((): CatalogNode[] => {
		const byKey = new Map((catalog.data ?? []).map((n) => [n.key, n]));
		return view.nodes.flatMap((r) => {
			const c = byKey.get(r.nodeKey);
			return c ? [{ ...c, liveStatus: LIVE[r.status] as CatalogNode['liveStatus'], stale: [], lastBuild: r.endedAt ? { ...(c.lastBuild ?? { fingerprintRunId: r.runId, runId: r.runId, jobKey: r.jobKey, fingerprint: '' }), builtAt: r.endedAt } : c.lastBuild }] : [];
		});
	});
	const isGroup = $derived(view.run?.kind === 'group');

	// ---- a job run: its nodes by folder ----
	const nodeTypes = { asset: AssetNode, folder: FolderNode, folderBox: JobBoxNode, stub: StubNode };
	let open = $state<ReadonlySet<string> | null>(null);
	let nodes = $state.raw<Node[]>([]);
	let edges = $state.raw<Edge[]>([]);
	let laidOut = '';
	const shape = $derived(runCatalog.map((n) => n.key).join(','));
	/** Every folder starts open, so the run's cards show inside their folders; a folder's header closes it. */
	function allFolders(data: CatalogNode[]): Set<string> {
		const out = new Set<string>();
		for (const n of data) {
			const parts = folderOf(n).split('/');
			parts.forEach((_, i) => out.add(parts.slice(0, i + 1).join('/')));
		}
		return out;
	}
	function draw(data: CatalogNode[]) {
		const opened = open ?? allFolders(data);
		const graph = lineageGraph(data, { grouped: true, open: opened, selected: [], tree: folderTree(data), onclose: (f) => setOpen(withFolderClosed(opened, f)), onadd: () => {} });
		const selected = new Set(view.selected);
		return { nodes: graph.nodes.map((n) => (selected.has(n.id) ? { ...n, selected: true } : n)), edges: graph.edges };
	}
	function setOpen(next: ReadonlySet<string>) {
		open = next;
		laidOut = '';
		const g = draw(runCatalog);
		nodes = g.nodes;
		edges = g.edges;
	}
	$effect(() => {
		if (isGroup || !runCatalog.length) return;
		void view.selected;
		const data = runCatalog;
		untrack(() => {
			const g = draw(data);
			if (laidOut && shape === laidOut) {
				// Same cards: keep their places, refresh their colours and the selection.
				const fresh = new Map(g.nodes.map((n) => [n.id, n]));
				nodes = nodes.map((n) => {
					const f = fresh.get(n.id);
					return f ? { ...n, data: f.data, selected: f.selected } : n;
				});
				return;
			}
			nodes = g.nodes;
			edges = g.edges;
		});
	});
	$effect(() => {
		if (laidOut === shape || !nodes.length) return;
		if (!nodes.every((n) => n.type === 'folderBox' || (n.measured?.width && n.measured?.height))) return;
		nodes = layoutFolderGraph(nodes, edges);
		laidOut = shape;
		// Fit once the laid-out cards are in place.
		requestAnimationFrame(() => requestAnimationFrame(() => fit?.({ duration: 0 })));
	});
	let fit = $state<((opts?: { duration?: number }) => void) | undefined>();
	function onnodeclick({ node, event }: { node: Node; event: MouseEvent | TouchEvent }) {
		if (node.type === 'folder') return setOpen(new Set([...(open ?? allFolders(runCatalog)), String(node.data.group)]));
		if (node.type !== 'asset') return;
		void event;
		view.clickNode(node.id);
	}

	// ---- a group run: the group's lineage, coloured by this run ----
	const groupJob = createResource((client, scope) => (view.run?.kind === 'group' && view.run.jobKey ? client.getJob(scope, view.run.jobKey) : Promise.resolve(null)), ['jobs', 'snapshot']);
	const memberNodes = createResource(async (client, scope) => (view.run?.kind === 'group' ? new Map((await client.listJobs(scope)).map((j) => [j.key, j.nodes ?? null])) : null), ['jobs', 'snapshot']);
	const info = $derived(
		new Map<string, MemberInfo>(
			(view.detail?.children ?? []).flatMap((c) => (c.jobKey ? [[c.jobKey, { nodeCount: view.nodes.filter((n) => n.runId === c.id).length || null, lastRun: { id: c.id, status: c.status, startedAt: c.startedAt, endedAt: c.endedAt }, nodes: memberNodes.data?.get(c.jobKey) ?? null }]] : []))
		)
	);
	const failedMembers = $derived((view.detail?.children ?? []).filter((c) => c.status === 'failed' || c.status === 'partial').filter((c) => view.nodes.some((n) => n.runId === c.id && n.status === 'failed')).flatMap((c) => (c.jobKey ? [c.jobKey] : [])));
	let memberSelection = $state<MemberSelection | null>(null);
	$effect(() => {
		const node = memberSelection?.node;
		if (node) untrack(() => view.clickNode(node));
	});
</script>

{#if isGroup}
	<div class="sb-flow relative flex min-h-0 flex-1 flex-col">
		<!-- Drawn once its members' nodes and the catalog are in, so its first fit is the whole graph. -->
		{#if groupJob.data?.groupMembers && memberNodes.data && catalog.data}
			<!-- The failed members open in place, showing their nodes by folder; the others stay closed. -->
			<MemberGraph members={groupJob.data.groupMembers} {info} catalog={runCatalog} bind:selection={memberSelection} initialExpanded={failedMembers} fitWhole />
		{:else}
			<div class="wait">Loading the group's lineage…</div>
		{/if}
	</div>
{:else}
	<div class="sb-flow relative flex-1" style="background: var(--sb-inset)">
		<!-- Opens zoomed to fit the whole run (CHI-1591), compact cards and all; zoom in to read them. -->
		<GraphCanvas bind:nodes bind:edges bind:fit fitOnInit readable={false} fitFloor={0.1} controls="fit" {nodeTypes} elementsSelectable nodesDraggable {onnodeclick} fitViewOptions={{ padding: 0.06, maxZoom: 1, minZoom: 0.1 }} minZoom={0.05} />
	</div>
{/if}

<style>
	.wait {
		padding: 32px;
		color: var(--sb-text-faint);
		font-size: 12.5px;
		text-align: center;
	}
</style>
