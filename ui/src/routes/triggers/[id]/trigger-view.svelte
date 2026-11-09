<script lang="ts">
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import type { TriggerState } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo, formatIn, formatStamp } from '$lib/format/time';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import { shortRunId, stateEventLabel, triggerKindHelp, triggerKindLabel } from '$lib/operate/labels';
	import SensorPanel from './sensor-panel.svelte';
	import TickLog from './tick-log.svelte';

	let { id }: { id: string } = $props();

	const res = createResource((client, scope) => client.getTrigger(scope, id), ['triggers', 'ticks', 'runs']);
	const t = $derived(res.data ?? null);
	const isSchedule = $derived(t?.type === 'schedule');

	let reason = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);

	async function run(action: () => Promise<unknown>) {
		busy = true;
		error = null;
		try {
			await action();
			reason = '';
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = false;
		}
	}
	function setState(state: TriggerState) {
		const scope = app.scope;
		if (scope) void run(() => app.client.setTriggerStates(scope, [id], state, reason.trim() || null));
	}
	function resetDefault() {
		const scope = app.scope;
		if (scope) void run(() => app.client.resetTriggerToDefault(scope, id));
	}
</script>

<AppTopbar title="Triggers">
	{#if t}
		{#if app.can('trigger.toggle')}<input class="reason" bind:value={reason} placeholder="Why? (saved in the history)" aria-label="Reason for the change" />{/if}
		{#if t.state !== t.defaultState}
			<GatedButton requires="trigger.toggle" {busy} onclick={resetDefault}>Set back to the code ({t.defaultState === 'running' ? 'on' : 'paused'})</GatedButton>
		{/if}
		{#if t.state === 'running'}
			<GatedButton requires="trigger.toggle" {busy} onclick={() => setState('paused')}>Pause</GatedButton>
		{:else}
			<GatedButton requires="trigger.toggle" variant="default" {busy} onclick={() => setState('running')}>Turn on</GatedButton>
		{/if}
	{/if}
</AppTopbar>

<div class="flex min-h-0 flex-1 flex-col">
	<div class="head">
		<a href="/triggers" class="back">← Schedules and sensors</a>
		<span class="font-display text-[17px] font-semibold">{t?.name ?? id}</span>
		{#if t}
			<span class="sb-status" style:--tone={t.state === 'running' ? 'var(--sb-success)' : 'var(--sb-text-faint)'}><span></span>{t.state === 'running' ? 'on' : 'paused'}</span>
			<span class="faint">{t.state === t.defaultState ? 'as in the code' : `switched here; the code says ${t.defaultState === 'running' ? 'on' : 'paused'}`}</span>
			<span class="faint" title={triggerKindHelp[t.kind]}>· {triggerKindLabel[t.kind]} · {t.cadence}</span>
		{/if}
	</div>
	{#if res.error}<div class="msg err">Couldn't load this: {res.error}</div>{:else if res.data === null}<div class="msg">There's no schedule or sensor <code>{id}</code> in the version deployed to {app.location?.name}.</div>{/if}
	{#if error}<div class="msg err">{error}</div>{/if}

	{#if t}
		{#if t.warning || t.calendar?.low}
			<div class="warning"><TriangleAlertIcon size={14} /> {t.warning}</div>
		{/if}
		<div class="flex min-h-0 flex-1">
			<div class="main sb-scroll">
				{#if isSchedule}
					<section>
						<h3 title="Each time it was due (a tick) and what happened">Times due · {t.ticks.length}</h3>
						<TickLog ticks={t.ticks} />
					</section>
				{:else}
					<SensorPanel trigger={t} />
				{/if}
				{#if t.declaration}
					<section>
						<h3>Code</h3>
						<CodePreview file="{t.declaration.sourceFile}:{t.declaration.line}" code={t.declaration.code} language="text" maxLines={20} />
					</section>
				{/if}
			</div>

			<aside class="rail sb-scroll">
				<h3>How it works</h3>
				<dl>
					<div><dt>Kind</dt><dd title={triggerKindHelp[t.kind]}>{triggerKindLabel[t.kind]}</dd></div>
					<div><dt>Runs</dt><dd>{#if t.jobKey}<a href="/jobs/{t.jobKey}">{t.jobKey}</a>{:else}its own code only{/if}</dd></div>
					{#if t.cron}<div><dt>Cron</dt><dd><code>{t.cron}</code></dd></div>{/if}
					{#if t.timezone}<div><dt>Time zone</dt><dd>{t.timezone}</dd></div>{/if}
					{#if t.catchUp}<div><dt>After downtime</dt><dd title="Catch-up policy: {t.catchUp.mode}">{t.catchUp.mode === 'latest' ? 'at most one run, for the latest missed time' : `runs for each missed time, up to ${t.catchUp.max}`}</dd></div>{/if}
					{#if t.declaration}<div><dt>Targets</dt><dd>{t.targets?.join(', ') ?? 'all targets'}</dd></div>{/if}
					<div><dt>In the code</dt><dd>{t.defaultState === 'running' ? 'on' : 'paused'}</dd></div>
					{#if t.sourceFile}<div><dt>Defined in</dt><dd><code>{t.sourceFile}</code></dd></div>{/if}
				</dl>

				{#if isSchedule && t.upcoming}
					<h3>Next runs</h3>
					{#if t.state === 'paused'}
						<p class="faint">Paused.</p>
					{:else if t.upcoming.length}
						{#each t.upcoming as due, i (due)}
							<p class:faint={i > 0}>{formatStamp(due)} <span class="faint">{formatIn(due, app.now)}</span></p>
						{/each}
					{:else}
						<p class="faint">No upcoming times. Check the calendar below.</p>
					{/if}
				{/if}

				{#if t.calendar}
					<h3 title="Calendar horizon">Calendar</h3>
					<dl>
						{#if t.calendar.horizon}<div><dt>Worked out ahead</dt><dd>{t.calendar.horizon}</dd></div>{/if}
						<div><dt>Known until</dt><dd class:warn={t.calendar.low}>{t.calendar.storedUntil ? formatStamp(t.calendar.storedUntil) : 'no times stored'}</dd></div>
						{#if t.calendar.lastRefreshedAt}<div><dt>Last updated</dt><dd>{formatAgo(t.calendar.lastRefreshedAt, app.now)}</dd></div>{/if}
						{#if t.calendar.nextRefreshAt}<div><dt>Next update</dt><dd title={formatStamp(t.calendar.nextRefreshAt)}>{Date.parse(t.calendar.nextRefreshAt) > app.now ? formatIn(t.calendar.nextRefreshAt, app.now) : `due ${formatAgo(t.calendar.nextRefreshAt, app.now)}`}</dd></div>{/if}
						{#if t.calendar.lowHorizonWarningBelow}<div><dt>Warns when under</dt><dd>{t.calendar.lowHorizonWarningBelow}</dd></div>{/if}
					</dl>
				{/if}

				<h3>Last run</h3>
				{#if t.lastRun}
					<p><i class="dot" style:background={runStatusTone[t.lastRun.status]}></i><a class="mono" href="/runs/{t.lastRun.id}" title={t.lastRun.id}>{shortRunId(t.lastRun.id)}</a> <span class="faint">{runStatusLabel[t.lastRun.status]}{t.lastRun.startedAt ? ` · ${formatAgo(t.lastRun.startedAt, app.now)}` : ''}</span></p>
				{:else}
					<p class="faint">No runs yet.</p>
				{/if}

				{#if t.stateHistory}
				<h3>On/off history</h3>
				{#if t.stateHistory.length}
					{#each t.stateHistory as e (e.id)}
						<div class="event">
							<div><b>{stateEventLabel[e.kind]}</b>{#if e.value} <code>{e.value}</code>{/if}</div>
							<div class="faint">{e.actor.displayName}{#if e.actor.name !== e.actor.displayName} ({e.actor.name}){/if} · <span title={formatStamp(e.at)}>{formatAgo(e.at, app.now)}</span></div>
							{#if e.reason}<div class="faint">“{e.reason}”</div>{/if}
						</div>
					{/each}
				{:else}
					<p class="faint">None.</p>
				{/if}
				{/if}
			</aside>
		</div>
	{/if}
</div>

<style>
	.head {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 24px;
		border-bottom: 1px solid var(--border);
		font-size: 12.5px;
	}
	.back {
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.reason {
		width: 200px;
		height: 32px;
		padding: 0 9px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		font-size: 12.5px;
	}
	.faint {
		color: var(--muted-foreground);
	}
	.msg {
		padding: 14px 24px;
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.err {
		color: var(--sb-error);
	}
	.warning {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 24px;
		border-bottom: 1px solid color-mix(in srgb, var(--sb-warning) 40%, transparent);
		background: color-mix(in srgb, var(--sb-warning) 7%, transparent);
		color: var(--sb-warning);
		font-size: 12.5px;
	}
	.main {
		min-width: 0;
		flex: 1;
		overflow: auto;
		padding: 18px 24px;
	}
	section {
		margin-bottom: 24px;
	}
	h3 {
		display: flex;
		align-items: baseline;
		gap: 8px;
		margin: 18px 0 8px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	section h3,
	.rail h3:first-child {
		margin-top: 0;
	}
	code,
	.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.rail {
		width: 330px;
		flex-shrink: 0;
		overflow: auto;
		padding: 18px 20px;
		border-left: 1px solid var(--border);
		font-size: 12.5px;
	}
	dl div {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		padding: 3px 0;
	}
	dt {
		color: var(--muted-foreground);
	}
	dd {
		text-align: right;
	}
	dd.warn {
		color: var(--sb-warning);
	}
	p {
		padding: 2px 0;
	}
	.dot {
		display: inline-block;
		width: 7px;
		height: 7px;
		margin-right: 6px;
		border-radius: 50%;
	}
	.event {
		padding: 6px 0;
		border-bottom: 1px solid var(--border-subtle);
	}
	a {
		color: var(--primary);
	}
	.sb-scroll {
		scrollbar-width: thin;
		scrollbar-color: var(--border-strong) transparent;
	}
</style>
