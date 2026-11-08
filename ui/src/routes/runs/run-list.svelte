<script lang="ts">
	import CalendarClockIcon from '@lucide/svelte/icons/calendar-clock';
	import RadarIcon from '@lucide/svelte/icons/radar';
	import UserIcon from '@lucide/svelte/icons/user';
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import type { RunRow } from '$lib/api';
	import CopyButton from '$lib/components/copy-button.svelte';
	import RunStatusPill from '$lib/components/operate/run-status-pill.svelte';
	import { formatAgo, formatDuration, formatStamp } from '$lib/format/time';
	import { shortRunId } from '$lib/operate/labels';
	import { app } from '$lib/state/app.svelte';
	import { runKindLabel, runStatusTone } from '$lib/status';
	import type { RunsState } from './state.svelte';
	import type { TokenKey } from './types';
	import { statusLine } from './utils';

	let { list, onfilter, emptyText }: { list: RunsState; onfilter: (key: TokenKey, value: string) => void; emptyText: string } = $props();

	const ROW_H = 52;
	const OVERSCAN = 8;
	let scroller = $state<HTMLDivElement>();
	let scrollTop = $state(0);
	let viewH = $state(600);

	onMount(() => {
		const el = scroller;
		if (!el) return;
		const ro = new ResizeObserver(() => (viewH = el.clientHeight));
		ro.observe(el);
		viewH = el.clientHeight;
		return () => ro.disconnect();
	});

	const rows = $derived(list.display);
	const first = $derived(Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN));
	const last = $derived(Math.min(rows.length, Math.ceil((scrollTop + viewH) / ROW_H) + OVERSCAN));
	const visible = $derived(rows.slice(first, last));

	// Fetch the next cursor page when the window nears the end.
	$effect(() => {
		if (list.hasMore && !list.loadingMore && !list.loading && last >= rows.length - OVERSCAN) void list.loadMore();
	});

	const CONTROL = 'a, button, input, label, [data-control]';
	function open(e: MouseEvent, id: string) {
		if ((e.target as Element).closest(CONTROL)) return;
		const href = `/runs/${id}`;
		if (e.metaKey || e.ctrlKey || e.button === 1) window.open(href, '_blank', 'noopener');
		else void goto(href);
	}

	const toneColor = { muted: 'var(--muted-foreground)', warn: 'var(--sb-warning)', error: 'var(--sb-error)', live: 'var(--sb-running)' };
	const kindToken = (r: RunRow) => (r.run.kind === 'adhoc' ? 'adhoc' : r.run.kind);
</script>

