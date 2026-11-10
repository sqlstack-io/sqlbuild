<script lang="ts">
	import { to } from '$lib/paths';
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';
	import type { RunRef } from '$lib/api';
	import { formatAgo } from '$lib/format/time';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import { app } from '$lib/state/app.svelte';

	let { data }: NodeProps = $props();
	const d = $derived(data as { jobKey: string; nodeCount: number; lastRun?: RunRef | null });
</script>

<div class="box">
	<Handle type="target" position={Position.Left} class="box-handle" />
	<div class="head">
		<a href={to(`/jobs/${d.jobKey}`)} class="name">{d.jobKey}</a>
		<span class="faint">{d.nodeCount} node{d.nodeCount === 1 ? '' : 's'}</span>
		{#if d.lastRun}
			<a href={to(`/runs/${d.lastRun.id}`)} class="last" title="Last run of {d.jobKey}">
				<i style:background={runStatusTone[d.lastRun.status]}></i>{runStatusLabel[d.lastRun.status]}{d.lastRun.startedAt ? ` · ${formatAgo(d.lastRun.startedAt, app.now)}` : ''}
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
	.head {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 30px;
		padding: 0 12px;
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
