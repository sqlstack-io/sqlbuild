<script lang="ts">
	import { to } from '$lib/paths';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import LoadRow from '$lib/components/list/load-row.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import PageTabs from '$lib/components/platform/page-tabs.svelte';
	import type { OldNameView } from '$lib/api';
	import { formatAgo, formatIn, formatStamp } from '$lib/format/time';
	import { formatRelation } from '$lib/platform/labels';
	import {
		decisionLabel,
		migrationKind,
		oldNameStatus,
		oldNameStatusLabel,
		oldNameStatusTone,
		oldNameStepLabel
	} from '$lib/platform/maintenance';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	const history = createResource((client, scope) => client.maintenanceHistory(scope), ['warehouse_sync']);

	type Tab = 'migrations' | 'views' | 'janitor';
	let tab = $state<Tab>('migrations');
	let query = $state('');

	const migrations = $derived(history.data?.migrations ?? []);
	const views = $derived(history.data?.oldNameViews ?? []);
	const janitor = $derived(history.data?.janitorEvents ?? []);

	const q = $derived(query.trim().toLowerCase());
	const shownMigrations = $derived(
		migrations.filter((m) => !q || `${m.originModel ?? ''} ${m.destinationModel} ${formatRelation(m.origin)} ${formatRelation(m.destination)}`.toLowerCase().includes(q))
	);
	const shownViews = $derived(
		views.filter((v) => !q || `${v.destinationModel} ${formatRelation(v.old)} ${formatRelation(v.new)}`.toLowerCase().includes(q))
	);
	const shownJanitor = $derived(
		janitor.filter((j) => !q || `${j.originalQualifiedName ?? ''} ${j.archiveQualifiedName}`.toLowerCase().includes(q))
	);
	const liveViews = $derived(views.filter((v) => oldNameStatus(v, app.now) === 'live').length);
	const expiredViews = $derived(views.filter((v) => oldNameStatus(v, app.now) === 'expired').length);

	function viewDetail(v: OldNameView): string {
		const status = oldNameStatus(v, app.now);
		if (status === 'live') return v.expiresAt ? `Removed ${formatIn(v.expiresAt, app.now)}` : 'Kept until removed by hand';
		if (status === 'expired') return `Ended ${formatAgo(v.expiresAt, app.now)}; the next clean-up removes it`;
		if (status === 'dropped') return v.dropReason ?? 'Removed';
		if (status === 'not_kept') return 'Set not to keep the old name; clean-up removes the old table';
		return 'Waiting for the move to finish';
	}

	const total = $derived(tab === 'migrations' ? `${shownMigrations.length} of ${migrations.length} renames and moves` : tab === 'views' ? `${shownViews.length} of ${views.length} old names` : `${shownJanitor.length} of ${janitor.length} clean-ups`);
</script>

<AppTopbar title="Maintenance">
	<Button variant="outline" size="sm" onclick={() => history.refresh()}>↻ Refresh</Button>
</AppTopbar>

<PageTabs
	bind:value={tab}
	tabs={[
		{ value: 'migrations', label: 'Renames & moves', count: migrations.length },
		{ value: 'views', label: 'Old names kept working', count: views.length },
		{ value: 'janitor', label: 'Clean-up', count: janitor.length }
	]}
/>

<ListToolbar
	bind:query
	placeholder={tab === 'janitor' ? 'Search relations' : 'Search models and relations'}
	{total}
	counts={tab === 'views'
		? [
				{ tone: 'var(--sb-success)', count: liveViews, label: 'working' },
				{ tone: 'var(--sb-warning)', count: expiredViews, label: 'past their end date' }
			]
		: []}
>
</ListToolbar>

