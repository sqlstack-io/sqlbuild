<script lang="ts">
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';
	import type { RunNode } from '$lib/api';
	import { nodeStatusLabel, nodeStatusTone } from '$lib/operate/labels';

	let { data, selected }: NodeProps = $props();
	const n = $derived((data as { node: RunNode; ownRunId: string }).node);
	const ownRunId = $derived((data as { node: RunNode; ownRunId: string }).ownRunId);
	const dim = $derived(n.status === 'skipped' || n.status === 'not_run' || n.status === 'cancelled' || n.status === 'pending');
</script>

<div class="rn" class:selected class:dim style:--tone={nodeStatusTone[n.status]}>
	<Handle type="target" position={Position.Left} class="node-handle" />
	<div class="head">
		<span class="name" title={n.nodeKey}>{n.nodeKey}</span>
		<span class="kind">{n.kind}</span>
	</div>
	<div class="status">
		<i></i>{nodeStatusLabel[n.status]}
		{#if n.jobKey && n.runId !== ownRunId}<span class="job">{n.jobKey}</span>{/if}
	</div>
	{#if n.skippedBecauseOf}
		<div class="why">needs {n.skippedBecauseOf.nodeKey}{n.skippedBecauseOf.runId !== n.runId ? ` · ${n.skippedBecauseOf.jobKey ?? n.skippedBecauseOf.runId}` : ''}</div>
	{:else if n.errorMessage}
		<div class="why err" title={n.errorMessage}>{n.errorMessage}</div>
	{/if}
	<Handle type="source" position={Position.Right} class="node-handle" />
</div>

<style>
	.rn {
		width: 220px;
		border: 1px solid var(--node-border);
		border-left: 3px solid var(--tone);
		border-radius: 2px;
		background: var(--card);
		color: var(--foreground);
		font-size: 12px;
	}
	.rn.selected {
		border-color: var(--primary);
		box-shadow: 0 0 0 1px var(--primary);
	}
	.rn.dim {
		border-style: dashed;
		border-left-style: solid;
		opacity: 0.75;
	}
	.head {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 7px 9px 3px;
	}
	.name {
		overflow: hidden;
		flex: 1;
		font-family: var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.kind,
	.job {
		color: var(--sb-text-faint);
		font-size: 10.5px;
	}
	.job {
		margin-left: auto;
	}
	.status {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0 9px 7px;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.status i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--tone);
	}
	.why {
		overflow: hidden;
		padding: 5px 9px 6px;
		border-top: 1px solid var(--border-subtle);
		color: var(--sb-text-faint);
		font-size: 11px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.why.err {
		color: var(--sb-error);
	}
</style>
