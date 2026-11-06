<script lang="ts">
	import { onMount } from 'svelte';
	import type { RunNode } from '$lib/api';
	import { nodeStatusTone } from '$lib/operate/labels';
	import { axisTicks as ticksFor, formatSpan, formatTick, tickStep as stepFor } from './utils';

	export type GanttGroup = { runId: string; label: string; tone: string; href: string };

	let {
		nodes,
		origin,
		now,
		selected = [],
		groups = [],
		onselect
	}: {
		nodes: RunNode[];
		/** Time zero of the axis (ms): the first node's start. */
		origin: number;
		now: number;
		selected?: string[];
		/** Child runs of a group, to section the rows. */
		groups?: GanttGroup[];
		/** `add` is true for Ctrl-, Cmd- or Shift-click. */
		onselect: (nodeKey: string, add: boolean) => void;
	} = $props();

	const startOf = (n: RunNode) => (n.startedAt ? (Date.parse(n.startedAt) - origin) / 1000 : null);
	const endOf = (n: RunNode) => (n.startedAt ? ((n.endedAt ? Date.parse(n.endedAt) : now) - origin) / 1000 : null);
	const total = $derived(Math.max(1, ...nodes.map((n) => endOf(n) ?? 0)));

	// Pixels-per-second with zoom and horizontal scroll; zoom 1 fits the run to the width.
	let zoom = $state(1);
	const ZOOM_MIN = 1;
	const ZOOM_MAX = 60;
	let viewportW = $state(900);
	let measureEl = $state<HTMLDivElement>();
	let ganttEl = $state<HTMLDivElement>();
	let gutterW = $state(250);
	let gutterDragging = $state(false);
	let scrolledX = $state(false);
	const GUTTER_MIN = 140;
	const GUTTER_MAX = 560;

	const trackBaseW = $derived(Math.max(120, viewportW - gutterW - 24));
	const trackW = $derived(trackBaseW * zoom);
	const pxPerSec = $derived(trackW / (total * 1.08));
	const xOf = (sec: number) => sec * pxPerSec;
	const tickStep = $derived(stepFor(total, Math.max(4, Math.floor(trackW / 110))));
	const axisTicks = $derived(ticksFor(total, tickStep));

	function autoFit(): number {
		const ctx = document.createElement('canvas').getContext('2d');
		if (!ctx) return 250;
		ctx.font = '12px "IBM Plex Mono", ui-monospace, monospace';
		const widest = Math.max(0, ...nodes.map((n) => ctx.measureText(n.nodeKey).width));
		return Math.min(360, Math.max(GUTTER_MIN, Math.ceil(widest) + 60));
	}

	onMount(() => {
		gutterW = autoFit();
		const el = measureEl;
		if (!el) return;
		const ro = new ResizeObserver(() => (viewportW = el.clientWidth));
		ro.observe(el);
		viewportW = el.clientWidth;
		return () => ro.disconnect();
	});

	function zoomBy(f: number) {
		zoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, zoom * f));
	}

	function onGutter(e: PointerEvent) {
		if (!gutterDragging || !ganttEl) return;
		gutterW = Math.min(GUTTER_MAX, Math.max(GUTTER_MIN, e.clientX - ganttEl.getBoundingClientRect().left));
	}

	type Row = { type: 'group'; group: GanttGroup; count: number } | { type: 'node'; node: RunNode };
	const rows = $derived.by((): Row[] => {
		if (groups.length < 2) return nodes.map((node) => ({ type: 'node', node }));
		return groups.flatMap((group): Row[] => {
			const own = nodes.filter((n) => n.runId === group.runId);
			return [{ type: 'group', group, count: own.length }, ...own.map((node): Row => ({ type: 'node', node }))];
		});
	});

	function skipText(n: RunNode): string {
		if (n.status === 'skipped' && n.skippedBecauseOf) {
			const across = n.skippedBecauseOf.runId !== n.runId;
			return `skipped: ${n.skippedBecauseOf.nodeKey} failed${across ? ` in ${n.skippedBecauseOf.jobKey ?? n.skippedBecauseOf.runId}` : ''}`;
		}
		if (n.status === 'pending') return 'not started yet';
		if (n.status === 'not_run') return 'not run: the run ended first';
		if (n.status === 'cancelled') return 'not run: cancelled';
		return n.status;
	}
</script>

