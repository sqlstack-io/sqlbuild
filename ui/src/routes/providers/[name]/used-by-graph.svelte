<script lang="ts">
	import WaypointsIcon from '@lucide/svelte/icons/waypoints';
	import { tick } from 'svelte';
	import type { ProviderConnection } from '$lib/api';
	import DisclosureButton from '$lib/components/disclosure-button.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import { lineageNodesHref, NODES_PARAM } from '$lib/lineage/folder-selection';
	import { to } from '$lib/paths';
	import { consumerKindLabel, nodeHref, nodeName, usedByGraph, usedBySentence } from '../utils';

	// The provider's Used by tab, read left to right like the Lineage: the provider, the code that uses
	// it grouped by kind, and the jobs that run that code, joined by curved edges.
	let { provider, catalogKind }: { provider: ProviderConnection; catalogKind: (key: string) => string | null } = $props();

	/** Names listed in an open group before "+N more · show in Catalog". */
	const MEMBERS_SHOWN = 6;
	/** Names a closed group previews. */
	const PREVIEW = 2;

	const graph = $derived(usedByGraph(provider, catalogKind));
	const allNodes = $derived(graph.groups.flatMap((g) => g.nodes));
	const capItems = (keys: string[], max: number) => ({ shown: keys.slice(0, max), hidden: keys.slice(max) });
	const catalogHref = (keys: readonly string[]) => `/catalog?${NODES_PARAM}=${keys.map(encodeURIComponent).join(',')}`;
	let open = $state<ReadonlySet<string>>(new Set());
	function toggle(kind: string) {
		const next = new Set(open);
		if (next.has(kind)) next.delete(kind);
		else next.add(kind);
		open = next;
	}

	// Edges join the cards where they are laid out, so they are measured after each render and resize.
	let graphEl = $state<HTMLDivElement>();
	let paths = $state<string[]>([]);
	function measure() {
		if (!graphEl) return;
		const base = graphEl.getBoundingClientRect();
		const box = (id: string) => graphEl!.querySelector(`[data-id="${CSS.escape(id)}"]`)?.getBoundingClientRect();
		const path = (a: DOMRect | undefined, b: DOMRect | undefined) => {
			if (!a || !b) return null;
			const x1 = a.right - base.left;
			const y1 = a.top + Math.min(a.height / 2, 22) - base.top;
			const x2 = b.left - base.left;
			const y2 = b.top + Math.min(b.height / 2, 22) - base.top;
			const mx = (x1 + x2) / 2;
			return `M${x1},${y1} C${mx},${y1} ${mx},${y2} ${x2},${y2}`;
		};
		const from = box('provider');
		paths = graph.groups.flatMap((g) => {
			const at = box(`group:${g.kind}`);
			return [path(from, at), ...g.jobs.map((j) => path(at, box(`job:${j}`)))].filter((d): d is string => d !== null);
		});
	}
	$effect(() => {
		void graph;
		void open;
		void tick().then(measure);
	});
	$effect(() => {
		if (!graphEl) return;
		const ro = new ResizeObserver(() => measure());
		ro.observe(graphEl);
		return () => ro.disconnect();
	});
</script>

