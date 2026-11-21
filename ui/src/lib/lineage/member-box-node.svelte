<script lang="ts">
	import { plural } from '$lib/format/plural';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';
	import { to } from '$lib/paths';
	import { formatAgo } from '$lib/format/time';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import { app } from '$lib/state/app.svelte';
	import type { MemberCardData } from './member-graph';
	import { clickGuard } from './click-guard';

	// A member job opened in place: a box around its nodes, as an open folder on Lineage, whose header closes it.
	let { data, selected }: NodeProps = $props();
	const d = $derived(data as MemberCardData);
	// The header drags the box like a title bar; a click that did not move closes it.
	const guard = clickGuard();
</script>

<div class="box" class:selected>
	<Handle type="target" position={Position.Left} class="box-handle" />
	<!-- The whole header row closes the box; its links go where they say. Zoomed out, the target grows upward. -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<span class="head-hit" aria-hidden="true" onpointerdown={guard.down} onclick={(e) => guard.still(e) && d.ontoggle?.(d.jobKey)}></span>
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="head" onpointerdown={guard.down} onclick={(e) => guard.still(e) && !(e.target as Element).closest('a, button') && d.ontoggle?.(d.jobKey)}>
		<!-- raw-button: the header of a member box on the group graph, clickable as a whole to close it -->
		<button class="toggle nodrag" title="Close {d.jobKey}" aria-expanded="true" onclick={() => d.ontoggle?.(d.jobKey)}>
			<ChevronDownIcon size={14} class="chevron" />
			<span class="name">{d.jobKey}</span>
			{#if d.nodeCount !== null}<span class="faint">· {plural(d.nodeCount, 'node')}</span>{/if}
		</button>
		{#if d.lastRun}
			<span class="last" title="Last run: {runStatusLabel[d.lastRun.status]}">
				<i style:background={runStatusTone[d.lastRun.status]}></i>{d.lastRun.startedAt ? formatAgo(d.lastRun.startedAt, app.now) : runStatusLabel[d.lastRun.status]}
			</span>
		{/if}
		<a class="open nodrag" href={to(`/jobs/${d.jobKey}`)} title="Open the page of {d.jobKey}">Open job ↗</a>
	</div>
	<Handle type="source" position={Position.Right} class="box-handle" />
</div>

<style>
	.box {
		position: relative;
		width: 100%;
		height: 100%;
		border: 1px solid var(--border-strong);
		border-radius: 3px;
		background: color-mix(in srgb, var(--card) 45%, transparent);
	}
	.box.selected {
		border-color: var(--primary);
	}
	.head {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 30px;
		padding-right: 12px;
		overflow: hidden;
		white-space: nowrap;
		border-bottom: 1px solid var(--border);
		font-size: 12px;
	}
	.head {
		cursor: pointer;
	}
	.head:hover {
		background: var(--sb-hover);
	}
	.head-hit {
		position: absolute;
		top: calc(30px * (1 - var(--sb-unzoom, 1)));
		right: 0;
		left: 0;
		height: calc(30px * (var(--sb-unzoom, 1) - 1));
		cursor: pointer;
	}
	.toggle {
		display: flex;
		min-width: 0;
		height: 100%;
		flex: 1;
		align-items: center;
		gap: 6px;
		padding-left: 8px;
		cursor: pointer;
		text-align: left;
	}
	.toggle:hover {
		background: var(--sb-hover);
	}
	.toggle :global(.chevron) {
		flex: 0 0 auto;
		color: var(--muted-foreground);
	}
	.name {
		overflow: hidden;
		font-family: var(--font-mono);
		font-weight: 600;
		text-overflow: ellipsis;
	}
	.faint {
		color: var(--sb-text-faint);
	}
	.last {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		color: var(--muted-foreground);
	}
	.open {
		color: var(--muted-foreground);
	}
	.open:hover {
		color: var(--primary);
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
