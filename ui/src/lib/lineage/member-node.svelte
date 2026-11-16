<script lang="ts">
	import WorkflowIcon from '@lucide/svelte/icons/workflow';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';
	import { formatAgo } from '$lib/format/time';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import { app } from '$lib/state/app.svelte';
	import type { MemberCardData } from './member-graph';

	// A job group's member job, drawn as a Lineage card; clicking it opens the member in place, as a folder opens on Lineage.
	let { data, selected }: NodeProps = $props();
	const d = $derived(data as MemberCardData);
	const tone = $derived(d.lastRun ? runStatusTone[d.lastRun.status] : 'var(--sb-text-faint)');
	const when = $derived(d.lastRun?.startedAt ? formatAgo(d.lastRun.startedAt, app.now) : '');
</script>

<div class="node" class:selected class:expandable={d.expandable} title={d.expandable ? `Show the nodes of ${d.jobKey}` : d.jobKey}>
	<Handle type="target" position={Position.Left} class="node-handle" />
	<div class="head">
		<WorkflowIcon size={14} strokeWidth={1.75} class="glyph" />
		<span class="name">{d.jobKey}</span>
		{#if d.nodeCount !== null}<span class="count">{d.nodeCount} node{d.nodeCount === 1 ? '' : 's'}</span>{/if}
		{#if d.expandable}<ChevronRightIcon size={13} class="glyph" />{/if}
	</div>
	<div class="status" style:--tone={tone}>
		<i></i>
		{#if d.lastRun}
			<span class="label">{runStatusLabel[d.lastRun.status]}</span>
			{#if when}<span class="when">{when}</span>{/if}
		{:else}
			<span class="label faint">No runs yet</span>
		{/if}
		{#if d.waitsFor}<span class="waits">waits for {d.waitsFor}</span>{/if}
	</div>
	<Handle type="source" position={Position.Right} class="node-handle" />
</div>

<style>
	.node {
		width: 236px;
		height: 62px;
		overflow: hidden;
		border: 1px solid var(--node-border);
		border-radius: 2px;
		background: var(--card);
		color: var(--foreground);
		font-family: var(--font-sans);
		transition: border-color 0.15s;
	}
	.node.expandable {
		cursor: pointer;
	}
	.node:hover {
		border-color: color-mix(in srgb, var(--foreground) 35%, transparent);
	}
	.node.selected {
		border-color: var(--primary);
		box-shadow: 0 0 0 1px var(--primary);
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
		flex: 0 0 auto;
		color: var(--sb-text-faint);
		font-size: 11px;
	}
	.status {
		display: flex;
		height: 26px;
		align-items: center;
		gap: 6px;
		padding: 0 10px;
		border-top: 1px solid var(--border);
		background: color-mix(in srgb, var(--tone) 9%, var(--card));
		font-size: 11px;
		white-space: nowrap;
	}
	.status i {
		width: 7px;
		height: 7px;
		flex: 0 0 7px;
		border-radius: 50%;
		background: var(--tone);
	}
	.label {
		font-weight: 500;
	}
	.faint {
		color: var(--sb-text-faint);
		font-weight: 400;
	}
	.when {
		color: var(--muted-foreground);
	}
	.when::before {
		content: '·';
		margin-right: 6px;
		color: var(--sb-text-faint);
	}
	.waits {
		margin-left: auto;
		color: var(--sb-text-faint);
	}
	.node :global(.node-handle) {
		width: 6px;
		height: 6px;
		min-width: 0;
		min-height: 0;
		border: 0;
		background: transparent;
	}
</style>
