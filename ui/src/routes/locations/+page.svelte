<script lang="ts">
	import { goto } from '$app/navigation';
	import MapPinIcon from '@lucide/svelte/icons/map-pin';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import LoadRow from '$lib/components/list/load-row.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { hasCapability, type ConnectionTest } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { errorMessage, healthTone, healthWords, targetWords, testTone } from '$lib/platform/labels';
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
	const partialTitle = 'You need view on every node of this location to see its health, deploys and processes';

	// Connection tests run per location, so each row checks that location's own permission.
	const mayTest = (id: string) =>
		app.client.supports('testConnection') &&
		(hasCapability(app.bootstrap?.capabilities, 'connection.test') || hasCapability(app.bootstrap?.locations.find((l) => l.id === id)?.capabilities, 'connection.test'));
	// A fresh result can be newer than the page clock's last tick; it then reads "just now".
	let testing = $state<Record<string, boolean>>({});
	let tested = $state<Record<string, ConnectionTest>>({});
	let testFailures = $state<Record<string, string>>({});
	async function testConnection(id: string) {
		testing[id] = true;
		delete testFailures[id];
		try {
			tested[id] = await app.client.testConnection({ locationId: id }, 'warehouse');
		} catch (error) {
			testFailures[id] = errorMessage(error);
		} finally {
			testing[id] = false;
		}
	}
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
				{@const test = tested[l.id] ?? l.connectionTest}
				{@const opens = opensDetail && l.health !== null}
				<tr class:static={!opens} onclick={() => opens && goto(`/locations/${l.id}`)}>
					<td>
						<div class="cell-name">
							<MapPinIcon size={15} strokeWidth={1.75} />
							<div class="min-w-0">
								{#if opens}<a href="/locations/{l.id}" onclick={(e) => e.stopPropagation()}>{l.name}</a>{:else}{l.name}{/if}
								<div class="cell-desc" title={l.target.connectionSummary}>{targetWords(l.target)}</div>
							</div>
						</div>
					</td>
					<td>
						{#if l.health}
							<StatusLabel tone={l.failedDeploy || l.activeProblems ? healthTone[l.health === 'healthy' ? 'degraded' : l.health] : healthTone[l.health]} label={healthWords(l)} />
						{:else}
							<span title={partialTitle}><StatusLabel tone="var(--sb-text-faint)" label="Unknown" muted /></span>
						{/if}
						{#if l.activeProblems}
							<div class="cell-desc"><a class="link" href="/locations/{l.id}#problems" onclick={(e) => e.stopPropagation()}>{l.activeProblems} open problem{l.activeProblems === 1 ? '' : 's'}</a></div>
						{/if}
					</td>
					<td>
						{#if l.health === null}
							<span class="muted" title={partialTitle}>—</span>
						{:else if l.snapshot}
							<div title="Commit {l.snapshot.gitRevision} by {l.snapshot.gitAuthor}, deployed {formatStamp(l.snapshot.publishedAt)} (snapshot generation {l.snapshot.generation})">Version {l.snapshot.generation}</div>
							<div class="cell-desc">deployed {formatAgo(l.snapshot.publishedAt, app.now)}</div>
						{:else}
							<span class="muted">Nothing deployed yet</span>
						{/if}
					</td>
					<td>
						{#if testing[l.id]}
							<StatusLabel tone="var(--sb-running)" label="Testing…" />
						{:else if test}
							<span title="{test.message} ({formatStamp(test.at)})">
								<StatusLabel tone={testTone[test.outcome]} label={test.outcome === 'ok' ? 'Connected' : 'Couldn’t connect'} />
							</span>
							<div class="cell-desc" title={test.message}>{test.trigger === 'manual' ? `tested ${formatAgo(test.at, Math.max(app.now, Date.parse(test.at)))} by you` : `tested ${formatAgo(test.at, app.now)} after deploying`}</div>
						{:else if l.health === null}
							<span class="muted" title={partialTitle}>—</span>
						{:else}
							<StatusLabel tone="var(--sb-text-faint)" label="Not tested yet" muted />
						{/if}
						{#if testFailures[l.id]}<div class="cell-desc err" title={testFailures[l.id]}>{testFailures[l.id]}</div>{/if}
						{#if mayTest(l.id)}
							<button class="test" disabled={testing[l.id]} onclick={(e) => (e.stopPropagation(), testConnection(l.id))} title="Ask the location to connect to its warehouse, as sqb debug does">Test connection</button>
						{/if}
					</td>
					<td>
						{#if l.health === null}<span class="muted" title={partialTitle}>—</span>{:else}<div>{l.liveInstances === 1 ? 'Running' : l.liveInstances === 0 ? 'Not running' : `${l.liveInstances} running`}</div>
						{#if l.lastSeenAt}<RelativeTime value="last check-in {formatAgo(l.lastSeenAt, app.now)}" exact={formatStamp(l.lastSeenAt)} />{/if}{/if}
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
	.test { display: block; margin-top: 4px; color: var(--primary); font-size: 12px; }
	.test:hover:not(:disabled) { text-decoration: underline; }
	.test:disabled { color: var(--sb-text-faint); }
	.err { color: var(--sb-error) !important; }
</style>
