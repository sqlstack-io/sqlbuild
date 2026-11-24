<script lang="ts">
	import { countBy } from '$lib/list/count-by';
	import UsersIcon from '@lucide/svelte/icons/users';
	import { openRow, to } from '$lib/paths';
	import WorkflowIcon from '@lucide/svelte/icons/workflow';
	import LayersIcon from '@lucide/svelte/icons/layers';
	import type { JobKind, Trigger } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { isActiveRun, isProblemRun, runStatusLabel, runStatusTone } from '$lib/status';
	import { jobPolicyText, overlapHelp } from '$lib/operate/labels';
	import FittingItems from '$lib/components/list/fitting-items.svelte';
	import { ownerChip, selectionTokens } from './utils';

	const jobs = createResource((client, scope) => client.listJobs(scope), ['jobs', 'runs', 'snapshot']);
	const triggers = createResource((client, scope) => client.listTriggers(scope), ['triggers', 'ticks']);

	const triggerById = $derived(new Map((triggers.data ?? []).map((t) => [t.id, t])));
	const canToggle = $derived(app.can('trigger.toggle'));
	let actionError = $state<string | null>(null);

	async function toggle(trigger: Trigger) {
		const scope = app.scope;
		if (!scope) return;
		actionError = null;
		try {
			await app.client.setTriggerStates(scope, [trigger.id], trigger.state === 'running' ? 'paused' : 'running');
		} catch (e) {
			actionError = e instanceof Error ? e.message : String(e);
		}
	}

	let query = $state('');
	let kind = $state<JobKind | 'all'>('all');
	type LastRun = 'problems' | 'active' | 'never';
	let lastRun = $state<LastRun[]>([]);
	const matchesLastRun = (j: (typeof all)[number], f: LastRun) =>
		f === 'problems' ? !!j.lastRun && isProblemRun(j.lastRun.status) : f === 'active' ? !!j.lastRun && isActiveRun(j.lastRun.status) : !j.lastRun;
	const all = $derived(jobs.data ?? []);
	const byKind = $derived(countBy(all, (j) => j.kind));
	const filtered = $derived(
		all.filter((j) => {
			if (kind !== 'all' && j.kind !== kind) return false;
			if (lastRun.length && !lastRun.some((f) => matchesLastRun(j, f))) return false;
			const q = query.trim().toLowerCase();
			return !q || `${j.key} ${j.description} ${j.select ?? ''} ${(j.members ?? []).join(' ')}`.toLowerCase().includes(q);
		})
	);
	const scheduledCount = $derived(all.filter((j) => j.triggerIds.some((id) => triggerById.get(id)?.state === 'running')).length);
	// Columns the server doesn't report stay hidden rather than showing dashes.
	const showPolicy = $derived(all.some((j) => jobPolicyText(j) !== ''));
	const showOwner = $derived(all.some((j) => j.owner !== null));
	const showSelection = $derived(all.some((j) => j.select !== null || j.members !== null));
	const jobColumns = $derived<TableColumn[]>([
		{ id: 'job', label: 'Job', minWidth: 240, truncate: false },
		...(showSelection ? [{ id: 'selection', label: 'Selection', width: '26%', truncate: false }] : []),
		{ id: 'nodes', label: 'Nodes', width: '80px', align: 'right' as const },
		...(showPolicy ? [{ id: 'policy', label: 'Policy', width: '170px', title: 'Whether scheduled requests skip while a run is active, and whether failed runs retry' }] : []),
		{ id: 'starts', label: 'Started by', width: '220px', truncate: false },
		...(showOwner ? [{ id: 'owner', label: 'Owner', width: '170px' }] : []),
		{ id: 'last', label: 'Last run', width: '170px' }
	]);
	const failingCount = $derived(all.filter((j) => j.lastRun && isProblemRun(j.lastRun.status)).length);
</script>

<AppTopbar title="Jobs">
	<RefreshButton onclick={() => (jobs.refresh(), triggers.refresh())} />
</AppTopbar>

<ListToolbar
	loading={jobs.data === undefined}
	bind:query
	placeholder="Search jobs and groups"
	total="{filtered.length} of {all.length}"
	counts={[
		{ tone: 'var(--sb-success)', count: scheduledCount, label: 'start automatically' },
		{ tone: 'var(--sb-error)', count: failingCount, label: 'failing', title: 'Last run failed or partial' }
	]}
