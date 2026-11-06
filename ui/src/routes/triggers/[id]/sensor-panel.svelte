<script lang="ts">
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
	let shown = $state(25);

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
</script>

<section>
	<h3>When it starts a run</h3>
	<p><code>{sensor.condition}</code></p>
	{#each sensor.nodeConditions as c (c)}<p class="faint">and {c}</p>{/each}
	<p class="faint">
		{sensor.minimumIntervalSeconds ? `Checks at most every ${sensor.minimumIntervalSeconds}s` : 'Checks each time something relevant happens'}{sensor.timeoutSeconds ? ` · a check is stopped if it takes over ${sensor.timeoutSeconds}s` : ''}
	</p>
	{#if trigger.kind === 'platform_sensor'}
		<p class="faint">Reacts to problems such as failed deploys or late schedules. <a href="/locations/{app.locationId}#problems">See the problems and how it handled them on the location page</a>.</p>
	{/if}
	{#if changed}
		<p class="warn" title="Fingerprint {sensor.lastEvaluatedFingerprint} → {sensor.currentFingerprint}">Its code changed since it last checked. The next check uses the new code.</p>
	{/if}
</section>

<section>
	<h3 title="The sensor's cursor">Where it's up to</h3>
	<div class="cursor">
		<code>{sensor.cursor ?? '(the start: nothing handled yet)'}</code>
		{#if sensor.cursorUpdatedAt}<span class="faint" title={formatStamp(sensor.cursorUpdatedAt)}>updated {formatAgo(sensor.cursorUpdatedAt, app.now)}</span>{/if}
		<GatedButton requires="sensor.reset_cursor" size="xs" disabled={sensor.cursor === null} reason="It's already at the start" onclick={() => (confirmReset = !confirmReset)}>Start over</GatedButton>
	</div>
	{#if confirmReset}
		<div class="confirm">
			<p>The next check looks at everything again from the start. Anything it already started a run for (same run key) doesn't start a second run.</p>
			<input bind:value={reason} placeholder="Why? (saved in the history)" />
			<div class="row">
				<GatedButton requires="sensor.reset_cursor" variant="default" size="sm" {busy} onclick={resetCursor}>Start over</GatedButton>
				<button class="ghost" onclick={() => (confirmReset = false)}>Cancel</button>
			</div>
		</div>
	{/if}
	{#if error}<p class="err">{error}</p>{/if}
</section>

<section>
	<h3 title="Each time the sensor ran its check (an evaluation)">Checks · {trigger.evaluations.length}</h3>
	<table>
		<thead><tr><th>#</th><th>When</th><th>Result</th><th title="Cursor before → after">Position</th><th class="num">Runs started</th><th>Detail</th></tr></thead>
		<tbody>
			{#each trigger.evaluations.slice(0, shown) as e (e.id)}
				<tr>
					<td class="faint">{e.sequence}</td>
					<td class="nowrap" title={formatStamp(e.requestedAt)}>{formatAgo(e.requestedAt, app.now)}</td>
					<td><span class="outcome" style:--c={evaluationTone[e.outcome]} title="{e.outcome} · {e.id}">{outcomeText[e.outcome] ?? e.outcome}</span></td>
					<td class="mono faint">{e.cursorBefore ?? '∅'}{#if e.cursorAfter && e.cursorAfter !== e.cursorBefore} → {e.cursorAfter}{/if}</td>
					<td class="num">{e.runRequests}</td>
					<td class:err={!!e.error} class:faint={!e.error}>
						{e.error ?? e.skipReason ?? ''}
						{#if e.definitionFingerprint !== sensor.currentFingerprint}<span class="tag" title="Checked with code fingerprint {e.definitionFingerprint}">older code</span>{/if}
					</td>
				</tr>
			{:else}
				<tr><td colspan="6" class="empty">It hasn't checked yet.</td></tr>
			{/each}
		</tbody>
	</table>
	{#if trigger.evaluations.length > shown}<button class="more" onclick={() => (shown += 50)}>Show more</button>{/if}
</section>

<section>
	<h3 title="Run requests by run key">Runs it started · {trigger.runRequests.length}</h3>
	{#if trigger.runRequests.length}
		<table>
			<thead><tr><th>Run key</th><th>Run</th><th>Status</th><th>Requested</th></tr></thead>
			<tbody>
				{#each trigger.runRequests as r (r.runKey)}
					<tr>
						<td class="mono">{r.runKey}</td>
						<td class="mono"><a href="/runs/{r.runId}" title={r.runId}>{shortRunId(r.runId)}</a></td>
						<td><i class="dot" style:background={runStatusTone[r.runStatus]}></i>{runStatusLabel[r.runStatus]}</td>
						<td class="faint" title={formatStamp(r.requestedAt)}>{formatAgo(r.requestedAt, app.now)}</td>
					</tr>
				{/each}
			</tbody>
		</table>
		<p class="faint small">One run per run key: the same key never starts a second run.</p>
	{:else}
		<p class="faint">{trigger.jobKey ? 'It hasn’t started any runs yet.' : 'This sensor only runs its own code; it doesn’t start runs.'}</p>
	{/if}
</section>

<style>
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
	.small {
		margin-top: 6px;
		font-size: 11.5px;
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
	.ghost {
		color: var(--muted-foreground);
		font-size: 12.5px;
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
	.num {
		text-align: right;
	}
	.nowrap {
		white-space: nowrap;
	}
	.outcome {
		color: var(--c);
		font-weight: 500;
	}
	.tag {
		margin-left: 6px;
		padding: 0 5px;
		border-radius: 2px;
		background: var(--sb-hover);
		color: var(--sb-text-faint);
		font-size: 11px;
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
