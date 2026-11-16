<script lang="ts">
	import PlusIcon from '@lucide/svelte/icons/plus';
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';
	import type { StubData } from '$lib/lineage/folder-selection';

	let { data }: NodeProps = $props();
	const d = $derived(data as StubData);
</script>

<!-- Lineage outside the selected folders, e.g. "+12 upstream in customers"; clicking it adds that folder. -->
<button class="stub nodrag" class:dimmed={d.dimmed} title="Add {d.folder} to the folders shown" onclick={() => d.onadd?.(d.folder)}>
	{#if d.direction === 'downstream'}<Handle type="target" position={Position.Left} class="stub-handle" />{/if}
	<PlusIcon size={12} />
	<span>{d.label.replace(/^\+/, '')}</span>
	{#if d.direction === 'upstream'}<Handle type="source" position={Position.Right} class="stub-handle" />{/if}
</button>

<style>
	.stub {
		display: inline-flex;
		max-width: 236px;
		height: 26px;
		align-items: center;
		gap: 5px;
		padding: 0 9px;
		border: 1px dashed var(--node-border, var(--border));
		border-radius: 13px;
		background: var(--card);
		color: var(--muted-foreground);
		cursor: pointer;
		font: 500 11.5px var(--font-sans);
		transition: opacity 0.15s, border-color 0.15s;
	}
	.stub:hover {
		border-color: var(--primary);
		color: var(--foreground);
	}
	.stub.dimmed {
		opacity: 0.35;
	}
	.stub span {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.stub :global(.stub-handle) {
		width: 6px;
		height: 6px;
		min-width: 0;
		min-height: 0;
		border: 0;
		background: transparent;
	}
</style>