>
	{#snippet filters()}
		<SegmentedFilter loading={jobs.data === undefined}
			bind:value={kind}
			options={[
				{ value: 'all', label: 'All', count: all.length },
				{ value: 'job', label: 'Jobs', count: byKind.get('job') ?? 0 },
				{ value: 'group', label: 'Groups', count: byKind.get('group') ?? 0 }
			]}
		/>
		<ChoiceFilter
			multiple
			label="Last run"
			bind:values={lastRun}
			options={[
				{ value: 'problems', label: 'Failed or partial' },
				{ value: 'active', label: 'Active' },
				{ value: 'never', label: 'Never run' }
			]}
		/>
	{/snippet}
	{#snippet actions()}
		{#if actionError}<span class="error">{actionError}</span>{/if}
	{/snippet}
</ListToolbar>

<DataTable columns={jobColumns} rows={filtered} key={(j) => j.key} resource={jobs} empty={all.length ? 'No jobs match the filter.' : 'No jobs yet.'}>
	{#snippet row(j)}
				{@const jobTriggers = j.triggerIds.map((id) => triggerById.get(id)).filter((t) => t !== undefined)}
				<tr onclick={(e) => openRow(e, `/jobs/${j.key}`)}>
					<td>
						<div class="cell-name">
							{#if j.kind === 'group'}<LayersIcon size={15} strokeWidth={1.75} />{:else}<WorkflowIcon size={15} strokeWidth={1.75} />{/if}
							<div class="job-name min-w-0">
								<a class="one-line" href={to(`/jobs/${j.key}`)} title={j.key}>{j.key}</a>
								<div class="cell-desc" title={j.description}>{j.description}</div>
							</div>
						</div>
					</td>
					{#if showSelection}
						<td>
							{#if j.kind === 'group'}
								<FittingItems items={j.members ?? []} label="members">
									{#snippet item(member)}<span class="sb-tag pill" title={member}>{member}</span>{/snippet}
								</FittingItems>
							{:else}
								<FittingItems items={selectionTokens(j.select)} label="selectors">
									{#snippet item(tok)}<span class="sb-tag pill" title={tok.token}>{#if tok.prefix}<span class="pfx">{tok.prefix}</span>{/if}{tok.name}</span>{/snippet}
								</FittingItems>
							{/if}
						</td>
					{/if}
					<td class="num muted nowrap">{j.nodeCount}</td>
					{#if showPolicy}<td class="muted nowrap" title={[j.overlap ? `overlap = ${j.overlap}: ${overlapHelp[j.overlap]}` : '', j.onFailure ? `[retry] attempts = ${j.onFailure.retries + 1}, delay_seconds = ${j.onFailure.retryDelaySeconds}` : '', j.priority ? 'Higher priority runs go first when they wait for the same slot' : ''].filter(Boolean).join('\n') || undefined}>{jobPolicyText(j) || '—'}</td>{/if}
					<td>
						{#if jobTriggers.length === 0}
							<span class="muted nowrap" title="No schedule or sensor starts it; start it from its page">Manual</span>
						{:else}
							<FittingItems items={jobTriggers} label="schedules and sensors">
								{#snippet item(t)}
									<span class="trigger">
										<span title={canToggle ? (t.state === 'running' ? `Pause ${t.name}: it stops starting runs` : `Turn ${t.name} on`) : "You don't have permission to pause or turn on schedules and sensors (trigger.toggle)"}>
											<button
												class="sb-toggle"
												class:on={t.state === 'running'}
												disabled={!canToggle}
												onclick={(e) => {
													e.stopPropagation();
													void toggle(t);
												}}
												aria-label="{t.state === 'running' ? 'Pause' : 'Turn on'} {t.name}"
											></button>
										</span>
										<a class:muted={t.state === 'paused'} class="one-line" href={to(`/triggers/${t.id}`)} title="{t.name} · {t.cadence}">{t.name}</a>
									</span>
								{/snippet}
							</FittingItems>
						{/if}
					</td>
					{#if showOwner}
						{@const owner = ownerChip(j.owner)}
						<td>
							{#if owner?.kind === 'team'}
								<span class="team" title="Team {owner.full}"><UsersIcon size={13} aria-hidden="true" />{owner.name}</span>
							{:else if owner}
								<span class="person" title={owner.full}><span class="initials" style:background={owner.colour} aria-hidden="true">{owner.initials}</span>{owner.name}</span>
							{:else}
								<span class="muted">—</span>
							{/if}
						</td>
					{/if}
					<td>
						{#if j.lastRun}
							<a class="last-run" href={to(`/runs/${j.lastRun.id}`)}>
								<StatusLabel tone={runStatusTone[j.lastRun.status]} label={runStatusLabel[j.lastRun.status]} />
								{#if j.lastRun.startedAt}<RelativeTime value={formatAgo(j.lastRun.startedAt, app.now)} exact={formatStamp(j.lastRun.startedAt)} />{/if}
							</a>
						{:else}
							<span class="muted nowrap">Never run</span>
						{/if}
					</td>
				</tr>
	{/snippet}
</DataTable>

<style>
	/* Descriptions don't wrap, so without a width limit one long description widens the whole column
	   and pushes the rest of the table off-screen. */
	.job-name {
		max-width: 420px;
	}
	.trigger {
		display: inline-flex;
		min-width: 0;
		align-items: center;
		gap: 10px;
		margin-right: 8px;
	}
	/* The shared tag, in mono, with the name standing out from its dimmed `tag:` prefix. */
	.pill {
		max-width: 100%;
		overflow: hidden;
		gap: 0;
		color: var(--foreground);
		font-family: var(--font-mono);
		text-overflow: ellipsis;
	}
	.pill .pfx {
		color: var(--sb-text-faint);
	}
	.team {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		color: var(--muted-foreground);
		white-space: nowrap;
	}
	.person {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		white-space: nowrap;
	}
	.initials {
		display: inline-grid;
		width: 20px;
		height: 20px;
		flex: 0 0 20px;
		place-items: center;
		border-radius: 50%;
		color: #0d0f12;
		font-size: 10px;
		font-weight: 600;
	}
	.one-line {
		display: block;
		overflow: hidden;
		max-width: 360px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.nowrap {
		white-space: nowrap;
	}
	.last-run {
		display: inline-flex;
		align-items: center;
		gap: 10px;
	}
	.error {
		margin-left: 8px;
		color: var(--sb-error);
		font-size: 12.5px;
	}
</style>