{#snippet chip(key: TokenKey, value: string, label: string, title: string)}
	<button type="button" class="chip" onclick={() => onfilter(key, value)} {title}>{label}</button>
{/snippet}

<div bind:this={scroller} class="sb-list-wrap scroller" onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
	<table class="sb-list runs">
		<colgroup>
			<col style:width="40px" />
			<col style:width="150px" />
			<col />
			<col style:width="230px" />
			<col style:width="280px" />
			<col style:width="150px" />
			<col style:width="90px" />
		</colgroup>
		<thead>
			<tr>
				<th class="sel">
					<input type="checkbox" class="sb-check" checked={list.allVisibleSelected} indeterminate={list.selected.size > 0 && !list.allVisibleSelected} onchange={() => list.toggleAll()} aria-label="Select all loaded runs" />
				</th>
				<th>Run</th>
				<th>What</th>
				<th>Started by</th>
				<th>Status</th>
				<th>Started</th>
				<th class="num">Took</th>
			</tr>
		</thead>
		<tbody>
			{#if first > 0}<tr class="spacer" style:height="{first * ROW_H}px"><td colspan="7"></td></tr>{/if}
			{#each visible as d (d.key)}
				{@const r = d.row.run}
				{@const line = statusLine(d.row, app.now)}
				<tr class:child={d.child} class:checked={list.selected.has(r.id)} style:height="{ROW_H}px" onclick={(e) => open(e, r.id)} onauxclick={(e) => e.button === 1 && open(e, r.id)}>
					<td class="sel">
						<i class="stripe" style:background={runStatusTone[r.status]}></i>
						<input type="checkbox" class="sb-check" checked={list.selected.has(r.id)} onchange={() => list.toggleSelected(r.id)} aria-label="Select run {shortRunId(r.id)}" />
					</td>
					<td>
						<div class="run-id">
							<a class="mono" href="/runs/{r.id}" title={r.id}>{shortRunId(r.id)}</a>
							<CopyButton value={r.id} label="Copy run ID" />
							{#if r.childRunCount && !d.child && !list.params.children}
								<button type="button" class="expand" onclick={() => list.toggleExpanded(r.id)} aria-expanded={list.expanded.has(r.id)} title="{list.expanded.has(r.id) ? 'Hide' : 'Show'} the {r.childRunCount} job runs in this group">
									{list.expanded.has(r.id) ? '▾' : '▸'} {r.childRunCount}
								</button>
							{/if}
						</div>
						{#if d.child || (list.params.children && r.parentRunId)}
							<div class="cell-desc">in group <a href="/runs/{r.parentRunId}">{shortRunId(r.parentRunId ?? '')}</a></div>
						{/if}
					</td>
					<td class="what">
						<div class="line">
							{#if r.jobKey}
								{@render chip('job', r.jobKey, r.jobKey, `Show only runs of ${r.jobKey}`)}
							{:else}
								<span class="adhoc" title={d.row.selection.join(' ')}>ad hoc: <code>{d.row.selection.join(' ') || 'whole project'}</code></span>
							{/if}
							{@render chip('kind', kindToken(d.row), runKindLabel[r.kind].toLowerCase(), `Show only ${runKindLabel[r.kind].toLowerCase()} runs`)}
						</div>
					</td>
					<td class="by">
						<div class="line">
							{#if r.origin === 'schedule' && r.triggerName}
								<CalendarClockIcon size={13} />
								{@render chip('schedule', r.triggerName, r.triggerName, `Show only runs started by the ${r.triggerName} schedule`)}
								<a class="go" href="/triggers/{r.triggerId}" title="Open the schedule">↗</a>
							{:else if r.origin === 'sensor' && r.triggerName}
								<RadarIcon size={13} />
								{@render chip('sensor', r.triggerName, r.triggerName, `Show only runs started by the ${r.triggerName} sensor`)}
								<a class="go" href="/triggers/{r.triggerId}" title="Open the sensor">↗</a>
							{:else}
								<UserIcon size={13} />
								{@render chip('by', r.requestedBy.name, r.requestedBy.displayName, `Show only runs started by ${r.requestedBy.displayName} (${r.requestedBy.name})`)}
							{/if}
						</div>
						{#if r.rerunOfRunId}
							<div class="cell-desc">Rerun of <a href="/runs/{r.rerunOfRunId}">{shortRunId(r.rerunOfRunId)}</a></div>
						{:else if r.origin === 'schedule' && r.tickScheduledFor}
							<div class="cell-desc" title="Scheduled time of the tick that requested it">due {formatStamp(r.tickScheduledFor).slice(11)}</div>
						{:else if r.runKey}
							<div class="cell-desc" title="Sensor run key: one run per key">for <code>{r.runKey}</code></div>
						{/if}
					</td>
					<td>
						<RunStatusPill status={r.status} />
						<div class="status-line" style:color={toneColor[line.tone]} title={line.title ?? line.text}>{line.text}</div>
					</td>
					<td>
						{#if r.startedAt}
							<time class="muted" title={formatStamp(r.startedAt)}>{formatAgo(r.startedAt, app.now)}</time>
						{:else}
							<time class="muted" title="Queued {formatStamp(r.queuedAt)}">queued {formatAgo(r.queuedAt, app.now)}</time>
						{/if}
					</td>
					<td class="num">
						<span class:live={!r.endedAt && !!r.startedAt} class="muted took">{r.startedAt ? formatDuration(r.startedAt, r.endedAt, app.now) : '—'}</span>
					</td>
				</tr>
			{/each}
			{#if last < rows.length}<tr class="spacer" style:height="{(rows.length - last) * ROW_H}px"><td colspan="7"></td></tr>{/if}
			{#if !rows.length}
				<tr class="load-row">
					<td colspan="7" class="empty">
						{#if list.error}
							<span class="err">Couldn't load runs: {list.error}</span> <button class="retry" onclick={() => list.refresh()}>Retry</button>
						{:else if list.loading}
							Loading…
						{:else}
							{emptyText}
						{/if}
					</td>
				</tr>
			{:else if list.loadingMore || list.hasMore}
				<tr class="load-row"><td colspan="7" class="empty">Loading more…</td></tr>
			{:else if rows.length > 20}
				<tr class="load-row"><td colspan="7" class="empty">That's all {rows.length} runs.</td></tr>
			{/if}
		</tbody>
	</table>
</div>

<style>
	.scroller {
		min-height: 0;
		flex: 1;
		overflow: auto;
	}
	.runs {
		table-layout: fixed;
	}
	.runs thead th {
		position: sticky;
		top: 0;
		z-index: 2;
		background: var(--background);
	}
	.runs td {
		overflow: hidden;
		padding-top: 4px;
		padding-bottom: 4px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.spacer td,
	.spacer {
		padding: 0 !important;
		border: 0 !important;
	}
	tr.checked td {
		background: color-mix(in srgb, var(--primary) 7%, transparent);
	}
	.sel {
		position: relative;
		padding-right: 4px !important;
		padding-left: 14px !important;
		text-align: center;
	}
	.stripe {
		position: absolute;
		top: 6px;
		bottom: 6px;
		left: 0;
		width: 3px;
		border-radius: 2px;
	}
	.run-id {
		display: flex;
		align-items: center;
		gap: 4px;
	}
	.mono,
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.run-id a:hover,
	.cell-desc a:hover {
		color: var(--primary);
		text-decoration: underline;
	}
	.expand {
		margin-left: 2px;
		padding: 0 6px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--muted-foreground);
		font-size: 11.5px;
		line-height: 18px;
	}
	.expand:hover {
		color: var(--foreground);
	}
	tr.child td:nth-child(2) {
		padding-left: 28px;
	}
	tr.child {
		background: color-mix(in srgb, var(--sb-hover) 45%, transparent);
	}
	.line {
		display: flex;
		min-width: 0;
		align-items: center;
		gap: 6px;
		color: var(--muted-foreground);
	}
	.chip {
		overflow: hidden;
		max-width: 100%;
		padding: 0 7px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--foreground);
		font-size: 12px;
		line-height: 18px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.chip:hover {
		border-color: color-mix(in srgb, var(--primary) 50%, var(--border));
		background: var(--sb-hover);
	}
	.what .chip + .chip {
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.adhoc {
		overflow: hidden;
		color: var(--muted-foreground);
		text-overflow: ellipsis;
	}
	.go {
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.go:hover {
		color: var(--primary);
	}
	.status-line {
		overflow: hidden;
		margin-top: 2px;
		font-size: 12px;
		text-overflow: ellipsis;
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.took {
		font-variant-numeric: tabular-nums;
	}
	.took.live {
		color: var(--sb-running);
	}
	.empty {
		padding: 28px;
		color: var(--muted-foreground);
		text-align: center;
	}
	.err {
		color: var(--sb-error);
	}
	.retry {
		color: var(--primary);
	}
</style>
