<script lang="ts">
	import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import SearchIcon from '@lucide/svelte/icons/search';
	import type { Snippet } from 'svelte';
	import type { TimelineBar, TimelineTickMarker } from '$lib/api';
	import { createResource } from '$lib/state/resource.svelte';
	import { runKindLabel, runOriginLabel, runStatusLabel, runStatusRank, runStatusTone } from '$lib/status';

	let {
		query = $bindable(''),
		showSearch = true,
		maxHeight,
		initialWindow = '12h',
		scroll = true,
		stickyTop = 0,
		actions
	}: {
		/** false = no inner scroll; the header sticks to the page's scroll container instead. */
		scroll?: boolean;
		stickyTop?: number;
		query?: string;
		showSearch?: boolean;
		/** Cap the scroll body (px). Omit to fill the parent. */
		maxHeight?: number;
		initialWindow?: WindowKey;
		actions?: Snippet;
	} = $props();

	type WindowKey = '1h' | '6h' | '12h' | '24h' | '7d';
	const MIN = 60_000;
	const HOUR = 60 * MIN;
	const WINDOWS: Record<WindowKey, { span: number; tick: number }> = {
		'1h': { span: HOUR, tick: 10 * MIN },
		'6h': { span: 6 * HOUR, tick: HOUR },
		'12h': { span: 12 * HOUR, tick: HOUR },
		'24h': { span: 24 * HOUR, tick: 2 * HOUR },
		'7d': { span: 7 * 24 * HOUR, tick: 24 * HOUR }
	};

	// svelte-ignore state_referenced_locally
	let windowKey = $state<WindowKey>(initialWindow);
	let pan = $state(0); // number of half-windows moved into the past

	// The server's clock at load; the window is anchored to it so history and
	// projected ticks line up.
	let anchor = $state(Date.now());
	const span = $derived(WINDOWS[windowKey].span);
	// keep a little future on screen so upcoming ticks are visible
	const end = $derived(anchor + span * 0.08 - pan * (span / 2));
	const start = $derived(end - span);
	const pct = (t: number) => ((t - start) / span) * 100;

	const timeline = createResource(
		(client, scope) =>
			client.timeline(scope, { since: new Date(start).toISOString(), until: new Date(end).toISOString() }),
		['runs', 'ticks']
	);
	const now = $derived(timeline.data ? Date.parse(timeline.data.now) : anchor);
	$effect(() => {
		if (timeline.data) anchor = Date.parse(timeline.data.now);
	});
	const nowPct = $derived(pct(now));

	type Bar = TimelineBar & { startMs: number; endMs: number; key: string };
	const bars = $derived<Bar[]>(
		(timeline.data?.bars ?? []).map((b) => ({
			...b,
			startMs: Date.parse(b.start),
			endMs: Date.parse(b.end),
			key: `${b.runId ?? 'tick'}-${b.jobKey}-${b.start}`
		}))
	);

	const ticks = $derived.by(() => {
		const step = WINDOWS[windowKey].tick;
		const first = new Date(start);
		if (step >= 24 * HOUR) first.setHours(0, 0, 0, 0);
		else first.setMinutes(0, 0, 0);
		const out: { t: number; label: string }[] = [];
		for (let t = first.getTime(); t <= end; t += step) {
			if (t < start) continue;
			const d = new Date(t);
			out.push({
				t,
				label:
					step >= 24 * HOUR
						? d.toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric' })
						: `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
			});
		}
		return out;
	});

	const days = $derived.by(() => {
		if (windowKey === '7d') return [];
		const out: { left: number; label: string }[] = [];
		const d = new Date(start);
		d.setHours(0, 0, 0, 0);
		for (let t = d.getTime(); t < end; t += 24 * HOUR) {
			out.push({
				left: Math.max(0, pct(t)),
				label: new Date(t).toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short' })
			});
		}
		if (out.length > 1 && out[0].left === 0 && out[1].left < 14) out.shift();
		return out;
	});

	type Marker = TimelineTickMarker & { atMs: number };
	const markersByJob = $derived.by(() => {
		const m = new Map<string, Marker[]>();
		for (const k of timeline.data?.markers ?? []) {
			const atMs = Date.parse(k.scheduledFor);
			if (atMs < start || atMs > end) continue;
			m.set(k.jobKey, [...(m.get(k.jobKey) ?? []), { ...k, atMs }]);
		}
		return m;
	});
	let markerHover = $state<{ marker: Marker; x: number; y: number } | null>(null);

	const visibleRuns = $derived(bars.filter((r) => r.endMs > start && r.startMs < end));
	const runsByJob = $derived.by(() => {
		const m = new Map<string, Bar[]>();
		for (const r of visibleRuns) m.set(r.jobKey, [...(m.get(r.jobKey) ?? []), r]);
		return m;
	});
	const counts = $derived({
		running: visibleRuns.filter((r) => r.status === 'running').length,
		ok: visibleRuns.filter((r) => r.status === 'success').length,
		warn: visibleRuns.filter((r) => r.status === 'warn').length,
		fail: visibleRuns.filter((r) => r.status === 'failed' || r.status === 'partial' || r.status === 'presumed_failed').length
	});

	// jobs with problems in the window first, then alphabetical
	const severity = (job: string) =>
		Math.max(0, ...(runsByJob.get(job) ?? []).map((r) => (r.status === 'projected' ? 0 : runStatusRank(r.status))));
	const q = $derived(query.trim().toLowerCase());
	const jobs = $derived(
		(timeline.data?.jobKeys ?? [])
			.filter((j) => !q || j.includes(q))
			.sort((a, b) => severity(b) - severity(a) || a.localeCompare(b))
			.map((name) => ({ name }))
	);

	const tone = (r: Bar) => (r.status === 'projected' ? 'transparent' : runStatusTone[r.status]);
	const label = (r: Bar) => (r.status === 'projected' ? 'Scheduled' : runStatusLabel[r.status]);
	const barClass = (r: Bar) => (r.status === 'projected' ? 'scheduled' : r.status === 'success' ? 'ok' : r.status);

	let innerW = $state(1600);
	let hover = $state<{ run: Bar; x: number; y: number } | null>(null);
	const clock = (t: number) =>
		new Date(t).toLocaleString('en-GB', { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' });
	function duration(r: Bar) {
		const m = Math.round((r.endMs - r.startMs) / MIN);
		return m < 60 ? `${m}m` : `${Math.floor(m / 60)}h ${m % 60}m`;
	}
</script>

<div class="tl" class:fill={scroll && maxHeight == null} style:--sticky-top="{stickyTop}px">
	<div class="tl-controls">
		{#if showSearch}
			<label class="tl-search">
				<SearchIcon size={13} />
				<input bind:value={query} placeholder="Search jobs" />
			</label>
		{/if}
		<div class="seg">
			{#each Object.keys(WINDOWS) as w (w)}
				<button class:on={windowKey === w} onclick={() => ((windowKey = w as WindowKey), (pan = 0))}>{w}</button>
			{/each}
		</div>
		<div class="seg">
			<button onclick={() => pan++} aria-label="Earlier"><ChevronLeftIcon size={14} /></button>
			<button class:on={pan === 0} onclick={() => (pan = 0)}>Now</button>
			<button onclick={() => (pan = Math.max(0, pan - 1))} disabled={pan === 0} aria-label="Later"><ChevronRightIcon size={14} /></button>
		</div>
		{#if actions}<div class="tl-actions">{@render actions()}</div>{/if}
	</div>

	<div class="tl-scroll" class:sb-scroll={scroll} class:static={!scroll} style:max-height={maxHeight ? `${maxHeight}px` : undefined}>
		<div class="tl-head">
			<div class="gutter">Jobs <span>{jobs.length}</span></div>
			<div class="scale">
				<div class="days">
					{#each days as d (d.label)}<span style:left="{d.left}%">{d.label}</span>{/each}
					{#if nowPct >= 0 && nowPct <= 100}
						<div class="now-counts" style:left="{nowPct}%">
							{#if counts.running}<b style:--c="var(--sb-running)" title="Running">{counts.running}</b>{/if}
							<b style:--c="var(--sb-success)" title="Succeeded">{counts.ok}</b>
							{#if counts.warn}<b style:--c="var(--sb-warning)" title="Warnings">{counts.warn}</b>{/if}
							{#if counts.fail}<b style:--c="var(--sb-error)" title="Failed">{counts.fail}</b>{/if}
						</div>
					{/if}
				</div>
				<div class="ticks">
					{#each ticks.filter((t) => pct(t.t) < 97 && Math.abs(pct(t.t) - nowPct) > 3) as t (t.t)}<span style:left="{pct(t.t)}%">{t.label}</span>{/each}
					{#if nowPct >= 0 && nowPct <= 100}<em class="now-tag" style:left="{nowPct}%">Now</em>{/if}
				</div>
			</div>
		</div>

		<div class="tl-body">
			<div class="overlay">
				{#each ticks as t (t.t)}<i class="gridline" style:left="{pct(t.t)}%"></i>{/each}
				{#if nowPct >= 0 && nowPct <= 100}<i class="now-line" style:left="{nowPct}%"></i>{/if}
			</div>
			{#each jobs as job (job.name)}
				<div class="row">
					<a class="gutter job" href="/jobs/{job.name}" title={job.name}>{job.name}</a>
					<div class="track">
						{#each runsByJob.get(job.name) ?? [] as r (r.key)}
							{@const left = Math.max(0, pct(r.startMs))}
							<a
								class="bar {barClass(r)}"
								href={r.runId ? `/runs/${r.runId}` : `/jobs/${r.jobKey}`}
								style:left="{left}%"
								style:width="{Math.min(100, pct(r.endMs)) - left}%"
								style:--c={tone(r)}
								onmouseenter={(e) => (hover = { run: r, x: e.clientX, y: e.clientY })}
								onmousemove={(e) => (hover = { run: r, x: e.clientX, y: e.clientY })}
								onmouseleave={() => (hover = null)}
								aria-label="{r.jobKey} {label(r)} {clock(r.startMs)}"
							></a>
						{/each}
						{#each markersByJob.get(job.name) ?? [] as m (m.tickId)}
							<a
								class="marker {m.outcome}"
								href="/triggers/{m.triggerId}"
								style:left="{pct(m.atMs)}%"
								onmouseenter={(e) => (markerHover = { marker: m, x: e.clientX, y: e.clientY })}
								onmouseleave={() => (markerHover = null)}
								aria-label="{m.triggerName} due {clock(m.atMs)}: {m.outcome === 'covered' ? 'merged into a waiting run' : 'skipped'}"
							></a>
						{/each}
					</div>
				</div>
			{:else}
				<div class="empty">
					{#if timeline.error}Couldn't load the timeline: {timeline.error}{:else if !timeline.data}Loading…{:else if query}No jobs match “{query}”.{:else}No runs in this window.{/if}
				</div>
			{/each}
		</div>
	</div>
</div>

<svelte:window bind:innerWidth={innerW} />

{#if hover}
	{@const r = hover.run}
	<div class="tl-tip" style:left="{hover.x + 270 > innerW ? hover.x - 254 : hover.x + 14}px" style:top="{hover.y + 16}px">
		<div class="tip-head"><i style:background={r.status === 'projected' ? 'var(--sb-text-faint)' : tone(r)}></i><b>{label(r)}</b><span>{r.jobKey}</span></div>
		<dl>
			{#if r.runId}<div><dt>Run</dt><dd class="mono">{r.runId.replace(/^run-/, '')}</dd></div>{/if}
			<div><dt>{r.status === 'projected' ? 'Due' : 'Started'}</dt><dd>{clock(r.startMs)}</dd></div>
			<div><dt>Duration</dt><dd>{r.status === 'projected' ? `~${duration(r)} typical` : r.status === 'running' ? `${duration(r)} so far` : duration(r)}</dd></div>
			<div><dt>Kind</dt><dd>{runKindLabel[r.kind]}</dd></div>
			<div><dt>Started by</dt><dd>{r.triggerName ?? runOriginLabel[r.origin]}</dd></div>
		</dl>
	</div>
{/if}

{#if markerHover}
	{@const m = markerHover.marker}
	<div class="tl-tip" style:left="{markerHover.x + 270 > innerW ? markerHover.x - 254 : markerHover.x + 14}px" style:top="{markerHover.y + 16}px">
		<div class="tip-head"><i style:background={m.outcome === 'covered' ? 'var(--sb-running)' : 'var(--sb-text-faint)'}></i><b>{m.outcome === 'covered' ? 'Merged into a waiting run' : 'Skipped'}</b><span>{m.triggerName}</span></div>
		<dl>
			<div><dt>Due</dt><dd>{clock(m.atMs)}</dd></div>
			{#if m.coveredByRunId}<div><dt>Merged into</dt><dd class="mono">run {m.coveredByRunId.replace(/^run-/, '')} (already waiting)</dd></div>{/if}
			{#if m.reason && m.outcome !== 'covered'}<div><dt>Why</dt><dd>{m.reason}</dd></div>{/if}
		</dl>
	</div>
{/if}

<style>
	.tl {
		--gutter: 200px;
		display: flex;
		min-height: 0;
		flex-direction: column;
	}
	.tl.fill {
		flex: 1;
	}
	.tl-controls {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 10px 14px;
		border-bottom: 1px solid var(--border);
	}
	.tl-search {
		display: flex;
		width: 240px;
		height: 28px;
		align-items: center;
		gap: 7px;
		padding: 0 9px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--muted-foreground);
	}
	.tl-search input {
		min-width: 0;
		flex: 1;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--foreground);
		font-size: 12.5px;
	}
	.seg {
		display: flex;
		gap: 2px;
		padding: 2px;
		border-radius: 2px;
		background: var(--sb-hover);
	}
	.seg button {
		display: inline-flex;
		height: 24px;
		min-width: 28px;
		align-items: center;
		justify-content: center;
		padding: 0 8px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.seg button:hover:not(:disabled) {
		color: var(--foreground);
	}
	.seg button:disabled {
		opacity: 0.4;
	}
	.seg button.on {
		background: var(--card);
		color: var(--foreground);
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
	}
	.tl-actions {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-left: auto;
	}
	.tl-scroll {
		position: relative;
		min-height: 0;
		flex: 1;
		overflow-y: auto;
	}
	.tl-scroll.static {
		overflow: visible;
	}
	.tl-head {
		position: sticky;
		z-index: 3;
		top: var(--sticky-top);
		display: flex;
		border-bottom: 1px solid var(--border);
		background: var(--card);
	}
	.gutter {
		width: var(--gutter);
		flex: 0 0 var(--gutter);
		border-right: 1px solid var(--border);
	}
	.tl-head .gutter {
		display: flex;
		align-items: flex-end;
		gap: 6px;
		padding: 0 14px 7px;
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 600;
	}
	.tl-head .gutter span {
		font-weight: 400;
	}
	.scale {
		position: relative;
		flex: 1;
		overflow: hidden;
	}
	.days,
	.ticks {
		position: relative;
		height: 22px;
	}
	.days span {
		position: absolute;
		top: 5px;
		padding-left: 6px;
		border-left: 1px solid var(--border);
		color: var(--muted-foreground);
		font-size: 11.5px;
		font-weight: 500;
		white-space: nowrap;
	}
	.ticks span {
		position: absolute;
		top: 3px;
		padding-left: 5px;
		color: var(--sb-text-faint);
		font-size: 11px;
		white-space: nowrap;
	}
	.now-tag {
		position: absolute;
		top: 3px;
		padding: 0 5px;
		transform: translateX(-50%);
		border-radius: 2px;
		background: var(--primary);
		color: var(--primary-foreground);
		font: normal 600 10.5px/16px var(--font-sans);
	}
	.now-counts {
		position: absolute;
		top: 3px;
		display: flex;
		gap: 3px;
		padding-right: 6px;
		transform: translateX(-100%);
	}
	.now-counts b {
		min-width: 20px;
		padding: 0 5px;
		border-radius: 2px;
		background: color-mix(in srgb, var(--c) 18%, transparent);
		color: var(--c);
		font: 600 10.5px/16px var(--font-sans);
		text-align: center;
		font-variant-numeric: tabular-nums;
	}
	.tl-body {
		position: relative;
	}
	.overlay {
		position: absolute;
		z-index: 0;
		inset: 0 0 0 var(--gutter);
		pointer-events: none;
	}
	.gridline,
	.now-line {
		position: absolute;
		top: 0;
		bottom: 0;
		width: 1px;
	}
	.gridline {
		background: var(--inner-line);
	}
	.now-line {
		z-index: 2;
		background: var(--primary);
	}
	.row {
		display: flex;
		height: 28px;
		border-bottom: 1px solid var(--inner-line);
	}
	.row:hover {
		background: color-mix(in srgb, var(--sb-hover) 55%, transparent);
	}
	.job {
		display: flex;
		align-items: center;
		overflow: hidden;
		padding: 0 14px;
		color: var(--foreground);
		font-size: 12.5px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.job:hover {
		color: var(--primary);
	}
	.track {
		position: relative;
		z-index: 1;
		flex: 1;
		overflow: hidden;
	}
	.bar {
		position: absolute;
		top: 7px;
		height: 14px;
		min-width: 3px;
		border-radius: 2px;
		background: var(--c);
	}
	.bar.ok {
		background: color-mix(in srgb, var(--c) 82%, var(--card));
	}
	.bar:hover {
		outline: 2px solid color-mix(in srgb, var(--c) 45%, transparent);
		filter: brightness(1.08);
	}
	.bar.scheduled {
		border: 1px dashed var(--sb-text-faint);
		background: transparent;
	}
	.bar.scheduled:hover {
		outline: 0;
		border-color: var(--primary);
	}
	.marker {
		position: absolute;
		z-index: 2;
		top: 9px;
		width: 9px;
		height: 9px;
		margin-left: -4.5px;
		transform: rotate(45deg);
		border: 1.5px solid var(--sb-running);
		background: var(--card);
	}
	.marker.skipped {
		border-color: var(--sb-text-faint);
		border-style: dashed;
	}
	.marker:hover {
		background: var(--sb-hover);
		border-color: var(--primary);
	}
	.empty {
		padding: 28px;
		color: var(--muted-foreground);
		font-size: 12.5px;
		text-align: center;
	}
	.tl-tip {
		position: fixed;
		z-index: 50;
		width: 240px;
		padding: 9px 11px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--popover, var(--card));
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.22);
		color: var(--foreground);
		font-size: 12px;
		pointer-events: none;
	}
	.tip-head {
		display: flex;
		align-items: center;
		gap: 7px;
		margin-bottom: 6px;
	}
	.tip-head i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
	}
	.tip-head span {
		color: var(--muted-foreground);
	}
	.tl-tip dl {
		margin: 0;
	}
	.tl-tip dl div {
		display: grid;
		grid-template-columns: 78px 1fr;
		padding: 2px 0;
	}
	.tl-tip dt {
		color: var(--muted-foreground);
	}
	.tl-tip dd {
		margin: 0;
	}
	.tl-tip .mono {
		font: 11.5px var(--font-mono);
	}
</style>