<div class="used-by">
	<div class="bar">
		<span>{usedBySentence(provider.name, graph)}</span>
		{#if allNodes.length}
			<Button href={to(lineageNodesHref(allNodes))} class="ml-auto" title="Open the Lineage with only this provider's code drawn"><WaypointsIcon />Open in Lineage</Button>
		{/if}
	</div>

	{#if graph.groups.length || graph.jobs.length}
		<div class="graph" bind:this={graphEl}>
			<svg class="edges" aria-hidden="true">
				{#each paths as d, i (i)}<path {d} />{/each}
			</svg>
			<div class="col">
				<h4>Provider</h4>
				<div class="box provider" data-id="provider">
					<div class="title">{provider.name}</div>
					<div class="sub mono" title={provider.className}>{provider.className}</div>
				</div>
			</div>
			<div class="col">
				<h4>Code using it · {allNodes.length}</h4>
				{#each graph.groups as g (g.kind)}
					{@const isOpen = open.has(g.kind)}
					<div class="box group" data-id="group:{g.kind}">
						<div class="title">
							<DisclosureButton tone="default" open={isOpen} onclick={() => toggle(g.kind)} title={isOpen ? 'Hide the names' : 'List the names'}>{g.label}</DisclosureButton>
							<a class="count" href={to(catalogHref(g.nodes))} title="Show these {g.nodes.length} in the Catalog">{g.nodes.length}</a>
						</div>
						{#if isOpen}
							{@const members = capItems(g.nodes, MEMBERS_SHOWN)}
							<div class="members">
								{#each members.shown as key (key)}<a class="mono" href={to(nodeHref(key, g.kind))} title={key}>{nodeName(key)}</a>{/each}
								{#if members.hidden.length}<a class="more" href={to(catalogHref(g.nodes))}>+{members.hidden.length} more · show in Catalog</a>{/if}
							</div>
						{:else}
							{@const preview = capItems(g.nodes, PREVIEW)}
							<div class="sub">
								{#each preview.shown as key, i (key)}{#if i}, {/if}<a href={to(nodeHref(key, g.kind))} title={key}>{nodeName(key)}</a>{/each}{#if preview.hidden.length}{' …'}{/if}
							</div>
						{/if}
					</div>
				{/each}
			</div>
			<div class="col">
				<h4>Jobs that run it · {graph.jobs.length}</h4>
				{#each graph.jobs as job (job)}
					<a class="box job" data-id="job:{job}" href={to(`/jobs/${job}`)}>
						<div class="title">{job}</div>
						<div class="sub">{(provider.jobNodes[job] ?? []).length} of its nodes use this provider</div>
					</a>
				{:else}
					<p class="none">No job runs this code.</p>
				{/each}
			</div>
		</div>
	{/if}

	{#if graph.others.length}
		<p class="others">
			Also used by
			{#each graph.others as c, i (c.kind + c.name)}{#if i}, {/if}{consumerKindLabel[c.kind]} <code>{c.name}</code>{/each}, which run inside other nodes or outside jobs.
		</p>
	{/if}
</div>

<style>
	.used-by {
		display: flex;
		max-width: 1100px;
		flex-direction: column;
	}
	.bar {
		display: flex;
		min-height: 30px;
		align-items: center;
		gap: 10px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.graph {
		position: relative;
		display: grid;
		grid-template-columns: minmax(180px, 220px) minmax(220px, 1fr) minmax(200px, 260px);
		align-items: center;
		column-gap: 90px;
		margin-top: 14px;
		padding: 18px 24px 24px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--sb-inset);
	}
	.edges {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		pointer-events: none;
	}
	.edges path {
		fill: none;
		stroke: var(--sb-edge);
		stroke-width: 1.5;
	}
	.col {
		position: relative;
		display: flex;
		min-width: 0;
		flex-direction: column;
		gap: 10px;
	}
	h4 {
		margin: 0 0 2px;
		color: var(--sb-text-faint);
		font-size: 11px;
		font-weight: 600;
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}
	.box {
		display: block;
		min-width: 0;
		padding: 8px 10px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--card);
		color: var(--foreground);
	}
	.box.provider {
		border-left: 3px solid var(--sb-stale);
	}
	.box.job {
		border-left: 3px solid var(--primary);
	}
	a.box:hover {
		border-color: var(--border-strong);
		border-left-color: var(--primary);
	}
	.title {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		font-weight: 500;
	}
	.count {
		margin-left: auto;
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 400;
	}
	.sub {
		overflow: hidden;
		margin-top: 2px;
		color: var(--muted-foreground);
		font-size: 11.5px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.sub.mono,
	.members .mono {
		font-family: var(--font-mono);
	}
	.members {
		display: flex;
		flex-direction: column;
		gap: 2px;
		margin-top: 6px;
	}
	.members a {
		overflow: hidden;
		padding: 1px 0;
		font-size: 11.5px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.members .more {
		color: var(--muted-foreground);
	}
	.members a:hover,
	.sub a:hover,
	.count:hover {
		color: var(--primary);
		text-decoration: underline;
	}
	.none,
	.others {
		margin: 0;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.others {
		margin-top: 10px;
	}
	.others code {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
</style>
