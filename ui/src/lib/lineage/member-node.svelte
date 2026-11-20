<script lang="ts">
	import { plural } from '$lib/format/plural';
	import WorkflowIcon from '@lucide/svelte/icons/workflow';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';
	import { to } from '$lib/paths';
	import { formatAgo } from '$lib/format/time';
	import { memberRunLabel, runStatusTone } from '$lib/status';
	import { app } from '$lib/state/app.svelte';
	import type { MemberCardData } from './member-graph';
	import CompactFace from '$lib/components/graph/compact-face.svelte';

	// A job group's member job, drawn as a Lineage card; clicking it opens the member in place, as a
	// folder opens on Lineage, and "Open job" goes to the job's own page.
	let { id, data, selected }: NodeProps = $props();
	const d = $derived(data as MemberCardData);
	// Before the first run the dot says whether the member needs building, in Lineage's purple.
	const tone = $derived(d.lastRun ? runStatusTone[d.lastRun.status] : d.needs.changed ? 'var(--sb-stale)' : d.needs.behind ? 'var(--sb-behind)' : 'var(--sb-text-faint)');
	const needsTitle = $derived(
		[d.needs.changed && `${d.needs.changed} changed since their last build`, d.needs.behind && `${d.needs.behind} behind an upstream rebuild`].filter(Boolean).join(', ')
	);
	// Zoomed out, the card's colour: a failed or running run first, then needing a build, then the last run.
	const faceTone = $derived(
		d.lastRun && !['success', 'warn'].includes(d.lastRun.status) ? tone : d.needs.changed ? 'var(--sb-stale)' : d.needs.behind ? 'var(--sb-behind)' : tone
	);
	const when = $derived(d.lastRun?.startedAt ? formatAgo(d.lastRun.startedAt, app.now) : '');
</script>

<div class="node" class:selected class:expandable={d.expandable} title={d.expandable ? `Show the nodes of ${d.jobKey}` : d.jobKey}>
	<Handle type="target" position={Position.Left} class="node-handle" />
	<div class="head">
		<WorkflowIcon size={14} strokeWidth={1.75} class="glyph" />
		<span class="name">{d.jobKey}</span>
		<a class="open nodrag" href={to(`/jobs/${d.jobKey}`)} title="Open the page of {d.jobKey}">Open job ↗</a>
	</div>
	<div class="facts">
		{#if d.expandable}<ChevronRightIcon size={12} class="glyph" />{/if}
		{#if d.nodeCount !== null}<span>{plural(d.nodeCount, 'node')}</span>{/if}
		{#if d.waitsFor}<span class="waits">waits for {d.waitsFor}</span>{/if}
		{#if d.needs.changed || d.needs.behind}
			<span class="needs" title="Needs building: {needsTitle}">
				{#if d.needs.changed}<span class="changed">{d.needs.changed} changed</span>{/if}
				{#if d.needs.behind}<span class="behind">{d.needs.changed ? '+' : ''}{d.needs.behind} behind</span>{/if}
			</span>
		{/if}
	</div>
	<div class="status" style:--tone={tone}>
		<i></i>
		{#if d.lastRun}
			<span class="label">{memberRunLabel(d.lastRun)}</span>
			{#if when}<span class="when">{when}</span>{/if}
		{:else}
			<span class="label faint">No runs yet</span>
		{/if}
	</div>
	<CompactFace {id} title={d.jobKey} tone={faceTone} kind="job" />
	<Handle type="source" position={Position.Right} class="node-handle" />
</div>

<style>
	.node {
		position: relative;
		width: 236px;
		height: 82px;
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
		height: 32px;
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
	.open {
		flex: 0 0 auto;
		color: var(--muted-foreground);
		font-size: 11px;
	}
	.open:hover {
		color: var(--primary);
	}
	.facts {
		display: flex;
		height: 20px;
		align-items: center;
		gap: 6px;
		padding: 0 10px 2px 10px;
		color: var(--sb-text-faint);
		font-size: 11px;
		white-space: nowrap;
	}
	.facts :global(.glyph) {
		flex: 0 0 auto;
		margin-left: -2px;
		color: var(--muted-foreground);
	}
	.needs {
		display: inline-flex;
		gap: 5px;
		margin-left: auto;
		font-weight: 500;
	}
	.changed {
		color: var(--sb-stale);
	}
	.behind {
		color: var(--sb-behind);
		font-weight: 400;
	}
	.waits::before {
		content: '·';
		margin-right: 6px;
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
	.node :global(.node-handle) {
		width: 6px;
		height: 6px;
		min-width: 0;
		min-height: 0;
		border: 0;
		background: transparent;
	}
</style>