<div class="gantt">
<div class="gantt-head">
	<span class="count">{nodes.length} node{nodes.length === 1 ? '' : 's'}</span>
	<div class="zoom">
		<button onclick={() => zoomBy(1 / 1.5)} disabled={zoom <= ZOOM_MIN} title="Zoom out">−</button>
		<input type="range" min={ZOOM_MIN} max={ZOOM_MAX} step="0.1" bind:value={zoom} title="Zoom" />
		<button onclick={() => zoomBy(1.5)} disabled={zoom >= ZOOM_MAX} title="Zoom in">+</button>
	</div>
	<button class="fit" onclick={() => ((zoom = 1), ganttEl && (ganttEl.scrollLeft = 0))} disabled={zoom === 1}>Fit</button>
</div>

<div class="pane">
	<div bind:this={measureEl} class="probe"></div>
	<div bind:this={ganttEl} class="sb-scroll scroller" class:select-none={gutterDragging} onscroll={(e) => (scrolledX = e.currentTarget.scrollLeft > 1)}>
		<div class="content" style:width="{gutterW + trackW + 24}px">
			<div class="grid" style:left="{gutterW}px" style:width="{trackW}px">
				{#each axisTicks as tk (tk)}<span style:left="{xOf(tk)}px"></span>{/each}
			</div>
			<div class="axis">
				<div class="gutter axis-gutter" class:shadow={scrolledX} style:width="{gutterW}px">Node</div>
				<div class="axis-track" style:width="{trackW}px">
					{#each axisTicks as tk (tk)}<span class:first={tk === 0} style:left="{xOf(tk)}px">{formatTick(tk, tickStep)}</span>{/each}
				</div>
			</div>
			<div
				class="gutter-handle"
				style:left="{gutterW}px"
				onpointerdown={(e) => {
					gutterDragging = true;
					(e.target as HTMLElement).setPointerCapture(e.pointerId);
				}}
				onpointermove={onGutter}
				onpointerup={() => (gutterDragging = false)}
				ondblclick={() => (gutterW = autoFit())}
				role="separator"
				aria-label="Resize the node column (double-click to fit)"
				tabindex="-1"
			></div>
			{#each rows as row (row.type === 'group' ? `g-${row.group.runId}` : `${row.node.runId}-${row.node.nodeKey}`)}
				{#if row.type === 'group'}
					<div class="row group-row">
						<div class="gutter cell" class:shadow={scrolledX} style:width="{gutterW}px">
							<i style:background={row.group.tone}></i>
							<a href={row.group.href}>{row.group.label}</a>
							<span class="faint">{row.count}</span>
						</div>
					</div>
				{:else}
					{@const n = row.node}
					{@const s = startOf(n)}
					{@const e = endOf(n)}
					<button class="row" class:sel={selected.includes(n.nodeKey)} onclick={(ev) => onselect(n.nodeKey, ev.metaKey || ev.ctrlKey || ev.shiftKey)}>
						<span
							class="gutter cell"
							class:shadow={scrolledX}
							style:width="{gutterW}px"
							style:color={n.status === 'failed' ? 'var(--sb-error)' : s == null ? 'var(--sb-text-faint)' : undefined}
							title={n.nodeKey}
						>
							{#if selected.includes(n.nodeKey)}<span class="sel-bar"></span>{/if}
							{n.nodeKey}{#if n.critical}<em title="Marked critical: if it fails, the whole group stops">critical</em>{/if}
						</span>
						<span class="track" style:width="{trackW}px">
							{#if s == null || e == null}
								<span class="note">{skipText(n)}</span>
							{:else}
								<span class="bar" class:live={n.status === 'running'} style:left="{xOf(s)}px" style:width="{Math.max(xOf(e - s), 3)}px" style:background={nodeStatusTone[n.status]}></span>
								<span class="dur" style:left="{xOf(e) > trackW - 56 ? xOf(s) - 8 : xOf(e) + 8}px" class:flip={xOf(e) > trackW - 56}>{formatSpan(e - s)}{n.status === 'running' ? ' so far' : ''}</span>
							{/if}
						</span>
					</button>
				{/if}
			{/each}
		</div>
	</div>
</div>
</div>

<style>
	.gantt {
		display: flex;
		min-height: 0;
		flex: 1;
		flex-direction: column;
	}
	.gantt-head {
		display: flex;
		justify-content: flex-end;
		align-items: center;
		gap: 6px;
		padding: 4px 14px;
		border-bottom: 1px solid var(--border-subtle);
	}
	.count {
		margin-right: 6px;
		color: var(--sb-text-faint);
		font-size: 11px;
	}
	.zoom {
		display: flex;
		align-items: center;
		overflow: hidden;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--secondary);
	}
	.zoom button,
	.fit {
		height: 24px;
		padding: 0 8px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.fit {
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--secondary);
		font-size: 11px;
	}
	.zoom button:disabled,
	.fit:disabled {
		opacity: 0.4;
	}
	.zoom input {
		width: 84px;
		height: 3px;
		margin: 0 6px;
		appearance: none;
		border-radius: 2px;
		background: var(--border-strong);
	}
	.zoom input::-webkit-slider-thumb {
		width: 11px;
		height: 11px;
		appearance: none;
		border-radius: 50%;
		background: var(--primary);
	}
	.pane {
		position: relative;
		min-height: 0;
		flex: 1;
	}
	.probe {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}
	.scroller {
		position: absolute;
		inset: 0;
		overflow: auto;
	}
	.content {
		position: relative;
		min-width: 100%;
	}
	.grid {
		position: absolute;
		top: 28px;
		bottom: 0;
		pointer-events: none;
	}
	.grid span {
		position: absolute;
		top: 0;
		bottom: 0;
		width: 1px;
		background: var(--sb-grid);
	}
	.axis {
		position: sticky;
		top: 0;
		z-index: 30;
		display: flex;
		height: 28px;
		border-bottom: 1px solid var(--border);
		background: var(--card);
	}
	.gutter {
		position: sticky;
		left: 0;
		z-index: 20;
		flex-shrink: 0;
		border-right: 1px solid var(--border);
		background: var(--sb-surface-low);
	}
	.gutter.shadow {
		box-shadow: 6px 0 8px -6px rgba(0, 0, 0, 0.55);
	}
	.axis-gutter {
		display: flex;
		align-items: center;
		padding-left: 24px;
		color: var(--sb-text-faint);
		font-size: 11.5px;
		font-weight: 500;
	}
	.axis-track {
		position: relative;
	}
	.axis-track span {
		position: absolute;
		top: 50%;
		transform: translate(-50%, -50%);
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
	.axis-track span.first {
		padding-left: 6px;
		transform: translateY(-50%);
	}
	.gutter-handle {
		position: absolute;
		top: 0;
		bottom: 0;
		z-index: 40;
		width: 8px;
		margin-left: -4px;
		cursor: col-resize;
	}
	.gutter-handle:hover::after {
		position: absolute;
		top: 0;
		bottom: 0;
		left: 50%;
		width: 1px;
		background: var(--primary);
		content: '';
	}
	.row {
		position: relative;
		display: flex;
		width: 100%;
		height: 32px;
		align-items: center;
		border-bottom: 1px solid var(--border-subtle);
		text-align: left;
	}
	button.row:hover,
	button.row:hover .gutter,
	.row.sel,
	.row.sel .gutter {
		background: var(--sb-hover);
	}
	.cell {
		display: flex;
		height: 100%;
		align-items: center;
		gap: 6px;
		overflow: hidden;
		padding: 0 12px 0 24px;
		color: var(--muted-foreground);
		font-family: var(--font-mono);
		font-size: 12px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.cell em {
		padding: 0 4px;
		border-radius: 2px;
		background: color-mix(in srgb, var(--sb-warning) 14%, transparent);
		color: var(--sb-warning);
		font-family: var(--font-sans);
		font-size: 10.5px;
		font-style: normal;
	}
	.sel-bar {
		position: absolute;
		top: 3px;
		bottom: 3px;
		left: 0;
		width: 2px;
		background: var(--primary);
	}
	.group-row {
		height: 28px;
		background: var(--sb-surface-low);
	}
	.group-row .cell {
		font-family: var(--font-sans);
		font-weight: 600;
		color: var(--foreground);
	}
	.group-row i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
	}
	.group-row a:hover {
		color: var(--primary);
	}
	.faint {
		color: var(--sb-text-faint);
		font-weight: 400;
	}
	.track {
		position: relative;
		height: 100%;
	}
	.bar {
		position: absolute;
		top: 50%;
		height: 16px;
		transform: translateY(-50%);
		border-radius: 2px;
		box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.18);
	}
	.bar.live {
		background-image: linear-gradient(90deg, transparent 0 60%, rgba(255, 255, 255, 0.28) 60% 80%, transparent 80%) !important;
		background-size: 24px 100%;
		animation: slide 1.2s linear infinite;
	}
	@keyframes slide {
		to {
			background-position: 24px 0;
		}
	}
	.dur,
	.note {
		position: absolute;
		top: 50%;
		transform: translateY(-50%);
		color: var(--sb-text-faint);
		font-size: 11.5px;
		white-space: nowrap;
	}
	.dur.flip {
		transform: translate(-100%, -50%);
	}
	.note {
		left: 8px;
	}
	.sb-scroll {
		scrollbar-width: thin;
		scrollbar-color: var(--border-strong) transparent;
	}
</style>
