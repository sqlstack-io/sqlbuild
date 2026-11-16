<script lang="ts">
	import { plural } from '$lib/format/plural';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';
	import type { GroupCard } from '$lib/lineage/catalog-graph';
	import CompactFace from '$lib/components/graph/compact-face.svelte';

	let { data }: NodeProps = $props();
	const d = $derived(data as GroupCard);
	// Zoomed out, the folder's colour is its worst state: any failure, then any stale node, then ok.
	const tone = $derived(d.failed ? 'var(--sb-error)' : d.stale ? 'var(--sb-stale)' : d.ok ? 'var(--sb-success)' : 'var(--sb-text-faint)');
</script>

<!-- A closed folder; clicking it opens the folder into a box around its folders and nodes. -->
<div class="group" class:dimmed={d.dimmed} title="Open {d.group}: {plural(d.count, d.noun ?? 'node')}">
	<Handle type="target" position={Position.Left} class="node-handle" />
	<div class="head">
		<FolderIcon size={14} strokeWidth={1.75} class="glyph" />
		<span class="name">{d.label ?? d.group}</span>
		<span class="count">{d.count}</span>
		<span class="hit" aria-hidden="true"><ChevronRightIcon size={13} class="glyph" /></span>
	</div>
	<div class="status">
		{#if d.ok}<span><i style:background="var(--sb-success)"></i>{d.ok} ok</span>{/if}
		{#if d.stale}<span><i style:background="var(--sb-stale)"></i>{d.stale} stale</span>{/if}
		{#if d.failed}<span><i style:background="var(--sb-error)"></i>{d.failed} failed</span>{/if}
		{#if !d.ok && !d.stale && !d.failed}<span class="faint">{d.empty ?? 'No builds'}</span>{/if}
	</div>
	<CompactFace title={d.label ?? d.group} {tone} height={59} badge={d.count} />
	<Handle type="source" position={Position.Right} class="node-handle" />
</div>

<style>
	.group {
		position: relative;
		width: 236px;
		border: 1px solid var(--node-border);
		border-radius: 2px;
		background: color-mix(in srgb, var(--card) 70%, var(--sb-inset));
		box-shadow: 3px 3px 0 -1px var(--card), 3px 3px 0 0 var(--node-border);
		color: var(--foreground);
		cursor: pointer;
		font-family: var(--font-sans);
		transition: opacity 0.15s, border-color 0.15s;
	}
	.group:hover {
		border-color: color-mix(in srgb, var(--foreground) 35%, transparent);
	}
	.group.dimmed {
		opacity: 0.35;
	}
	.head {
		display: flex;
		height: 34px;
		align-items: center;
		gap: 8px;
		padding: 0 10px;
	}
	.head :global(.glyph) {
		flex: 0 0 auto;
		color: var(--muted-foreground);
	}
	.name {
		min-width: 0;
		flex: 1;
		overflow: hidden;
		font-size: 12.5px;
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.count {
		color: var(--sb-text-faint);
		font-size: 11px;
	}
	/* The chevron's target: at least 28 px square on screen, however far the graph is zoomed out
	   (--sb-unzoom, set by the graph). The whole card opens the folder too. */
	.hit {
		display: inline-grid;
		width: 28px;
		height: 28px;
		flex: 0 0 28px;
		margin-right: -8px;
		place-items: center;
		border-radius: 3px;
		transform: scale(var(--sb-unzoom, 1));
		transform-origin: center right;
	}
	.group:hover .hit {
		background: var(--sb-hover);
	}
	.status {
		display: flex;
		gap: 10px;
		padding: 0 10px 8px 32px;
		color: var(--muted-foreground);
		font-size: 11px;
	}
	.status span {
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}
	.status i {
		width: 6px;
		height: 6px;
		border-radius: 50%;
	}
	.faint {
		color: var(--sb-text-faint);
	}
	.group :global(.node-handle) {
		width: 6px;
		height: 6px;
		min-width: 0;
		min-height: 0;
		border: 0;
		background: transparent;
	}
</style>
