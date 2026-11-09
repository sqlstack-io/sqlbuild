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
	const unhealthy = $derived(all.filter((l) => (l.health && l.health !== 'healthy') || l.activeProblems > 0).length);
	const opensDetail = $derived(app.client.supports('getLocation'));
	const showsServer = $derived(app.client.supports('platformHealth'));
	const knowsPolicy = $derived(all.some((l) => l.brokenDeployPolicy));
</script>

<AppTopbar title="Locations">
	<Button variant="outline" size="sm" onclick={() => (locations.refresh(), status?.refresh())}>↻ Refresh</Button>
</AppTopbar>

{#if showsServer}<ServerStatus bind:this={status} />{/if}

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
				{#if knowsPolicy}<th title="What happens when a deploy fails to load">If a deploy fails</th>{/if}
			</tr>
		</thead>
		<tbody>
			{#each filtered as l (l.id)}
				<tr class:static={!opensDetail} onclick={() => opensDetail && goto(`/locations/${l.id}`)}>
					<td>
						<div class="cell-name">
							<MapPinIcon size={15} strokeWidth={1.75} />
							<div class="min-w-0">
								{#if opensDetail}<a href="/locations/{l.id}" onclick={(e) => e.stopPropagation()}>{l.name}</a>{:else}{l.name}{/if}
								<div class="cell-desc" title={l.target.connectionSummary}>{targetWords(l.target)}</div>
							</div>
						</div>
					</td>
					<td>
						{#if l.health}
							<StatusLabel tone={l.failedDeploy || l.activeProblems ? healthTone[l.health === 'healthy' ? 'degraded' : l.health] : healthTone[l.health]} label={healthWords(l)} />
						{:else}
							<span title="You need view on every node of this location to see its health"><StatusLabel tone="var(--sb-text-faint)" label="Unknown" muted /></span>
						{/if}
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
						{#if l.lastSeenAt}<RelativeTime value="last check-in {formatAgo(l.lastSeenAt, app.now)}" exact={formatStamp(l.lastSeenAt)} />{/if}
					</td>
					{#if knowsPolicy}<td class="muted">{l.brokenDeployPolicy === 'fallback' ? 'Keep running the previous version' : l.brokenDeployPolicy ? 'Hold scheduled runs until a deploy works' : '—'}</td>{/if}
				</tr>
			{:else}
				<LoadRow colspan={knowsPolicy ? 6 : 5} resource={locations} empty={all.length ? 'No locations match the search.' : 'No code locations yet. Register one with the server using a location token.'} />
			{/each}
		</tbody>
	</table>
</div>

<style>
	tr.static {
		cursor: default;
	}
</style>
