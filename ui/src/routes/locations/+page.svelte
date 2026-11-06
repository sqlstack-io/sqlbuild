<script lang="ts">
	import { goto } from '$app/navigation';
	import MapPinIcon from '@lucide/svelte/icons/map-pin';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import LoadRow from '$lib/components/list/load-row.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { healthTone, healthWords, targetWords, testTone } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import ServerStatus from './server-status.svelte';

	const locations = createResource((client) => client.listLocations(), ['snapshot', 'platform_events']);
	let status = $state<ServerStatus>();

	let query = $state('');
	const all = $derived(locations.data ?? []);
	const filtered = $derived(all.filter((l) => !query.trim() || l.name.toLowerCase().includes(query.trim().toLowerCase())));
	const unhealthy = $derived(all.filter((l) => l.health !== 'healthy' || l.activeProblems > 0).length);
</script>

<AppTopbar title="Locations">
	<Button variant="outline" size="sm" onclick={() => (locations.refresh(), status?.refresh())}>↻ Refresh</Button>
</AppTopbar>

<ServerStatus bind:this={status} />

<ListToolbar
	bind:query
	placeholder="Search locations"
	total="{filtered.length} of {all.length} locations"
	counts={[{ tone: 'var(--sb-warning)', count: unhealthy, label: 'need attention' }]}
/>

<div class="sb-list-wrap">
	<table class="sb-list">
		<thead>
			<tr>
				<th>Location</th>
				<th>Health</th>
				<th>Deployed version</th>
				<th>Warehouse</th>
				<th>Location processes</th>
				<th title="What happens when a deploy fails to load">If a deploy fails</th>
			</tr>
		</thead>
		<tbody>
			{#each filtered as l (l.id)}
				<tr onclick={() => goto(`/locations/${l.id}`)}>
					<td>
						<div class="cell-name">
							<MapPinIcon size={15} strokeWidth={1.75} />
							<div class="min-w-0">
								<a href="/locations/{l.id}" onclick={(e) => e.stopPropagation()}>{l.name}</a>
								<div class="cell-desc" title={l.target.connectionSummary}>{targetWords(l.target)}</div>
							</div>
						</div>
					</td>
					<td>
						<StatusLabel tone={l.failedDeploy || l.activeProblems ? healthTone[l.health === 'healthy' ? 'degraded' : l.health] : healthTone[l.health]} label={healthWords(l)} />
						{#if l.activeProblems}
							<div class="cell-desc"><a class="link" href="/locations/{l.id}#problems" onclick={(e) => e.stopPropagation()}>{l.activeProblems} open problem{l.activeProblems === 1 ? '' : 's'}</a></div>
						{/if}
					</td>
					<td>
						{#if l.snapshot}
							<div title="Commit {l.snapshot.gitRevision} by {l.snapshot.gitAuthor}, deployed {formatStamp(l.snapshot.publishedAt)} (snapshot generation {l.snapshot.generation})">Version {l.snapshot.generation}</div>
							<div class="cell-desc">deployed {formatAgo(l.snapshot.publishedAt, app.now)}</div>
						{:else}
							<span class="muted">Nothing deployed yet</span>
						{/if}
					</td>
					<td>
						{#if l.connectionTest}
							<span title="{l.connectionTest.message} ({formatStamp(l.connectionTest.at)})">
								<StatusLabel tone={testTone[l.connectionTest.outcome]} label={l.connectionTest.outcome === 'ok' ? 'Connected' : 'Couldn’t connect'} />
							</span>
							<div class="cell-desc">tested {formatAgo(l.connectionTest.at, app.now)}</div>
						{:else}
							<StatusLabel tone="var(--sb-text-faint)" label="Not tested yet" muted />
						{/if}
					</td>
					<td>
						<div>{l.liveInstances === 1 ? 'Running' : l.liveInstances === 0 ? 'Not running' : `${l.liveInstances} running`}</div>
						<RelativeTime value="last check-in {formatAgo(l.lastSeenAt, app.now)}" exact={formatStamp(l.lastSeenAt)} />
					</td>
					<td class="muted">{l.brokenDeployPolicy === 'fallback' ? 'Keep running the previous version' : 'Hold scheduled runs until a deploy works'}</td>
				</tr>
			{:else}
				<LoadRow colspan={6} resource={locations} empty={all.length ? 'No locations match the search.' : 'No code locations yet. Register one with the server using a location token.'} />
			{/each}
		</tbody>
	</table>
</div>
