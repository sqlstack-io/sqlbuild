<script lang="ts">
	import { plural } from '$lib/format/plural';
	import PageBody from '$lib/components/page-body.svelte';
	import Loading from '$lib/components/loading.svelte';
	import { to } from '$lib/paths';
	import { page } from '$app/state';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import ConnectionTest from '$lib/components/platform/connection-test.svelte';
	import WarehouseSyncNow from '$lib/components/platform/warehouse-sync-now.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import LocationTokens from '$lib/components/platform/location-tokens.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import ProblemList from '$lib/components/platform/problem-list.svelte';
	import type { LocationInstance } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { brokenDeployPolicyHelp, brokenDeployPolicyLabel, healthHelp, healthLabel, healthTone, shortImage, targetWords } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { openRecord } from '../utils';
	import { firstLine, summariseProcesses } from './utils';

	const id = $derived(page.params.id ?? '');
	const location = createResource((client) => client.getLocation(id), ['snapshot', 'warehouse_sync']);
	// Without recorded history, the page lists the open problems the server derives now.
	const history = $derived(app.client.supports('listPlatformEventHistory'));
	const problems = createResource(
		(client) =>
			client.supports('listPlatformEventHistory')
				? client.listPlatformEventHistory({ locationId: id })
				: client.listPlatformEvents({ activeOnly: true }).then((events) => events.filter((e) => e.locationId === id).map(openRecord)),
		['platform_events']
	);
	const detail = $derived(location.data ?? null);
	let problemView = $state<'open' | 'all'>('open');
	const problemRows = $derived((problems.data ?? []).filter((e) => problemView === 'all' || e.active));
	const openProblems = $derived((problems.data ?? []).filter((e) => e.active).length);
	const processes = $derived(summariseProcesses(detail?.instances ?? []));
	const running = $derived(processes.live[0] ?? null);

	const instanceTone: Record<LocationInstance['state'], string> = {
		live: 'var(--sb-success)',
		stale: 'var(--sb-warning)',
		gone: 'var(--sb-text-faint)'
	};
	const instanceLabel: Record<LocationInstance['state'], string> = { live: 'Running', stale: 'Not checking in', gone: 'Stopped' };
</script>

<AppTopbar title="Locations" subject={detail ? `${detail.name} · Locations` : null}>
	<RefreshButton onclick={() => (location.refresh(), problems.refresh())} />
</AppTopbar>

