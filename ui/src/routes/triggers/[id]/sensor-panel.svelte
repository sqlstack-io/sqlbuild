<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';
	import type { TriggerDetail } from '$lib/api';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import { app } from '$lib/state/app.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import { abandonReasonLabel, evaluationHelp, evaluationLabel, evaluationTone, shortRunId } from '$lib/operate/labels';
	import { plural } from '$lib/format/plural';

	let { trigger }: { trigger: TriggerDetail } = $props();
	const sensor = $derived(trigger.sensor!);

	let confirmReset = $state(false);
	let reason = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);

	async function resetCursor() {
		const scope = app.scope;
		if (!scope) return;
		busy = true;
		error = null;
		try {
			await app.client.resetSensorCursor(scope, trigger.id, reason.trim() || null);
			confirmReset = false;
			reason = '';
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = false;
		}
	}

	const changed = $derived(sensor.lastEvaluatedFingerprint !== null && sensor.lastEvaluatedFingerprint !== sensor.currentFingerprint);
	const evaluationColumns: TableColumn[] = [
		{ id: 'when', label: 'When', width: '120px' },
		{ id: 'event', label: 'Event', width: '30%', title: 'The event the function was called with' },
		{ id: 'result', label: 'Outcome', width: '130px' },
		{ id: 'detail', label: 'Detail', minWidth: 220, truncate: false }
	];
	const requestColumns: TableColumn[] = [
		{ id: 'key', label: 'Run key', minWidth: 200 },
		{ id: 'run', label: 'Run', width: '120px' },
		{ id: 'status', label: 'Status', width: '140px' },
		{ id: 'requested', label: 'Requested', width: '130px' }
	];
</script>