<div class="sb-list-wrap">
	{#if tab === 'migrations'}
		<table class="sb-list static">
			<thead>
				<tr>
					<th>When</th>
					<th>What</th>
					<th>From</th>
					<th>To</th>
					<th>What happened to the data</th>
					<th>Run</th>
				</tr>
			</thead>
			<tbody>
				{#each shownMigrations as m (m.eventId)}
					<tr>
						<td><RelativeTime value={formatAgo(m.createdAt, app.now)} exact={formatStamp(m.createdAt)} /></td>
						<td>
							<span class="sb-tag">{migrationKind(m) === 'move' ? 'Move' : 'Rename'}</span>
							<div class="cell-desc" title={m.discovery === 'automatic' ? undefined : 'Declared with migrate_from'}>{m.discovery === 'automatic' ? 'Spotted automatically' : 'Set in the code'}</div>
						</td>
						<td>
							<span class="model">{m.originModel ?? formatRelation(m.origin)}</span>
							<div class="cell-desc"><code>{formatRelation(m.origin)}</code></div>
						</td>
						<td>
							<a class="model link" href={to(`/catalog/${m.destinationModel}`)}>{m.destinationModel}</a>
							<div class="cell-desc"><code>{formatRelation(m.destination)}</code></div>
						</td>
						<td class="muted" title="decision = {m.decision}">{decisionLabel[m.decision]}</td>
						<td class="nowrap"><code class="muted" title="The run that did it: {m.runId}">{m.runId.replace(/^run-/, '')}</code></td>
					</tr>
				{:else}
					<LoadRow colspan={6} resource={history} empty={migrations.length ? 'Nothing matches the search.' : 'No models have been renamed or moved in this location.'} />
				{/each}
			</tbody>
		</table>
	{:else if tab === 'views'}
		<table class="sb-list static">
			<thead>
				<tr>
					<th>Old name</th>
					<th>Now points to</th>
					<th>Status</th>
					<th title="Old column names kept as aliases">Renamed columns</th>
					<th class="num" title="Access grants copied to the old name">Grants copied</th>
					<th>History</th>
				</tr>
			</thead>
			<tbody>
				{#each shownViews as v (v.migrationEventId)}
					{@const status = oldNameStatus(v, app.now)}
					<tr class:sb-row-off={status === 'dropped' || status === 'not_kept'}>
						<td title={v.archiveName ? `Old table archived as ${v.archiveName}` : undefined}>
							<code>{formatRelation(v.old)}</code>
							<div class="cell-desc" title="retention = {v.retention}">{v.retention === 'false' ? 'Not kept' : `Kept for ${v.retention}`}</div>
						</td>
						<td>
							<a class="model link" href={to(`/catalog/${v.destinationModel}`)}>{v.destinationModel}</a>
							<div class="cell-desc"><code>{formatRelation(v.new)}</code></div>
						</td>
						<td>
							<StatusLabel tone={oldNameStatusTone[status]} label={oldNameStatusLabel[status]} />
							<div class="cell-desc" title={v.expiresAt ? `Expires ${formatStamp(v.expiresAt)}` : undefined}>{viewDetail(v)}</div>
						</td>
						<td class="muted">
							{#each v.columnAliases as a (a.old)}
								<div class="alias"><code>{a.new} AS {a.old}</code></div>
							{:else}
								—
							{/each}
						</td>
						<td class="num muted">{v.grantsCopied ?? '—'}</td>
						<td>
							<ol class="steps">
								{#each v.steps as s (s.eventType)}
									<li title="{formatStamp(s.at)} · {s.runId}">{oldNameStepLabel[s.eventType]} <span>{formatAgo(s.at, app.now)}</span></li>
								{/each}
							</ol>
						</td>
					</tr>
				{:else}
					<LoadRow colspan={6} resource={history} empty={views.length ? 'Nothing matches the search.' : 'No old names are being kept working in this location.'} />
				{/each}
			</tbody>
		</table>
	{:else}
		<table class="sb-list static">
			<thead>
				<tr>
					<th>When</th>
					<th>What</th>
					<th>Original name</th>
					<th>Archived as</th>
					<th>Archived</th>
					<th>Run</th>
				</tr>
			</thead>
			<tbody>
				{#each shownJanitor as j (j.eventId)}
					<tr>
						<td><RelativeTime value={formatAgo(j.occurredAt, app.now)} exact={formatStamp(j.occurredAt)} /></td>
						<td>
							<StatusLabel tone={j.eventType === 'delete' ? 'var(--sb-error)' : 'var(--sb-warning)'} label={j.eventType === 'delete' ? 'Deleted' : 'Archived'} />
							<div class="cell-desc">{j.relationType ?? 'relation'} in {j.relationSchema}</div>
						</td>
						<td>{#if j.originalQualifiedName}<code>{j.originalQualifiedName}</code>{:else}<span class="muted" title="A temporary table SQLBuild used while building">Temporary build table</span>{/if}</td>
						<td><code class="muted">{j.archiveQualifiedName}</code></td>
						<td><RelativeTime value={formatAgo(j.archivedAt, app.now)} exact={formatStamp(j.archivedAt)} /></td>
						<td class="nowrap"><code class="muted" title={j.runId}>{j.runId.replace(/^run-/, '')}</code></td>
					</tr>
				{:else}
					<LoadRow colspan={6} resource={history} empty={janitor.length ? 'Nothing matches the search.' : 'Nothing has been archived or deleted by clean-up in this location.'} />
				{/each}
			</tbody>
		</table>
	{/if}
</div>

<style>
	:global(.sb-list.static tbody tr) {
		cursor: default;
	}
	.model {
		font-weight: 500;
	}
	.nowrap,
	.alias {
		white-space: nowrap;
	}
	.steps {
		display: flex;
		flex-direction: column;
		gap: 1px;
		white-space: nowrap;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.steps li span {
		color: var(--sb-text-faint);
	}
</style>