<div class="flex min-h-0 flex-1 flex-col">
	{#if location.error}
		<div class="text-[var(--sb-error)] px-6 py-4 text-[13px]">Couldn't load this location: {location.error}</div>
	{:else if location.data === null}
		<div class="text-muted-foreground px-6 py-4 text-[13px]"><a href={to(`/locations`)} class="back">← Locations</a> No location with ID <code>{id}</code>.</div>
	{:else if !detail}
		<div class="px-6 py-4"><Loading /></div>
	{:else}
		<header class="head">
			<div class="line title-line">
				<a href={to(`/locations`)} class="back">← Locations</a>
				<span class="font-display text-[17px] font-semibold">{detail.name}</span>
				<span class="muted">· {targetWords(detail.target)}</span>
				<span class="right">
					<span title={healthHelp[detail.health]}><StatusLabel tone={healthTone[detail.health]} label={healthLabel[detail.health]} /></span>
					{#if openProblems}<a class="warn" href="#problems">· {plural(openProblems, 'open problem')}</a>{/if}
				</span>
			</div>

			<div class="line muted">
				{#if detail.snapshot}
					<span title="Snapshot {detail.snapshot.id}, generation {detail.snapshot.generation}, deployed {formatStamp(detail.snapshot.publishedAt)}">
						<b class="fg">Version {detail.snapshot.generation}</b> · commit <code>{detail.snapshot.gitRevision}</code>{#if detail.snapshot.gitDirty}{' '}<span class="warn">with uncommitted changes</span>{/if}
						· by {detail.snapshot.gitAuthor} · deployed {formatAgo(detail.snapshot.publishedAt, app.now)} · sqb {detail.snapshot.sqbVersion}
					</span>
				{:else}
					Nothing deployed yet
				{/if}
			</div>

			<div class="line">
				<span class="muted">
					{#if processes.live.length === 1 && running}
						<b class="fg">Running</b> ·
						{#if running.imageId}image <code title={running.imageId}>{shortImage(running.imageId)}</code> ·{:else if detail.imageId === null}<span title="Runs start as local processes, not containers">local process</span> ·{/if}
						<span title={formatStamp(running.lastSeenAt)}>last check-in {formatAgo(running.lastSeenAt, app.now)}</span>
					{:else if processes.live.length > 1}
						<b class="fg" title="Usually a deploy in progress; the new one takes over once it loads">{processes.live.length} processes running</b>
					{:else}
						<!-- copy-allow: consequence warning -->
						<b class="err">Not running</b> · no runs can start
					{/if}
				</span>
				<span class="right">
					<ConnectionTest compact scope={{ locationId: detail.id }} connection={detail.warehouseConnection ?? 'warehouse'} last={detail.connectionTest} />
					<WarehouseSyncNow scope={{ locationId: detail.id }} />
				</span>
			</div>

			{#if processes.listed.length}
				<ul class="processes">
					{#each processes.listed as i (i.id)}
						<li class:off={i.state === 'gone'}>
							<StatusLabel tone={instanceTone[i.state]} label={instanceLabel[i.state]} muted={i.state !== 'live'} />
							<code class="muted" title={i.id}>{i.id}</code>
							{#if i.imageId && i.gitRevision}<span class="muted">image <code title={i.imageId}>{shortImage(i.imageId)}</code> · commit <code>{i.gitRevision}</code></span>{/if}
							<span class="muted">{#if i.startedAt}started <RelativeTime value={formatAgo(i.startedAt, app.now)} exact={formatStamp(i.startedAt)} /> ·{/if} last check-in <RelativeTime value={formatAgo(i.lastSeenAt, app.now)} exact={formatStamp(i.lastSeenAt)} /></span>
						</li>
					{/each}
				</ul>
			{/if}

			{#if detail.failedDeploy}
				<details class="alert">
					<summary>
						<TriangleAlertIcon size={14} />
						<span class="summary-text">
							<b>Deploy failed {formatAgo(detail.failedDeploy.attemptedAt, app.now)}</b>
							· {detail.brokenDeployPolicy === 'fallback' ? `on version ${detail.snapshot ? detail.snapshot.generation : 'before'}` : 'runs held'}
							· <code>{firstLine(detail.failedDeploy.error)}</code>
						</span>
						<span class="more">details</span>
					</summary>
					<div class="alert-body">
						<p title="A deploy takes over only once it loads cleanly">Commit <code>{detail.failedDeploy.gitRevision}</code> (image <code>{shortImage(detail.failedDeploy.imageId)}</code>) didn't load.</p>
						<pre>{detail.failedDeploy.error}</pre>
					</div>
				</details>
			{:else if detail.lastLoadError}
				<details class="alert">
					<summary>
						<TriangleAlertIcon size={14} />
						<span class="summary-text"><b>The last load reported an error</b> · <code>{firstLine(detail.lastLoadError)}</code></span>
						<span class="more">details</span>
					</summary>
					<div class="alert-body"><pre>{detail.lastLoadError}</pre></div>
				</details>
			{:else if detail.health === 'falling_back'}
				<div class="alert note">
					<TriangleAlertIcon size={14} />
					<span class="summary-text">
						<b title="The location keeps the error: see its output, or run sqb compile in its project">Reload failed</b> · on version {detail.snapshot ? detail.snapshot.generation : 'before'}
					</span>
				</div>
			{/if}
			{#if detail.poolCapWarnings.length}
				<!-- The server's own warning, from its pool caps: it doesn't touch the location's health or last check-in. -->
				<div class="alert note pool-caps">
					<TriangleAlertIcon size={14} />
					<span class="summary-text">
						<b title="This server's pool_caps lower a pool's limit below what these jobs ask for; each takes the whole pool, so it runs alone in it">Pool caps on this server</b>
						{#each detail.poolCapWarnings as w (`${w.jobKey}:${w.pool}`)}
							<span class="cap" title={w.message}>
								· {w.kind} <a href={to(`/jobs/${w.jobKey}`)}>{w.jobKey}</a> takes {w.amount} of pool {w.pool}; capped at {w.cap}, so it runs alone in that pool
							</span>
						{/each}
					</span>
				</div>
			{/if}

			<details class="facts">
				<summary>Details</summary>
				<div class="facts-body">
					<KeyValues
						rows={[
							{ label: 'Location ID', value: detail.id, mono: true },
							detail.createdAt && { label: 'Created', value: formatAgo(detail.createdAt, app.now), title: formatStamp(detail.createdAt) },
							{ label: 'Target', value: [targetWords(detail.target), detail.target.connectionSummary].filter(Boolean).join(' · ') },
							detail.warehouseConnection && { label: 'Warehouse connection', value: detail.warehouseConnection, mono: true },
							detail.brokenDeployPolicy && {
								label: 'If a deploy fails',
								value: brokenDeployPolicyLabel[detail.brokenDeployPolicy === 'fallback' ? 'fallback' : 'hold'],
								help: brokenDeployPolicyHelp[detail.brokenDeployPolicy === 'fallback' ? 'fallback' : 'hold']
							}
						].filter((row) => !!row)}
					/>
				</div>
			</details>
		</header>

		<PageBody padding="24px">
			<div id="problems">
				<Panel
					title="Problems"
					help={history ? 'Overdue schedules, failed deploys, runs that couldn’t start, connection tests and warehouse syncs that failed.' : 'Late schedules, and the location no longer checking in.'}
				>
					{#snippet actions()}
						{#if history}<SegmentedFilter
							bind:value={problemView}
							options={[
								{ value: 'open', label: 'Open', count: openProblems },
								{ value: 'all', label: 'History', count: problems.data?.length ?? 0 }
							]}
						/>{/if}
					{/snippet}
					<ProblemList events={problemRows} resource={problems} currentLocationId={id} empty={problemView === 'open' ? 'No open problems.' : 'No problems recorded for this location.'} />
				</Panel>
			</div>

			{#if app.can('location.manage') && app.client.supports('listLocationTokens')}
				<Panel title="Access tokens" help="Lets this location's processes connect to the server. Can't do anything else.">
					<LocationTokens locationId={detail.id} />
				</Panel>
			{/if}
		</PageBody>
	{/if}
</div>

<style>
	.head {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 14px 24px 12px;
		border-bottom: 1px solid var(--border);
		font-size: 12.5px;
	}
	.line {
		display: flex;
		min-width: 0;
		align-items: center;
		gap: 8px;
	}
	.title-line {
		gap: 10px;
	}
	.right {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-left: auto;
	}
	.back {
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.back:hover {
		color: var(--foreground);
	}
	.fg {
		color: var(--foreground);
		font-weight: 500;
	}
	.processes {
		display: flex;
		flex-direction: column;
		margin: 2px 0;
		border-top: 1px solid var(--border);
	}
	.processes li {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 5px 2px;
		border-bottom: 1px solid var(--border);
	}
	.processes li.off {
		opacity: 0.7;
	}
	.alert {
		border: 1px solid color-mix(in srgb, var(--sb-warning) 40%, var(--border));
		border-radius: 2px;
		background: color-mix(in srgb, var(--sb-warning) 8%, var(--background));
	}
	.alert summary {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 7px 10px;
		cursor: pointer;
		list-style: none;
	}
	.alert summary::-webkit-details-marker {
		display: none;
	}
	.alert summary > :global(svg) {
		flex: 0 0 auto;
		color: var(--sb-warning);
	}
	.summary-text {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.summary-text b {
		font-weight: 600;
	}
	.more {
		flex: 0 0 auto;
		margin-left: auto;
		color: var(--muted-foreground);
		text-decoration: underline;
	}
	.alert[open] .more {
		visibility: hidden;
	}
	.note {
		display: flex;
		align-items: baseline;
		gap: 8px;
		padding: 7px 10px;
	}
	.note > :global(svg) {
		flex: 0 0 auto;
		align-self: center;
		color: var(--sb-warning);
	}
	.note .summary-text {
		white-space: normal;
	}
	.alert-body {
		padding: 0 10px 10px 32px;
		color: var(--muted-foreground);
	}
	.alert-body p {
		margin-bottom: 8px;
	}
	.facts summary {
		width: max-content;
		color: var(--muted-foreground);
		cursor: pointer;
	}
	.facts-body {
		max-width: 640px;
		padding: 8px 0 2px;
	}
	#problems {
		scroll-margin-top: 12px;
	}
	pre {
		overflow: auto;
		padding: 8px 10px;
		border-radius: 2px;
		background: var(--sb-inset);
		color: var(--sb-mono-err);
		font-family: var(--font-mono);
		font-size: 12px;
		white-space: pre-wrap;
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.muted {
		color: var(--muted-foreground);
	}
	.warn {
		color: var(--sb-warning);
	}
	.err {
		color: var(--sb-error);
		font-weight: 500;
	}
</style>
