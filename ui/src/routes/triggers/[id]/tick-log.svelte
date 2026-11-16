<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { to } from '$lib/paths';
	import { TICK_OUTCOMES, type Tick, type TickOutcome } from '$lib/api';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import { formatStamp } from '$lib/format/time';
	import { countBy } from '$lib/list/count-by';
	import { formatSeconds, shortRunId, tickOutcomeHelp, tickOutcomeText, tickOutcomeTone } from '$lib/operate/labels';

	let { ticks }: { ticks: Tick[] } = $props();

	const LATE_AFTER_SECONDS = 60;
	let outcome = $state<(TickOutcome | 'late')[]>([]);
	const isLate = (t: Tick) => t.lateBySeconds > LATE_AFTER_SECONDS || t.catchUp;
	/** Each tick under its outcome, and under "late" too when it was. */
	const tagsOf = (t: Tick): (TickOutcome | 'late')[] => (isLate(t) ? [t.outcome, 'late'] : [t.outcome]);
	const filtered = $derived(ticks.filter((t) => !outcome.length || tagsOf(t).some((o) => outcome.includes(o))));
	const byOutcome = $derived(countBy(ticks, tagsOf));
	// The server records no reason per tick; the column shows only where one exists.
	const showWhy = $derived(ticks.some((t) => t.reason !== null));
	const tickColumns = $derived<TableColumn[]>([
		{ id: 'due', label: 'Due', width: '170px' },
		{ id: 'handled', label: 'Handled', width: '170px', title: 'When the scheduler handled it' },
		{ id: 'what', label: 'Outcome', width: '200px' },
		{ id: 'run', label: 'Run', minWidth: 200 },
		{ id: 'late', label: 'Late by', width: '90px' },
		...(showWhy ? [{ id: 'why', label: 'Why', width: '24%' }] : [])
	]);
</script>

<div class="bar">
	<ChoiceFilter
		multiple
		label="Outcome"
		bind:values={outcome}
		options={[
			...TICK_OUTCOMES.map((o) => ({ value: o, label: tickOutcomeText[o], count: byOutcome.get(o) ?? 0 })),
			{ value: 'late' as const, label: 'Late', title: 'Late, or made up after downtime', count: byOutcome.get('late') ?? 0 }
		]}
	/>
	<span class="faint" title="One entry (tick) per due time">{filtered.length} ticks</span>
</div>
<DataTable class="static compact" embedded columns={tickColumns} rows={filtered} key={(t) => t.id} empty={ticks.length ? 'Nothing matches the filter.' : 'It hasn’t been due yet.'}>
	{#snippet row(t)}
			<tr>
				<td class="nowrap">{formatStamp(t.scheduledFor)}</td>
				<td class="nowrap faint">{t.evaluatedAt ? formatStamp(t.evaluatedAt) : 'not yet'}</td>
				<td title={tickOutcomeHelp[t.outcome]}><StatusLabel muted tone={tickOutcomeTone[t.outcome]} label={tickOutcomeText[t.outcome]} />{#if t.catchUp}<span class="sb-tag warn ml-1.5" title="Made up after the scheduler was down (catch-up)">made up</span>{/if}</td>
				<td class="mono">
					{#if t.runId}<a href={to(`/runs/${t.runId}`)} title={t.runId}>{shortRunId(t.runId)}</a>
					{:else if t.coveredByRunId}<span class="sans" title={tickOutcomeHelp.covered}>Merged · <a href={to(`/runs/${t.coveredByRunId}`)} title={t.coveredByRunId}>{shortRunId(t.coveredByRunId)}</a></span>
					{:else if t.activeRunId}<span class="sans" title="A run of this job was still active">Active · <a href={to(`/runs/${t.activeRunId}`)} title={t.activeRunId}>{shortRunId(t.activeRunId)}</a></span>
					{:else}<span class="faint">—</span>{/if}
				</td>
				<td class="faint nowrap">{t.lateBySeconds > LATE_AFTER_SECONDS ? formatSeconds(t.lateBySeconds) : '—'}</td>
				{#if showWhy}<td class="faint" title={t.reason ?? undefined}>{t.outcome === 'covered' ? '' : (t.reason ?? '')}</td>{/if}
			</tr>
	{/snippet}
</DataTable>

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
	/* Only the dot carries the outcome's colour; a column of the usual outcome stays calm. */
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
</style>
