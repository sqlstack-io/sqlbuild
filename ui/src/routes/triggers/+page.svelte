<script lang="ts">
	import ClockIcon from '@lucide/svelte/icons/clock';
	import CalendarIcon from '@lucide/svelte/icons/calendar-days';
	import RadioIcon from '@lucide/svelte/icons/radio';
	import CodeIcon from '@lucide/svelte/icons/code';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import { goto } from '$app/navigation';
	import type { Trigger, TriggerKind, TriggerState } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import LoadRow from '$lib/components/list/load-row.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo, formatIn, formatStamp } from '$lib/format/time';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import { shortRunId, tickOutcomeHelp, tickOutcomeText, tickOutcomeTone, triggerKindHelp, triggerKindLabel } from '$lib/operate/labels';

	const triggers = createResource((client, scope) => client.listTriggers(scope), ['triggers', 'ticks', 'runs']);
	const canToggle = $derived(app.can('trigger.toggle'));
	let actionError = $state<string | null>(null);
	let busy = $state(false);

	let query = $state('');
	let type = $state<'all' | 'schedule' | 'sensor'>('all');
	let kind = $state<TriggerKind | ''>('');
	let stateFilter = $state<TriggerState | 'overridden' | ''>('');
	const all = $derived(triggers.data ?? []);
	const filtered = $derived(
		all.filter((t) => {
			if (type !== 'all' && t.type !== type) return false;
			if (kind && t.kind !== kind) return false;
			if (stateFilter === 'overridden' ? t.state === t.defaultState : stateFilter && t.state !== stateFilter) return false;
			const q = query.trim().toLowerCase();
			return !q || `${t.name} ${t.jobKey ?? ''} ${t.cadence} ${t.description}`.toLowerCase().includes(q);
		})
	);

	let selected = $state<Set<string>>(new Set());
	let reason = $state('');
	function toggleSel(id: string) {
		const next = new Set(selected);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		selected = next;
	}
	const allSelected = $derived(filtered.length > 0 && filtered.every((t) => selected.has(t.id)));
	function toggleAll() {
		selected = allSelected ? new Set() : new Set(filtered.map((t) => t.id));
	}

	async function setStates(ids: string[], state: TriggerState, why: string | null = null) {
		const scope = app.scope;
		if (!scope || !ids.length) return;
		actionError = null;
		busy = true;
		try {
			await app.client.setTriggerStates(scope, ids, state, why);
		} catch (error) {
			actionError = error instanceof Error ? error.message : String(error);
		} finally {
			busy = false;
		}
	}
	async function bulk(state: TriggerState) {
		await setStates([...selected], state, reason.trim() || null);
		selected = new Set();
		reason = '';
	}
	async function resetDefault(t: Trigger) {
		const scope = app.scope;
		if (!scope) return;
		actionError = null;
		try {
			await app.client.resetTriggerToDefault(scope, t.id);
		} catch (error) {
			actionError = error instanceof Error ? error.message : String(error);
		}
	}

	const runningCount = $derived(all.filter((t) => t.state === 'running').length);
	const overriddenCount = $derived(all.filter((t) => t.state !== t.defaultState).length);

	function nextDue(t: Trigger): string {
		if (t.type === 'sensor') return t.state === 'paused' ? 'Paused' : 'When its condition is met';
		if (t.state === 'paused') return 'Paused';
		if (!t.nextDueAt) return '—';
		if (Date.parse(t.nextDueAt) < app.now) return `Late: was due ${formatAgo(t.nextDueAt, app.now)}`;
		return formatIn(t.nextDueAt, app.now);
	}
</script>

<AppTopbar title="Triggers">
	<Button variant="outline" size="sm" onclick={() => triggers.refresh()}>↻ Refresh</Button>
</AppTopbar>

<ListToolbar
	bind:query
	placeholder="Search schedules, sensors and jobs"
	total="{filtered.length} of {all.length} schedules and sensors"
	counts={[
		{ tone: 'var(--sb-success)', count: runningCount, label: 'on' },
		{ tone: 'var(--sb-text-faint)', count: all.length - runningCount, label: 'paused' },
		{ tone: 'var(--sb-warning)', count: overriddenCount, label: 'switched from what the code says' }
	]}
