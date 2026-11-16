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
	import { evaluationTone, shortRunId } from '$lib/operate/labels';

	const outcomeText: Record<string, string> = { requested: 'checking…', completed: 'done', failed: 'failed', timed_out: 'took too long', abandoned: 'gave up' };

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
		{ id: 'result', label: 'Result', width: '160px' },
		{ id: 'position', label: 'Position', width: '24%', title: 'Cursor before → after' },
		{ id: 'runs', label: 'Runs started', width: '110px', align: 'right' },
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
	<h3>When it starts a run</h3>
	<p><code>{sensor.condition}</code></p>
	{#each sensor.nodeConditions as c (c)}<p class="faint">and {c}</p>{/each}
	<p class="faint">
		{sensor.minimumIntervalSeconds ? `Checks at most every ${sensor.minimumIntervalSeconds}s` : 'Checks each time something relevant happens'}{sensor.timeoutSeconds ? ` · a check is stopped if it takes over ${sensor.timeoutSeconds}s` : ''}
	</p>
	{#if trigger.kind === 'platform_sensor'}
		<p class="faint"><a href={to(`/locations/${app.locationId}#problems`)}>Problems it handled ↗</a></p>
	{/if}
	{#if changed}
		<p class="warn" title="Fingerprint {sensor.lastEvaluatedFingerprint} → {sensor.currentFingerprint}">Its code changed since it last checked. The next check uses the new code.</p>
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
			<p>The next check looks at everything again from the start. Anything it already started a run for (same run key) doesn't start a second run.</p>
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
	<h3 title="Each time the sensor ran its check (an evaluation), newest first">Recent checks</h3>
	<DataTable class="static compact" embedded columns={evaluationColumns} rows={trigger.evaluations} key={(e) => e.id} empty="It hasn't checked yet.">
		{#snippet row(e)}
				<tr>
					<td class="nowrap" title={formatStamp(e.requestedAt)}>{formatAgo(e.requestedAt, app.now)}</td>
					<td title="{e.outcome} · {e.id}"><StatusLabel tone={evaluationTone[e.outcome]} label={outcomeText[e.outcome] ?? e.outcome} /></td>
					<td class="mono faint">{e.cursorBefore ?? '∅'}{#if e.cursorAfter && e.cursorAfter !== e.cursorBefore} → {e.cursorAfter}{/if}</td>
					<td class="num">{e.runRequests}</td>
					<td class:err={!!e.error} class:faint={!e.error}>
						{e.error ?? e.skipReason ?? ''}
						{#if e.skippedFor.length}<span title="The job's overlap is skip, so matching requests that arrived while a run was active were dropped">{e.skippedFor.length} skipped while {#each [...new Set(e.skippedFor)] as id, i (id)}{i ? ', ' : ''}<a class="mono" href={to(`/runs/${id}`)} title={id}>{shortRunId(id)}</a>{/each} ran</span>{/if}
						{#if e.definitionFingerprint !== sensor.currentFingerprint}<span class="sb-tag ml-1.5" title="Checked with code fingerprint {e.definitionFingerprint}">older code</span>{/if}
					</td>
				</tr>
		{/snippet}
	</DataTable>
</section>
{/if}

{#if trigger.runRequests}
<section>
	<h3 title="Run requests by run key">Runs it started · {trigger.runRequests.length}</h3>
	{#if trigger.runRequests.length}
		<DataTable class="static compact" embedded columns={requestColumns} rows={trigger.runRequests} key={(r) => r.runKey} empty="No runs requested yet.">
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
		<p class="faint">{trigger.jobKey ? 'It hasn’t started any runs yet.' : 'This sensor only runs its own code; it doesn’t start runs.'}</p>
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
	.num {
		text-align: right;
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
