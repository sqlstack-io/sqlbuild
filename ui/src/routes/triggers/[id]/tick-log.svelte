<script lang="ts">
	import { TICK_OUTCOMES, type Tick, type TickOutcome } from '$lib/api';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import { formatStamp } from '$lib/format/time';
	import { formatSeconds, shortRunId, tickOutcomeHelp, tickOutcomeText, tickOutcomeTone } from '$lib/operate/labels';

	let { ticks }: { ticks: Tick[] } = $props();

	const LATE_AFTER_SECONDS = 60;
	let outcome = $state<TickOutcome | 'late' | ''>('');
	let shown = $state(60);
	const filtered = $derived(
		ticks.filter((t) => (outcome === 'late' ? t.lateBySeconds > LATE_AFTER_SECONDS || t.catchUp : !outcome || t.outcome === outcome))
	);
</script>

<div class="bar">
	<ChoiceFilter
		label="Outcome"
		bind:value={outcome}
		options={[
			...TICK_OUTCOMES.map((o) => ({ value: o, label: tickOutcomeText[o], count: ticks.filter((t) => t.outcome === o).length })),
			{ value: 'late' as const, label: 'Late, or made up after downtime', count: ticks.filter((t) => t.lateBySeconds > LATE_AFTER_SECONDS || t.catchUp).length }
		]}
	/>
	<span class="faint" title="One entry (tick) per due time">{filtered.length} times due</span>
</div>
<table>
	<thead>
		<tr><th>Due</th><th title="When the scheduler handled it">Handled</th><th>What happened</th><th>Run</th><th>Late by</th><th>Why</th></tr>
	</thead>
	<tbody>
		{#each filtered.slice(0, shown) as t (t.id)}
			<tr>
				<td class="nowrap">{formatStamp(t.scheduledFor)}</td>
				<td class="nowrap faint">{t.evaluatedAt ? formatStamp(t.evaluatedAt) : 'not yet'}</td>
				<td title={tickOutcomeHelp[t.outcome]}><span class="outcome" style:--c={tickOutcomeTone[t.outcome]}>{tickOutcomeText[t.outcome]}</span>{#if t.catchUp}<span class="tag" title="Made up after the scheduler was down (catch-up)">made up</span>{/if}</td>
				<td class="mono">
					{#if t.runId}<a href="/runs/{t.runId}" title={t.runId}>{shortRunId(t.runId)}</a>
					{:else if t.coveredByRunId}<span class="sans">Merged into run <a href="/runs/{t.coveredByRunId}" title={t.coveredByRunId}>{shortRunId(t.coveredByRunId)}</a> (already waiting)</span>
					{:else}<span class="faint">—</span>{/if}
				</td>
				<td class="faint nowrap">{t.lateBySeconds > LATE_AFTER_SECONDS ? formatSeconds(t.lateBySeconds) : '—'}</td>
				<td class="faint" title={t.reason ?? undefined}>{t.outcome === 'covered' ? '' : (t.reason ?? '')}</td>
			</tr>
		{:else}
			<tr><td colspan="6" class="empty">{ticks.length ? 'Nothing matches the filter.' : 'It hasn’t been due yet.'}</td></tr>
		{/each}
	</tbody>
</table>
{#if filtered.length > shown}
	<button class="more" onclick={() => (shown += 100)}>Show {Math.min(100, filtered.length - shown)} more</button>
{/if}

<style>
	.sans {
		font-family: var(--font-sans);
	}
	.bar {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-bottom: 4px;
		font-size: 12px;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 12.5px;
	}
	th {
		padding: 6px 12px 6px 0;
		border-bottom: 1px solid var(--border);
		color: var(--sb-text-faint);
		font-size: 11.5px;
		font-weight: 500;
		text-align: left;
	}
	td {
		padding: 6px 12px 6px 0;
		border-bottom: 1px solid var(--border-subtle);
		vertical-align: top;
	}
	.outcome {
		color: var(--c);
		font-weight: 500;
	}
	.tag {
		margin-left: 6px;
		padding: 0 5px;
		border-radius: 2px;
		background: color-mix(in srgb, var(--sb-warning) 14%, transparent);
		color: var(--sb-warning);
		font-size: 11px;
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
	a {
		color: var(--primary);
	}
	.empty {
		padding: 24px;
		color: var(--sb-text-faint);
		text-align: center;
	}
	.more {
		margin-top: 8px;
		color: var(--primary);
		font-size: 12px;
	}
</style>
