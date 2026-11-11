<script lang="ts">
	import { to } from '$lib/paths';
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import type { RunRef } from '$lib/api';
	import { formatAgo } from '$lib/format/time';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import { app } from '$lib/state/app.svelte';

	let { data }: NodeProps = $props();
	const d = $derived(
		data as { jobKey: string; nodeCount: number; drawnElsewhere: number; collapsed: boolean; ontoggle?: (jobKey: string) => void; lastRun?: RunRef | null }
	);
</script>

<div class="box" class:collapsed={d.collapsed}>
	<Handle type="target" position={Position.Left} class="box-handle" />
	<div class="head">
		{#if d.ontoggle}
			<button class="toggle nodrag" aria-expanded={!d.collapsed} aria-label="{d.collapsed ? 'Expand' : 'Collapse'} {d.jobKey}" onclick={() => d.ontoggle?.(d.jobKey)}>
				{#if d.collapsed}<ChevronRightIcon size={14} />{:else}<ChevronDownIcon size={14} />{/if}
			</button>
		{/if}
		<a href={to(`/jobs/${d.jobKey}`)} class="name">{d.jobKey}</a>
		<span class="faint" title={d.drawnElsewhere ? `${d.drawnElsewhere} of them ${d.drawnElsewhere === 1 ? 'is' : 'are'} drawn in a job that builds fewer nodes` : undefined}>{d.nodeCount} node{d.nodeCount === 1 ? '' : 's'}</span>
		{#if d.lastRun}
			<!-- The dot carries the status, so the header fits a box one card wide. -->
			<a href={to(`/runs/${d.lastRun.id}`)} class="last" title="Last run of {d.jobKey}: {runStatusLabel[d.lastRun.status]}">
				<i style:background={runStatusTone[d.lastRun.status]}></i>{d.lastRun.startedAt ? formatAgo(d.lastRun.startedAt, app.now) : runStatusLabel[d.lastRun.status]}
			</a>
		{/if}
	</div>
	<Handle type="source" position={Position.Right} class="box-handle" />
</div>

<style>
	.box {
		width: 100%;
		height: 100%;
		border: 1px solid var(--border-strong);
		border-radius: 3px;
		background: color-mix(in srgb, var(--card) 45%, transparent);
	}
	.collapsed .head {
		border-bottom: 0;
	}
	.toggle {
		display: inline-flex;
		margin-left: -6px;
		padding: 2px;
		border-radius: 2px;
		color: var(--muted-foreground);
	}
	.toggle:hover {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	.head {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 30px;
		padding: 0 12px;
		overflow: hidden;
		white-space: nowrap;
		border-bottom: 1px solid var(--border);
		font-size: 12px;
	}
	.name {
		font-family: var(--font-mono);
		font-weight: 600;
	}
	.name:hover,
	.last:hover {
		color: var(--primary);
	}
	.faint {
		color: var(--sb-text-faint);
	}
	.last {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		margin-left: auto;
		color: var(--muted-foreground);
		white-space: nowrap;
	}
	.last i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
	}
	.box :global(.box-handle) {
		opacity: 0;
	}
</style>