>
	{#snippet filters()}
		<SegmentedFilter
			bind:value={type}
			options={[
				{ value: 'all', label: 'All' },
				{ value: 'schedule', label: 'Schedules', count: all.filter((t) => t.type === 'schedule').length },
				{ value: 'sensor', label: 'Sensors', count: all.filter((t) => t.type === 'sensor').length }
			]}
		/>
		<ChoiceFilter label="Kind" bind:value={kind} options={(Object.keys(triggerKindLabel) as TriggerKind[]).map((k) => ({ value: k, label: triggerKindLabel[k], count: all.filter((t) => t.kind === k).length }))} />
		<ChoiceFilter
			label="Status"
			bind:value={stateFilter}
			options={[
				{ value: 'running', label: 'On' },
				{ value: 'paused', label: 'Paused' },
				{ value: 'overridden', label: 'Switched from what the code says' }
			]}
		/>
	{/snippet}
	{#snippet actions()}
		{#if selected.size}
			<span class="bulk">{selected.size} selected</span>
			<input class="reason" bind:value={reason} placeholder="Why? (saved in the history)" aria-label="Reason" />
			<button class="bulk-btn" disabled={!canToggle || busy} onclick={() => bulk('running')}>Turn on</button>
			<button class="bulk-btn" disabled={!canToggle || busy} onclick={() => bulk('paused')}>Pause</button>
		{/if}
		{#if actionError}<span class="bulk error">{actionError}</span>{/if}
	{/snippet}
</ListToolbar>

<div class="sb-list-wrap">
	<table class="sb-list">
		<thead>
			<tr>
				<th class="check-col" title={canToggle ? undefined : "You don't have permission to pause or turn on schedules and sensors (trigger.toggle)"}>
					<input type="checkbox" checked={allSelected} onchange={toggleAll} class="sb-check" aria-label="Select all" disabled={!canToggle} />
				</th>
				<th>Name</th>
				<th>Kind</th>
				<th>Runs</th>
				<th>Status</th>
				<th>Next run</th>
				<th title="What happened the last time it was due (its last tick)">Last time due</th>
				<th>Last run</th>
			</tr>
		</thead>
		<tbody>
			{#each filtered as t (t.id)}
				<tr class:sb-row-off={t.state === 'paused'} onclick={() => goto(`/triggers/${t.id}`)}>
					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<td class="check-col" onclick={(e) => e.stopPropagation()} title={canToggle ? undefined : "You don't have permission to pause or turn on schedules and sensors (trigger.toggle)"}>
						<input type="checkbox" checked={selected.has(t.id)} onchange={() => toggleSel(t.id)} class="sb-check" aria-label="Select {t.name}" disabled={!canToggle} />
					</td>
					<td>
						<div class="cell-name">
							<span title={canToggle ? (t.state === 'running' ? 'Pause: stop starting runs' : 'Turn on') : "You don't have permission to pause or turn on schedules and sensors (trigger.toggle)"}>
								<button
									class="sb-toggle"
									class:on={t.state === 'running'}
									disabled={!canToggle}
									onclick={(e) => {
										e.stopPropagation();
										void setStates([t.id], t.state === 'running' ? 'paused' : 'running');
									}}
									aria-label="{t.state === 'running' ? 'Pause' : 'Turn on'} {t.name}"
								></button>
							</span>
							<div class="min-w-0">
								<a href="/triggers/{t.id}" onclick={(e) => e.stopPropagation()}>{t.name}</a>
								<div class="cell-desc">{t.cadence}</div>
							</div>
							{#if t.warning}<span class="warn-icon" title={t.warning}><TriangleAlertIcon size={14} /></span>{/if}
						</div>
					</td>
					<td>
						<span class="type-cell" title={triggerKindHelp[t.kind]}>
							{#if t.kind === 'cron_schedule'}<ClockIcon size={14} />{:else if t.kind === 'calendar_schedule'}<CalendarIcon size={14} />{:else if t.kind === 'custom_sensor'}<CodeIcon size={14} />{:else}<RadioIcon size={14} />{/if}
							{triggerKindLabel[t.kind]}
						</span>
					</td>
					<td>
						{#if t.jobKey}<a class="link" href="/jobs/{t.jobKey}" onclick={(e) => e.stopPropagation()}>{t.jobKey}</a>{:else}<span class="muted" title="It runs its own code, e.g. to post a message, and starts no job">Runs its own code</span>{/if}
					</td>
					<td>
						<span class:muted={t.state === 'paused'}>{t.state === 'running' ? 'On' : 'Paused'}</span>
						{#if t.state !== t.defaultState}
							<div class="cell-desc override" title="Someone switched it here; the code says {t.defaultState}">
								code says {t.defaultState === 'running' ? 'on' : 'paused'} ·
								{#if canToggle}
									<button class="reset" onclick={(e) => (e.stopPropagation(), resetDefault(t))}>reset</button>
								{:else}
									<span title="You don't have permission to pause or turn on schedules and sensors (trigger.toggle)">reset</span>
								{/if}
							</div>
						{:else}
							<div class="cell-desc">as in the code</div>
						{/if}
					</td>
					<td class="muted" title={t.nextDueAt ? formatStamp(t.nextDueAt) : undefined}>{nextDue(t)}</td>
					<td class="muted" title={t.lastTick ? `${tickOutcomeHelp[t.lastTick.outcome]}${t.lastTick.reason ? `\n${t.lastTick.reason}` : ''}` : undefined}>
						{#if t.lastTick}
							<span class="tick" style:--c={tickOutcomeTone[t.lastTick.outcome]}>{tickOutcomeText[t.lastTick.outcome]}</span>
							{#if t.lastTick.coveredByRunId}into run <a class="link code" href="/runs/{t.lastTick.coveredByRunId}" onclick={(e) => e.stopPropagation()}>{shortRunId(t.lastTick.coveredByRunId)}</a>{/if}
							<div class="cell-desc">{formatAgo(t.lastTick.scheduledFor, app.now)}</div>
						{:else}
							{t.type === 'sensor' ? 'See its checks' : 'Not due yet'}
						{/if}
					</td>
					<td>
						{#if t.lastRun}
							<a href="/runs/{t.lastRun.id}" class="last-run" onclick={(e) => e.stopPropagation()}>
								<StatusLabel tone={runStatusTone[t.lastRun.status]} label={runStatusLabel[t.lastRun.status]} />
								{#if t.lastRun.startedAt}<RelativeTime value={formatAgo(t.lastRun.startedAt, app.now)} exact={formatStamp(t.lastRun.startedAt)} />{/if}
							</a>
						{:else}
							<span class="muted">—</span>
						{/if}
					</td>
				</tr>
			{:else}
				<LoadRow colspan={8} resource={triggers} empty={all.length ? 'Nothing matches the filter.' : 'No schedules or sensors yet. Declare them in the project code and deploy.'} />
			{/each}
		</tbody>
	</table>
</div>

<style>
	.check-col {
		width: 44px;
	}
	.type-cell {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		color: var(--muted-foreground);
		white-space: nowrap;
	}
	.last-run {
		display: inline-flex;
		align-items: center;
		gap: 10px;
	}
	.override {
		color: var(--sb-warning) !important;
	}
	.reset {
		color: var(--primary);
	}
	.reset:hover {
		text-decoration: underline;
	}
	.warn-icon {
		color: var(--sb-warning);
	}
	.tick {
		color: var(--c);
		font-weight: 500;
	}
	.bulk {
		margin-left: 8px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.bulk.error {
		color: var(--sb-error);
	}
	.reason {
		width: 180px;
		height: 28px;
		padding: 0 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		font-size: 12.5px;
	}
	.bulk-btn {
		height: 28px;
		padding: 0 10px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: transparent;
		font-size: 12.5px;
	}
	.bulk-btn:hover:not(:disabled) {
		background: var(--sb-hover);
	}
	.bulk-btn:disabled {
		opacity: 0.5;
	}
</style>
