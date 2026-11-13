<script lang="ts">
	import FolderIcon from '@lucide/svelte/icons/folder';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';
	import type { GroupCard } from '$lib/lineage/catalog-graph';

	let { data }: NodeProps = $props();
	const d = $derived(data as GroupCard);
</script>

<!-- A collapsed folder; clicking it opens the folder into its nodes. -->
<div class="group" class:dimmed={d.dimmed} title="Show the {d.count} nodes in {d.group}">
	<Handle type="target" position={Position.Left} class="node-handle" />
	<div class="head">
		<FolderIcon size={14} strokeWidth={1.75} class="glyph" />
		<span class="name">{d.group}</span>
		<span class="count">{d.count}</span>
		<ChevronRightIcon size={13} class="glyph" />
	</div>
	<div class="status">
		{#if d.ok}<span><i style:background="var(--sb-success)"></i>{d.ok} ok</span>{/if}
		{#if d.stale}<span><i style:background="var(--sb-stale)"></i>{d.stale} stale</span>{/if}
		{#if d.failed}<span><i style:background="var(--sb-error)"></i>{d.failed} failed</span>{/if}
		{#if !d.ok && !d.stale && !d.failed}<span class="faint">No builds</span>{/if}
	</div>
	<Handle type="source" position={Position.Right} class="node-handle" />
</div>

<style>
	.group {
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
