<script lang="ts">
	import SearchIcon from '@lucide/svelte/icons/search';
	import type { RunEvent, RunEventLevel } from '$lib/api';
	import { formatStamp } from '$lib/format/time';
	import { clockTime, eventTone, shortRunId } from '$lib/operate/labels';
	import type { EventCategory } from './types';
	import { EVENT_CATEGORIES, eventCategory, eventSummary } from './utils';

	let { events, grouped }: { events: RunEvent[]; grouped: boolean } = $props();

	const LEVELS: { value: RunEventLevel; label: string }[] = [
		{ value: 'error', label: 'Errors' },
		{ value: 'warn', label: 'Warnings' },
		{ value: 'info', label: 'Info' },
		{ value: 'debug', label: 'Debug' }
	];
	let levels = $state<Set<RunEventLevel>>(new Set(['info', 'warn', 'error']));
	let categories = $state<Set<EventCategory>>(new Set(['run', 'nodes', 'queries', 'checks']));
	let query = $state('');

	const q = $derived(query.trim().toLowerCase());
	const byCategory = $derived(events.filter((e) => categories.has(eventCategory(e))));
	const byLevel = $derived(events.filter((e) => levels.has(e.level)));
	const shown = $derived(
		events.filter(
			(e) =>
				levels.has(e.level) &&
				categories.has(eventCategory(e)) &&
				(!q || `${e.nodeKey ?? ''} ${e.type} ${e.message} ${eventSummary(e)}`.toLowerCase().includes(q))
		)
	);
	const levelCount = (l: RunEventLevel) => byCategory.filter((e) => e.level === l).length;
	const categoryCount = (c: EventCategory) => byLevel.filter((e) => eventCategory(e) === c).length;

	function toggle<T>(set: Set<T>, value: T): Set<T> {
		const next = new Set(set);
		if (next.has(value)) next.delete(value);
		else next.add(value);
		return next;
	}
</script>

<div class="filters">
	<div class="chips" role="group" aria-label="Levels">
		{#each LEVELS as l (l.value)}
			<button class:on={levels.has(l.value)} onclick={() => (levels = toggle(levels, l.value))}>{l.label} <span>{levelCount(l.value)}</span></button>
		{/each}
	</div>
	<div class="chips" role="group" aria-label="Kinds">
		{#each EVENT_CATEGORIES as c (c.value)}
			<button class:on={categories.has(c.value)} onclick={() => (categories = toggle(categories, c.value))}>{c.label} <span>{categoryCount(c.value)}</span></button>
		{/each}
	</div>
	<label class="search">
		<SearchIcon size={13} />
		<input bind:value={query} placeholder="Search events" aria-label="Search events" />
	</label>
</div>

<table class="events">
	<colgroup>
		<col style:width="110px" />
		{#if grouped}<col style:width="100px" />{/if}
		<col style:width="220px" />
		<col />
	</colgroup>
	<thead>
		<tr>
			<th>Time</th>
			{#if grouped}<th>Job run</th>{/if}
			<th>Node</th>
			<th>What happened</th>
		</tr>
	</thead>
	<tbody>
		{#each shown as ev (ev.id)}
			<tr>
				<td class="faint nowrap" title={formatStamp(ev.occurredAt)}>{clockTime(ev.occurredAt)}</td>
				{#if grouped}<td class="mono faint"><a href="/runs/{ev.runId}">{shortRunId(ev.runId)}</a></td>{/if}
				<td class="mono">{ev.nodeKey ?? ''}</td>
				<td>
					<span class="dot" style:background={eventTone(ev)}></span>
					<span class:err={ev.level === 'error'} class:warn={ev.level === 'warn'} title="{ev.producer === 'scheduler' ? 'Scheduler' : 'sqb'} event {ev.type}{ev.message !== eventSummary(ev) ? `\n${ev.message}` : ''}">{eventSummary(ev)}</span>
				</td>
			</tr>
		{:else}
			<tr><td colspan={grouped ? 4 : 3} class="empty">{events.length ? 'No events match these filters.' : 'No events yet.'}</td></tr>
		{/each}
	</tbody>
</table>

<style>
	.filters {
		position: sticky;
		top: 0;
		z-index: 2;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 16px;
		border-bottom: 1px solid var(--border);
		background: var(--background);
	}
	.chips {
		display: flex;
		gap: 2px;
		padding: 2px;
		border-radius: 2px;
		background: var(--sb-hover);
	}
	.chips button {
		padding: 2px 8px;
		border-radius: 2px;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.chips button.on {
		background: var(--card);
		color: var(--foreground);
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
	}
	.chips span {
		color: var(--sb-text-faint);
	}
	.search {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-left: auto;
		padding: 3px 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--sb-text-faint);
	}
	.search input {
		width: 200px;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--foreground);
		font-size: 12px;
	}
	.events {
		width: 100%;
		border-collapse: collapse;
		font-size: 12px;
	}
	th {
		padding: 6px 16px;
		border-bottom: 1px solid var(--border);
		color: var(--sb-text-faint);
		font-size: 11.5px;
		font-weight: 500;
		text-align: left;
	}
	td {
		padding: 5px 16px;
		border-bottom: 1px solid var(--border-subtle);
		vertical-align: top;
	}
	tr:hover td {
		background: var(--sb-hover);
	}
	.dot {
		display: inline-block;
		width: 6px;
		height: 6px;
		margin-right: 8px;
		border-radius: 50%;
		vertical-align: middle;
	}
	.err {
		color: var(--sb-error);
	}
	.warn {
		color: var(--sb-warning);
	}
	.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.faint {
		color: var(--muted-foreground);
	}
	.nowrap {
		white-space: nowrap;
	}
	.empty {
		padding: 32px;
		color: var(--sb-text-faint);
		text-align: center;
	}
	a:hover {
		color: var(--primary);
	}
</style>
