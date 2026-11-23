<script lang="ts">
	import { plural } from '$lib/format/plural';
	import { to } from '$lib/paths';
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import type { RunRef } from '$lib/api';
	import { formatAgo } from '$lib/format/time';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import { app } from '$lib/state/app.svelte';

	import type { FolderBoxData } from './folder-graph';
	import { clickGuard } from './click-guard';
	import type { DiscoveredBox } from './job-graph';
	import { discoveredStatusLabel, discoveredStatusTone } from '$lib/platform/labels';

	type JobBoxData = { jobKey: string; nodeCount: number; drawnElsewhere: number; collapsed: boolean; ontoggle?: (jobKey: string) => void; lastRun?: RunRef | null; discovered?: DiscoveredBox };

	// A job group's member box, or an open lineage folder, whose header closes it.
	let { data }: NodeProps = $props();
	const d = $derived(data as JobBoxData);
	const folder = $derived('folder' in data ? (data as FolderBoxData) : null);
	// An open folder's header drags its box like a title bar; a click that did not move closes it.
	const guard = clickGuard();
	const close = (e: MouseEvent) => guard.still(e) && folder?.ontoggle?.(folder.folder);
</script>

<div class="box" class:collapsed={!folder && d.collapsed} class:discovered={!folder && !!d.discovered && !d.collapsed}>
	<Handle type="target" position={Position.Left} class="box-handle" />
	{#if folder}
		<!-- Zoomed out, the header's target grows upward so it stays at least its height on screen. -->
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<span class="head-hit" aria-hidden="true" onpointerdown={guard.down} onclick={close}></span>
		<!-- raw-button: the header of a box on the lineage canvas: dragging it moves the box, a click closes it -->
		<button class="head folder-head" title="Close {folder.folder}; drag to move it" aria-expanded="true" onpointerdown={guard.down} onclick={close}>
			<ChevronDownIcon size={14} class="chevron" />
			<span class="name" title={folder.folder}>{folder.label}</span>
			<span class="faint">· {plural(folder.nodeCount, folder.noun ?? 'node')}{#if folder.stale} · <span class="stale">{folder.stale} stale</span>{/if}{#if folder.failed} · <span class="failed">{folder.failed} failed</span>{/if}</span>
		</button>
	{:else if d.discovered}
		{@const set = d.discovered}
		<!-- Mockup: a discovered set's header; the whole header collapses or expands the set. -->
		<!-- raw-button: the header of a discovered set's frame on the canvas; it must not start a drag -->
		<button
			class="head set-head nodrag"
			aria-expanded={!d.collapsed}
			aria-label="{d.collapsed ? 'Expand' : 'Collapse'} discovered set {set.set}"
			title="{d.collapsed ? 'Expand' : 'Collapse'} discovered set {set.set} (asset {d.jobKey})"
			onclick={() => d.ontoggle?.(d.jobKey)}
		>
			{#if d.collapsed}<ChevronRightIcon size={14} class="chevron" />{:else}<ChevronDownIcon size={14} class="chevron" />{/if}
			<span class="name">{set.set}</span>
			<span class="faint">· discovered set · {plural(d.nodeCount, 'step')}{#if set.release} · release <span class="mono-faint" title={set.releaseFull ?? undefined}>{set.release}</span>{/if}</span>
			<span class="last" title="{discoveredStatusLabel[set.status]}{set.error ? ` (${set.error})` : ''}"><i style:background={discoveredStatusTone[set.status]}></i>{discoveredStatusLabel[set.status]}</span>
		</button>
	{:else}
	<div class="head">
		{#if d.ontoggle}
			<!-- raw-button: the expand control inside a box on the lineage canvas; it must not start a drag -->
			<button class="toggle nodrag" aria-expanded={!d.collapsed} aria-label="{d.collapsed ? 'Expand' : 'Collapse'} {d.jobKey}" onclick={() => d.ontoggle?.(d.jobKey)}>
				{#if d.collapsed}<ChevronRightIcon size={14} />{:else}<ChevronDownIcon size={14} />{/if}
			</button>
		{/if}
		<a href={to(`/jobs/${d.jobKey}`)} class="name">{d.jobKey}</a>
		<span class="faint" title={d.drawnElsewhere ? `${d.drawnElsewhere} also in a smaller job` : undefined}>{plural(d.nodeCount, 'node')}</span>
		{#if d.lastRun}
			<!-- The dot carries the status, so the header fits a box one card wide. -->
			<a href={to(`/runs/${d.lastRun.id}`)} class="last" title="Last run of {d.jobKey}: {runStatusLabel[d.lastRun.status]}">
				<i style:background={runStatusTone[d.lastRun.status]}></i>{d.lastRun.startedAt ? formatAgo(d.lastRun.startedAt, app.now) : runStatusLabel[d.lastRun.status]}
			</a>
		{/if}
	</div>
	{/if}
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
	/* Mockup: an open discovered set's title sits on its frame, at a size that reads at any zoom. */
	.discovered {
		border-style: dashed;
		border-color: color-mix(in srgb, var(--primary) 45%, var(--border-strong));
		background: color-mix(in srgb, var(--primary) 4%, transparent);
	}
	.discovered .head {
		position: absolute;
		bottom: 100%;
		left: -1px;
		width: max-content;
		max-width: calc(100% / var(--sb-unzoom, 1));
		border: 1px solid color-mix(in srgb, var(--primary) 45%, var(--border-strong));
		border-bottom: 0;
		border-radius: 3px 3px 0 0;
		background: var(--card);
		transform: scale(var(--sb-unzoom, 1));
		transform-origin: bottom left;
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
	.folder-head {
		width: 100%;
		gap: 6px;
		padding-left: 8px;
		cursor: pointer;
		text-align: left;
	}
	.head-hit {
		position: absolute;
		top: calc(30px * (1 - var(--sb-unzoom, 1)));
		right: 0;
		left: 0;
		height: calc(30px * (var(--sb-unzoom, 1) - 1));
		cursor: pointer;
	}
	.folder-head:hover {
		background: var(--sb-hover);
	}
	.folder-head .name {
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.folder-head :global(.chevron) {
		flex: 0 0 auto;
		color: var(--muted-foreground);
	}
	.stale {
		color: var(--sb-stale);
	}
	.failed {
		color: var(--sb-error);
	}
	.set-head {
		width: 100%;
		gap: 6px;
		padding-left: 8px;
		cursor: pointer;
		text-align: left;
	}
	.set-head:hover {
		background: var(--sb-hover);
	}
	.set-head:focus-visible {
		outline: 2px solid var(--primary);
		outline-offset: -2px;
	}
	.set-head :global(.chevron) {
		flex: 0 0 auto;
		color: var(--muted-foreground);
	}
	.set-head .name:hover {
		color: inherit;
	}
	.mono-faint {
		font-family: var(--font-mono);
		font-size: 11.5px;
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
