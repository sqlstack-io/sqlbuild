<script lang="ts">
	import WorkflowIcon from '@lucide/svelte/icons/workflow';
	import LayersIcon from '@lucide/svelte/icons/layers';
	import { goto } from '$app/navigation';
	import type { JobKind, Trigger } from '$lib/api';
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
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { isActiveRun, isProblemRun, runStatusLabel, runStatusTone } from '$lib/status';
	import { overlapHelp, overlapPlain } from '$lib/operate/labels';

	const jobs = createResource((client, scope) => client.listJobs(scope), ['jobs', 'runs']);
	const triggers = createResource((client, scope) => client.listTriggers(scope), ['triggers']);

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
	let lastRun = $state<'problems' | 'active' | 'never' | ''>('');
	const all = $derived(jobs.data ?? []);
	const filtered = $derived(
		all.filter((j) => {
			if (kind !== 'all' && j.kind !== kind) return false;
			if (lastRun === 'problems' && !(j.lastRun && isProblemRun(j.lastRun.status))) return false;
			if (lastRun === 'active' && !(j.lastRun && isActiveRun(j.lastRun.status))) return false;
			if (lastRun === 'never' && j.lastRun) return false;
			const q = query.trim().toLowerCase();
			return !q || `${j.key} ${j.description} ${j.select ?? ''} ${j.members.join(' ')}`.toLowerCase().includes(q);
		})
	);
	const scheduledCount = $derived(all.filter((j) => j.triggerIds.some((id) => triggerById.get(id)?.state === 'running')).length);
	const failingCount = $derived(all.filter((j) => j.lastRun && isProblemRun(j.lastRun.status)).length);
</script>

<AppTopbar title="Jobs">
	<Button variant="outline" size="sm" onclick={() => (jobs.refresh(), triggers.refresh())}>↻ Refresh</Button>
</AppTopbar>

<ListToolbar
	bind:query
	placeholder="Search jobs and groups"
	total="{filtered.length} of {all.length}"
	counts={[
		{ tone: 'var(--sb-success)', count: scheduledCount, label: 'start automatically' },
		{ tone: 'var(--sb-error)', count: failingCount, label: 'last run failed or partial' }
	]}
>
	{#snippet filters()}
		<SegmentedFilter
			bind:value={kind}
			options={[
				{ value: 'all', label: 'All', count: all.length },
				{ value: 'job', label: 'Jobs', count: all.filter((j) => j.kind === 'job').length },
				{ value: 'group', label: 'Groups', count: all.filter((j) => j.kind === 'group').length }
			]}
		/>
		<ChoiceFilter
			label="Last run"
			bind:value={lastRun}
			all="Any"
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

<div class="sb-list-wrap">
	<table class="sb-list">
		<thead>
			<tr>
				<th>Job</th>
				<th>Selection</th>
				<th class="num">Nodes</th>
				<th title="What happens when it's asked to run while a run is already waiting or running">If already running</th>
				<th>Started by</th>
				<th>Owner</th>
				<th>Last run</th>
			</tr>
		</thead>
		<tbody>
			{#each filtered as j (j.key)}
				{@const jobTriggers = j.triggerIds.map((id) => triggerById.get(id)).filter((t) => t !== undefined)}
				<tr onclick={() => goto(`/jobs/${j.key}`)}>
					<td>
						<div class="cell-name">
							{#if j.kind === 'group'}<LayersIcon size={15} strokeWidth={1.75} />{:else}<WorkflowIcon size={15} strokeWidth={1.75} />{/if}
							<div class="min-w-0">
								<a href="/jobs/{j.key}" onclick={(e) => e.stopPropagation()}>{j.key}</a>
								<div class="cell-desc">{j.description}</div>
							</div>
						</div>
					</td>
					<td>
						{#if j.kind === 'group'}
							<span class="muted">Group · {j.members.join(', ')}</span>
						{:else}
							<code>{j.select}</code>
						{/if}
					</td>
					<td class="num muted">{j.nodeCount}</td>
					<td class="muted nowrap" title="{overlapHelp[j.overlap]} (overlap = {j.overlap})">{overlapPlain[j.overlap]}{#if j.priority}<span title="Higher priority runs go first when they wait for the same slot"> · priority {j.priority}</span>{/if}</td>
					<td>
						{#if jobTriggers.length === 0}
							<span class="muted" title="No schedule or sensor starts it; start it from its page">Only by hand</span>
						{:else}
							<div class="flex flex-col gap-1">
								{#each jobTriggers as t (t.id)}
									<div class="flex items-center gap-2.5">
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
										<a class:muted={t.state === 'paused'} href="/triggers/{t.id}" title={t.cadence} onclick={(e) => e.stopPropagation()}>{t.name}</a>
									</div>
								{/each}
							</div>
						{/if}
					</td>
					<td class="muted">{j.owner ?? '—'}</td>
					<td>
						{#if j.lastRun}
							<a class="last-run" href="/runs/{j.lastRun.id}" onclick={(e) => e.stopPropagation()}>
								<StatusLabel tone={runStatusTone[j.lastRun.status]} label={runStatusLabel[j.lastRun.status]} />
								{#if j.lastRun.startedAt}<RelativeTime value={formatAgo(j.lastRun.startedAt, app.now)} exact={formatStamp(j.lastRun.startedAt)} />{/if}
							</a>
						{:else}
							<span class="muted">Never run</span>
						{/if}
					</td>
				</tr>
			{:else}
				<LoadRow colspan={7} resource={jobs} empty={all.length ? 'No jobs match the filter.' : 'No jobs yet. Declare jobs and groups in the project code and deploy; they show up here.'} />
			{/each}
		</tbody>
	</table>
</div>

<style>
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
