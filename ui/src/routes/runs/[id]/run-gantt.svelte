<script lang="ts">
	import { plural } from '$lib/format/plural';
	import MinusIcon from '@lucide/svelte/icons/minus';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import { onMount } from 'svelte';
	import type { RunNode } from '$lib/api';
	import { nodeLabel, nodeStatusTone } from '$lib/operate/labels';
	import { axisTicks as ticksFor, formatSpan, formatTick, tickStep as stepFor } from './utils';

	export type GanttGroup = { runId: string; label: string; tone: string; href: string };

	let {
		nodes,
		origin,
		now,
		selected = [],
		groups = [],
		active = false,
		skipText: causeOf = () => null,
		onselect,
		onopen
	}: {
		nodes: RunNode[];
		/** Time zero of the axis (ms): the first node's start. */
		origin: number;
		now: number;
		selected?: string[];
		/** Child runs of a group, to section the rows. */
		groups?: GanttGroup[];
		/** The run is still going, so a node that hasn't started is waiting, not left out. */
		active?: boolean;
		/** Why a node was skipped, on one line, chain included; null for the plain status. */
		skipText?: (nodeKey: string) => string | null;
		/** The tick box: adds the node to the selection, or takes it out. */
		onselect: (nodeKey: string) => void;
		/** A click on the row: the node's details, in the drawer. */
		onopen: (nodeKey: string) => void;
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
	const GUTTER_MAX = 720;

	const trackBaseW = $derived(Math.max(120, viewportW - gutterW - 24));
	const trackW = $derived(trackBaseW * zoom);
	const pxPerSec = $derived(trackW / (total * 1.08));
	const xOf = (sec: number) => sec * pxPerSec;
	/** A bar's duration reads on its right unless it would run past the row, which ends 24px past the
	 * track and clips what overflows (`content-visibility`); then it reads on the bar's left if there is
	 * more room there. */
	let durCtx: CanvasRenderingContext2D | null = null;
	function durFlips(text: string, start: number, end: number): boolean {
		durCtx ??= document.createElement('canvas').getContext('2d');
		if (!durCtx) return false;
		durCtx.font = '11.5px "Inter Variable", sans-serif';
		// Labels set tabular figures, which a canvas can't, so zeros stand in for the digits.
		const width = Math.ceil(durCtx.measureText(text.replace(/[0-9]/g, '0')).width) + 1;
		const right = trackW + 24 - xOf(end) - 8;
		return width > right && xOf(start) - 8 > right;
	}
	const tickStep = $derived(stepFor(total, Math.max(4, Math.floor(trackW / 110))));
	const axisTicks = $derived(ticksFor(total, tickStep));

	function autoFit(): number {
		const ctx = document.createElement('canvas').getContext('2d');
		if (!ctx) return 250;
		ctx.font = '12px "IBM Plex Mono", ui-monospace, monospace';
		const widest = Math.max(0, ...nodes.map((n) => ctx.measureText(nodeLabel(n.nodeKey)).width));
		// Names are never cut: the column fits the longest, up to the most it may take.
		return Math.min(GUTTER_MAX, Math.max(GUTTER_MIN, Math.ceil(widest) + 84));
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

	// Rows run in start order, so the bars read as a cascade; nodes that never started go last.
	const ordered = $derived([...nodes].sort((a, b) => (a.startedAt ?? '\uffff').localeCompare(b.startedAt ?? '\uffff')));

	type Row = { type: 'group'; group: GanttGroup; count: number } | { type: 'node'; node: RunNode };
	/** Job runs whose node rows are folded away. */
	let collapsed = $state<Set<string>>(new Set());
	function toggleGroup(runId: string) {
		const next = new Set(collapsed);
		if (!next.delete(runId)) next.add(runId);
		collapsed = next;
	}
	const rows = $derived.by((): Row[] => {
		if (groups.length < 2) return ordered.map((node) => ({ type: 'node', node }));
		return groups.flatMap((group): Row[] => {
			const own = ordered.filter((n) => n.runId === group.runId);
			const head: Row = { type: 'group', group, count: own.length };
			return collapsed.has(group.runId) ? [head] : [head, ...own.map((node): Row => ({ type: 'node', node }))];
		});
	});

	function skipText(n: RunNode): string {
		const cause = causeOf(n.nodeKey);
		if (cause) return cause;
		if (n.status === 'skipped' && n.skippedBecauseOf) {
			const across = n.skippedBecauseOf.runId !== n.runId;
			return `skipped: ${n.skippedBecauseOf.nodeKey} failed${across ? ` in ${n.skippedBecauseOf.jobKey ?? n.skippedBecauseOf.runId}` : ''}`;
		}
		if (n.status === 'pending') return active ? 'Waiting' : 'Not run';
		if (n.status === 'not_run') return 'Not run';
		if (n.status === 'cancelled') return 'Cancelled';
		return n.status;
	}
</script>

<div class="gantt">
<div class="gantt-head">
	<span class="count">{plural(nodes.length, 'node')}</span>
	<div class="zoom">
		<Button variant="ghost" size="icon-sm" onclick={() => zoomBy(1 / 1.5)} disabled={zoom <= ZOOM_MIN} title="Zoom out" aria-label="Zoom out"><MinusIcon /></Button>
		<input type="range" min={ZOOM_MIN} max={ZOOM_MAX} step="0.1" bind:value={zoom} title="Zoom" />
		<Button variant="ghost" size="icon-sm" onclick={() => zoomBy(1.5)} disabled={zoom >= ZOOM_MAX} title="Zoom in" aria-label="Zoom in"><PlusIcon /></Button>
	</div>
	<Button size="sm" onclick={() => ((zoom = 1), ganttEl && (ganttEl.scrollLeft = 0))} disabled={zoom === 1}>Fit</Button>
</div>

<div class="pane">
	<div bind:this={measureEl} class="probe"></div>
	<div bind:this={ganttEl} class="sb-scroll scroller" class:select-none={gutterDragging} onscroll={(e) => (scrolledX = e.currentTarget.scrollLeft > 1)}>
		<!-- Bars are placed by their times in seconds and one --pps (pixels per second), so zooming changes one value, not every bar. -->
		<div class="content" style:width="{gutterW + trackW + 24}px" style:--pps={pxPerSec} style:--track-w="{trackW}px">
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
							<!-- raw-button: the fold control in a group row of the chart's name column -->
							<button
								class="fold"
								class:open={!collapsed.has(row.group.runId)}
								onclick={() => toggleGroup(row.group.runId)}
								aria-expanded={!collapsed.has(row.group.runId)}
								aria-label="{collapsed.has(row.group.runId) ? 'Show' : 'Hide'} the nodes of {row.group.label}"
							>
								<ChevronRightIcon size={13} />
							</button>
							<i style:background={row.group.tone}></i>
							<a href={to(row.group.href)}>{row.group.label}</a>
							<span class="faint">{row.count}</span>
						</div>
					</div>
				{:else}
					{@const n = row.node}
					{@const s = startOf(n)}
					{@const e = endOf(n)}
					<!-- A row of the chart: a click opens its node's details; its tick box selects it. -->
					<div class="row" class:sel={selected.includes(n.nodeKey)} role="button" tabindex="0" onclick={() => onopen(n.nodeKey)} onkeydown={(ev) => (ev.key === 'Enter' || ev.key === ' ') && onopen(n.nodeKey)} title="Show {n.nodeKey}'s details">
						<span
							class="gutter cell"
							class:shadow={scrolledX}
							style:width="{gutterW}px"
							style:color={n.status === 'failed' ? 'var(--sb-error)' : s == null ? 'var(--sb-text-faint)' : undefined}
							title={n.nodeKey}
						>
							{#if selected.includes(n.nodeKey)}<span class="sel-bar"></span>{/if}
							<input
								type="checkbox"
								class="sb-check tick"
								checked={selected.includes(n.nodeKey)}
								onclick={(ev) => {
									ev.stopPropagation();
									onselect(n.nodeKey);
								}}
								aria-label="Select {n.nodeKey}"
								title="Add to the selection, or take it out"
							/>
							{nodeLabel(n.nodeKey)}{#if n.critical}<em title="Marked critical: if it fails, the whole group stops">critical</em>{/if}
						</span>
						<span class="track">
							{#if s == null || e == null}
								<span class="note">{skipText(n)}</span>
							{:else}
								<span class="bar" class:live={n.status === 'running'} style:--s={s} style:--e={e} style:background={nodeStatusTone[n.status]}></span>
								{@const dur = `${formatSpan(e - s)}${n.status === 'running' ? ' so far' : ''}`}
								<span class="dur" style:--s={s} style:--e={e} class:flip={durFlips(dur, s, e)}>{dur}</span>
							{/if}
						</span>
					</div>
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
		border: 1px solid var(--input);
		border-radius: 2px;
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
	/* Rows out of view are neither laid out nor painted, so scrolling a long run, or zooming it out, only
	   draws the rows on screen; each row's sticky name cell would otherwise be its own compositor layer. */
	.row {
		position: relative;
		display: flex;
		width: 100%;
		height: 32px;
		content-visibility: auto;
		contain-intrinsic-size: auto 32px;
		align-items: center;
		border-bottom: 1px solid var(--border-subtle);
		text-align: left;
	}
	.row[role="button"] {
		cursor: pointer;
	}
	.row[role="button"]:hover,
	.row[role="button"]:hover .gutter,
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
		padding: 0 12px 0 10px;
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
	.fold {
		display: grid;
		width: 16px;
		height: 16px;
		margin-left: -4px;
		place-items: center;
		color: var(--muted-foreground);
	}
	.fold:hover {
		color: var(--foreground);
	}
	.fold :global(svg) {
		transition: transform 0.12s;
	}
	.fold.open :global(svg) {
		transform: rotate(90deg);
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
		width: var(--track-w);
		height: 100%;
	}
	.bar {
		position: absolute;
		top: 50%;
		left: calc(var(--s) * var(--pps) * 1px);
		width: max(3px, calc((var(--e) - var(--s)) * var(--pps) * 1px));
		height: 16px;
		transform: translateY(-50%);
		border-radius: 2px;
		box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.18);
	}
	/* A running bar's stripes move by transform, which the compositor animates without repainting. */
	.bar.live {
		overflow: hidden;
	}
	.bar.live::after {
		position: absolute;
		inset: 0 0 0 -24px;
		background-image: linear-gradient(90deg, transparent 0 60%, rgba(255, 255, 255, 0.28) 60% 80%, transparent 80%);
		background-size: 24px 100%;
		animation: slide 1.2s linear infinite;
		content: '';
		will-change: transform;
	}
	@keyframes slide {
		to {
			transform: translateX(24px);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.bar.live::after {
			animation: none;
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
	.dur {
		left: calc(var(--e) * var(--pps) * 1px + 8px);
	}
	.dur.flip {
		left: calc(var(--s) * var(--pps) * 1px - 8px);
		transform: translate(-100%, -50%);
	}
	.note {
		left: 8px;
	}
	.tick {
		flex: none;
		margin: 0 2px 0 0;
	}
</style>