<section>
	<h3>Condition</h3>
	<p><code>{sensor.condition}</code></p>
	<p title="Each tick calls this function in the code location, once per event">Runs <code>{sensor.action}</code></p>
	{#each sensor.nodeConditions as c (c)}<p class="faint">and {c}</p>{/each}
	<p class="faint" title={[sensor.minimumIntervalSeconds ? `Checks at most every ${sensor.minimumIntervalSeconds}s` : 'Checks each time something relevant happens', sensor.timeoutSeconds ? `a check is stopped if it takes over ${sensor.timeoutSeconds}s` : ''].filter(Boolean).join('; ')}>
		{sensor.minimumIntervalSeconds ? `Min interval ${sensor.minimumIntervalSeconds}s` : 'On every event'}{sensor.timeoutSeconds ? ` · timeout ${sensor.timeoutSeconds}s` : ''}
	</p>
	{#if trigger.kind === 'platform_sensor'}
		<p class="faint"><a href={to(`/locations/${app.locationId}#problems`)}>Problems it handled ↗</a></p>
	{/if}
	{#if sensor.failing}
		<p class="warn" title="A failed tick leaves its event undelivered; it is retried after the minimum interval and later events wait">
			<!-- copy-allow: consequence of a failing sensor -->
			Stuck{#if sensor.failing.eventId} on <code>{sensor.failing.eventId}</code>{/if} since {formatAgo(sensor.failing.since, app.now)} · {plural(sensor.failing.failedTicks, 'failed tick')}{#if sensor.failing.reason} · {sensor.failing.reason}{/if}
		</p>
	{/if}
	{#if changed}
		<p class="warn" title="The next check uses the new code. Fingerprint {sensor.lastEvaluatedFingerprint} → {sensor.currentFingerprint}">Code changed since last check</p>
	{/if}
</section>

<section>
	<h3 title="The sensor's cursor">Where it's up to</h3>
	<div class="cursor">
		{#if sensor.cursorKnown}
			<code>{sensor.cursor ?? '(the start: nothing handled yet)'}</code>
			{#if sensor.cursorUpdatedAt}<span class="faint updated" title={formatStamp(sensor.cursorUpdatedAt)}>updated {formatAgo(sensor.cursorUpdatedAt, app.now)}</span>{/if}
		{/if}
		<GatedButton requires="sensor.reset_cursor" size="sm" disabled={sensor.cursorKnown && sensor.cursor === null} reason="It's already at the start" onclick={() => (confirmReset = !confirmReset)}>Start over</GatedButton>
	</div>
	{#if confirmReset}
		<div class="confirm">
			<!-- copy-allow: consequence of a reset -->
			<p>Re-reads everything from the start. Runs with an existing run key aren't repeated.</p>
			<input bind:value={reason} placeholder="Why? (saved in the history)" />
			<div class="row">
				<GatedButton requires="sensor.reset_cursor" variant="default" size="sm" {busy} onclick={resetCursor}>Start over</GatedButton>
				<Button variant="ghost" size="sm" onclick={() => (confirmReset = false)}>Cancel</Button>
			</div>
		</div>
	{/if}
	{#if error}<p class="err">{error}</p>{/if}
</section>

{#if trigger.evaluations}
<section>
	<h3 title="Each tick is one call of the function, newest first">Recent ticks</h3>
	<DataTable class="static compact" embedded columns={evaluationColumns} rows={trigger.evaluations} key={(e) => e.id} empty="No ticks yet.">
		{#snippet row(e)}
				<tr>
					<td class="nowrap" title={formatStamp(e.requestedAt)}>{formatAgo(e.requestedAt, app.now)}</td>
					<td title={e.eventSummary ?? undefined}>{#if e.eventId}<code>{e.eventId}</code>{:else}<span class="faint" title="A polling sensor's tick has no event">Poll</span>{/if}</td>
					<td title="{evaluationHelp[e.outcome]} · {e.id}"><StatusLabel tone={evaluationTone[e.outcome]} label={evaluationLabel[e.outcome]} /></td>
					<td class:faint={!e.error}>
						{#if e.outcome === 'skipped'}{e.skipReason ?? ''}{/if}
						{#if e.runIds.length}→ {#each e.runIds as id, i (id)}{i ? ', ' : ''}<a class="mono" href={to(`/runs/${id}`)} title={id}>{shortRunId(id)}</a>{/each}{/if}
						{#if e.outcome === 'failed' || e.outcome === 'timed_out'}
							{#if e.error}<details class="trace"><summary class="err">{e.error.split('\n').at(-1)}</summary><pre>{e.error}</pre></details>{:else}<span class="err" title="The traceback needs view.evidence">{e.failureCode ?? e.outcome}</span>{/if}
						{/if}
						{#if e.abandonedBecause}<span class:err={e.abandonedBecause === 'unanswered'}>{abandonReasonLabel[e.abandonedBecause]}</span>{/if}
						{#if e.skippedFor.length}<span title="The job's overlap is skip, so matching requests that arrived while a run was active were dropped">{e.skippedFor.length} skipped · {#each [...new Set(e.skippedFor)] as id, i (id)}{i ? ', ' : ''}<a class="mono" href={to(`/runs/${id}`)} title={id}>{shortRunId(id)}</a>{/each}</span>{/if}
						{#if e.definitionFingerprint !== sensor.currentFingerprint}<span class="sb-tag ml-1.5" title="Checked with code fingerprint {e.definitionFingerprint}">older code</span>{/if}
					</td>
				</tr>
		{/snippet}
	</DataTable>
</section>
{/if}

{#if trigger.runRequests}
<section>
	<h3 title="Run requests by run key">Runs · {trigger.runRequests.length}</h3>
	{#if trigger.runRequests.length}
		<DataTable class="static compact" embedded columns={requestColumns} rows={trigger.runRequests} key={(r) => r.runId} empty="No runs requested yet.">
			{#snippet row(r)}
					<tr>
						<td class="mono">{r.runKey}</td>
						<td class="mono"><a href={to(`/runs/${r.runId}`)} title={r.runId}>{shortRunId(r.runId)}</a></td>
						<td><i class="dot" style:background={runStatusTone[r.runStatus]}></i>{runStatusLabel[r.runStatus]}</td>
						<td class="faint" title={formatStamp(r.requestedAt)}>{formatAgo(r.requestedAt, app.now)}</td>
					</tr>
			{/snippet}
		</DataTable>
	{:else}
		<p class="faint">No runs yet.</p>
	{/if}
</section>
{/if}

<style>
	/* Room for the longest relative time, so the button beside it stays put as the time ticks. */
	.updated {
		min-width: 15ch;
	}
	section {
		margin-bottom: 24px;
	}
	h3 {
		margin-bottom: 8px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	p {
		margin-bottom: 4px;
		font-size: 12.5px;
	}
	code,
	.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.faint {
		color: var(--muted-foreground);
	}
	.warn {
		padding: 6px 10px;
		border: 1px solid color-mix(in srgb, var(--sb-warning) 40%, transparent);
		border-radius: 2px;
		background: color-mix(in srgb, var(--sb-warning) 6%, transparent);
	}
	.err {
		color: var(--sb-error);
	}
	.cursor {
		display: flex;
		align-items: center;
		gap: 12px;
		font-size: 12.5px;
	}
	.confirm {
		max-width: 560px;
		margin-top: 8px;
		padding: 10px 12px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--secondary);
	}
	.confirm input {
		width: 100%;
		margin: 6px 0;
		padding: 4px 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		font-size: 12.5px;
	}
	.row {
		display: flex;
		gap: 8px;
	}
	.trace summary {
		cursor: pointer;
	}
	.trace pre {
		max-height: 320px;
		margin-top: 4px;
		padding: 6px 8px;
		overflow: auto;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--secondary);
		font-family: var(--font-mono);
		font-size: 11px;
		white-space: pre-wrap;
	}
	.nowrap {
		white-space: nowrap;
	}
	.dot {
		display: inline-block;
		width: 7px;
		height: 7px;
		margin-right: 6px;
		border-radius: 50%;
	}
	a {
		color: var(--primary);
	}
</style>
