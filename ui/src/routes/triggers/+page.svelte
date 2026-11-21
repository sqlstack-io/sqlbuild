<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Button from '$lib/components/ui/button/button.svelte';
	import { openRow, to } from '$lib/paths';
	import ClockIcon from '@lucide/svelte/icons/clock';
	import CalendarIcon from '@lucide/svelte/icons/calendar-days';
	import RadioIcon from '@lucide/svelte/icons/radio';
	import CodeIcon from '@lucide/svelte/icons/code';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import PinIcon from '@lucide/svelte/icons/pin';
	import type { Trigger, TriggerKind, TriggerState } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { app } from '$lib/state/app.svelte';
	import { countBy } from '$lib/list/count-by';
	import { matchesAny, toggleValue } from '$lib/list/choice';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo, formatIn, formatStamp } from '$lib/format/time';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import SetInUiPin from './set-in-ui-pin.svelte';
	import { setInUi, setInUiCount } from './utils';
	import { shortRunId, tickOutcomeHelp, tickOutcomeText, tickOutcomeTone, triggerKindHelp, triggerKindLabel } from '$lib/operate/labels';

	const triggers = createResource((client, scope) => client.listTriggers(scope), ['triggers', 'ticks', 'runs']);
	const canToggle = $derived(app.can('trigger.toggle'));
	let actionError = $state<string | null>(null);
	let busy = $state(false);

	let query = $state('');
	let type = $state<'all' | 'schedule' | 'sensor'>('all');
	let kind = $state<TriggerKind[]>([]);
	let stateFilter = $state<(TriggerState | 'set_in_ui')[]>([]);
	const all = $derived(triggers.data ?? []);
	const byType = $derived(countBy(all, (t) => t.type));
	const kindSet = $derived(new Set(kind));
	const byKind = $derived(countBy(all, (t) => t.kind));
	/** Any of the chosen states; "set in UI" matches a trigger whose state differs from its code. */
	const matchesState = (t: Trigger) => !stateFilter.length || stateFilter.some((f) => (f === 'set_in_ui' ? setInUi(t) : t.state === f));
	const filtered = $derived(
		all.filter((t) => {
			if (type !== 'all' && t.type !== type) return false;
			if (!matchesAny(kindSet, t.kind)) return false;
			if (!matchesState(t)) return false;
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
	const setInUiTotal = $derived(setInUiCount(all));

	function nextDue(t: Trigger): string {
		if (t.state === 'paused') return '—';
		if (t.type === 'sensor') return 'On condition';
		if (!t.nextDueAt) return '—';
		if (Date.parse(t.nextDueAt) < app.now) return `Late: was due ${formatAgo(t.nextDueAt, app.now)}`;
		return formatIn(t.nextDueAt, app.now);
	}
	const tableColumns = $derived<TableColumn[]>([
		{ id: 'sel', label: '', width: '44px', title: canToggle ? undefined : "You don't have permission to pause or turn on schedules and sensors (trigger.toggle)" },
		{ id: 'name', label: 'Name', minWidth: 220, truncate: false },
		{ id: 'kind', label: 'Kind', width: '150px' },
		{ id: 'runs', label: 'Runs', width: '18%' },
		{ id: 'status', label: 'Status', width: '130px' },
		{ id: 'next', label: 'Next run', width: '190px' },
		{ id: 'tick', label: 'Last tick', width: '170px', title: 'What happened the last time it was due (its last tick)' },
		{ id: 'last', label: 'Last run', width: '170px' }
	]);
</script>

<AppTopbar title="Triggers">
	<RefreshButton onclick={() => triggers.refresh()} />
</AppTopbar>

<ListToolbar
	loading={triggers.data === undefined}
	bind:query
	placeholder="Search schedules, sensors and jobs"
	total="{filtered.length} of {all.length} schedules and sensors"
	counts={[
		{ tone: 'var(--sb-success)', count: runningCount, label: 'on', reserve: true },
		{ tone: 'var(--sb-text-faint)', count: all.length - runningCount, label: 'paused', reserve: true },
		{
			count: setInUiTotal,
			label: 'set in UI',
			reserve: true,
			icon: PinIcon,
			pressed: stateFilter.includes('set_in_ui'),
			onclick: () => (stateFilter = toggleValue(stateFilter, 'set_in_ui'))
		}
	]}
>
	{#snippet filters()}
		<SegmentedFilter loading={triggers.data === undefined}
			bind:value={type}
			options={[
				{ value: 'all', label: 'All' },
				{ value: 'schedule', label: 'Schedules', count: byType.get('schedule') ?? 0 },
				{ value: 'sensor', label: 'Sensors', count: byType.get('sensor') ?? 0 }
			]}
		/>
		<ChoiceFilter multiple label="Kind" bind:values={kind} options={(Object.keys(triggerKindLabel) as TriggerKind[]).map((k) => ({ value: k, label: triggerKindLabel[k], count: byKind.get(k) ?? 0 }))} />
		<ChoiceFilter
			multiple
			label="Status"
			bind:values={stateFilter}
			options={[
				{ value: 'running', label: 'On' },
				{ value: 'paused', label: 'Paused' },
				{ value: 'set_in_ui', label: 'Set in UI' }
			]}
		/>
	{/snippet}
	{#snippet actions()}
		{#if selected.size}
			<span class="bulk">{selected.size} selected</span>
			<input class="reason" bind:value={reason} placeholder="Why? (saved in the history)" aria-label="Reason" />
			<Button disabled={!canToggle || busy} onclick={() => bulk('running')}>Turn on</Button>
			<Button disabled={!canToggle || busy} onclick={() => bulk('paused')}>Pause</Button>
		{/if}
		{#if actionError}<span class="bulk error">{actionError}</span>{/if}
	{/snippet}
</ListToolbar>

<DataTable columns={tableColumns} rows={filtered} key={(t) => t.id} resource={triggers} empty={all.length ? 'Nothing matches the filter.' : 'No schedules or sensors yet.'}>
	{#snippet head(c)}
		{#if c.id === 'sel'}<input type="checkbox" checked={allSelected} onchange={toggleAll} class="sb-check" aria-label="Select all" disabled={!canToggle} />{/if}
	{/snippet}
	{#snippet row(t)}
				<tr onclick={(e) => openRow(e, `/triggers/${t.id}`)}>
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
								<a href={to(`/triggers/${t.id}`)}>{t.name}</a>
								<div class="cell-desc">{t.cadence}</div>
							</div>
							{#if t.warning}<span class="warn-icon" title={t.warning}><TriangleAlertIcon size={14} /></span>{/if}
						</div>
					</td>
					<td>
						<span class="type-cell" title={triggerKindHelp[t.kind]}>
							{#if t.kind === 'cron_schedule'}<ClockIcon size={14} />{:else if t.kind === 'calendar_schedule'}<CalendarIcon size={14} />{:else if t.kind === 'polling_sensor'}<CodeIcon size={14} />{:else}<RadioIcon size={14} />{/if}
							{triggerKindLabel[t.kind]}
						</span>
					</td>
					<td>
						{#if t.jobKey}<a class="link" href={to(`/jobs/${t.jobKey}`)}>{t.jobKey}</a>{:else}<span class="muted" title="It runs its own code, e.g. to post a message, and starts no job">Own code</span>{/if}
					</td>
					<td class="status-col">
						<span class="status-cell">
							<StatusLabel tone={t.state === 'running' ? 'var(--sb-success)' : 'var(--sb-text-faint)'} label={t.state === 'running' ? 'On' : 'Paused'} />
							{#if setInUi(t)}<SetInUiPin trigger={t} canReset={canToggle} onreset={() => resetDefault(t)} />{/if}
						</span>
					</td>
					<td class="muted next-col" title={t.state === 'running' && t.nextDueAt ? formatStamp(t.nextDueAt) : undefined}>{nextDue(t)}</td>
					<td class="muted" title={t.lastTick ? `${tickOutcomeHelp[t.lastTick.outcome]}${t.lastTick.reason ? `\n${t.lastTick.reason}` : ''}` : undefined}>
						{#if t.lastTick}
							<StatusLabel muted tone={tickOutcomeTone[t.lastTick.outcome]} label={tickOutcomeText[t.lastTick.outcome]} />
							{#if t.lastTick.coveredByRunId}into run <a class="link code" href={to(`/runs/${t.lastTick.coveredByRunId}`)}>{shortRunId(t.lastTick.coveredByRunId)}</a>{/if}
							<div class="cell-desc">{formatAgo(t.lastTick.scheduledFor, app.now)}</div>
						{:else}
							{t.type === 'sensor' ? '—' : 'Not due yet'}
						{/if}
					</td>
					<td>
						{#if t.lastRun}
							<a href={to(`/runs/${t.lastRun.id}`)} class="last-run">
								<StatusLabel tone={runStatusTone[t.lastRun.status]} label={runStatusLabel[t.lastRun.status]} />
								{#if t.lastRun.startedAt}<RelativeTime value={formatAgo(t.lastRun.startedAt, app.now)} exact={formatStamp(t.lastRun.startedAt)} />{/if}
							</a>
						{:else}
							<span class="muted">—</span>
						{/if}
					</td>
				</tr>
	{/snippet}
</DataTable>

<style>
	/* Switching a trigger changes what these two cells say. Their widths fit every variant and they never
	   wrap, so a switch can't resize the columns or change the row's height and move the page. */
	.status-col {
		white-space: nowrap;
	}
	/* A fixed height, so the pin coming and going never changes the row's height. */
	.status-cell {
		display: inline-flex;
		height: 20px;
		align-items: center;
	}
	.next-col {
		white-space: nowrap;
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
	.warn-icon {
		color: var(--sb-warning);
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
</style>
