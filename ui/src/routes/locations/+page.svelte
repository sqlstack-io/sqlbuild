<script lang="ts">
	import { plural } from '$lib/format/plural';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Button from '$lib/components/ui/button/button.svelte';
	import { openRow, to } from '$lib/paths';
	import MapPinIcon from '@lucide/svelte/icons/map-pin';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { hasCapability, type ConnectionTest } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { errorMessage, healthTone, healthWords, targetWords, testTone, brokenDeployPolicyHelp, brokenDeployPolicyLabel } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import ServerStatus from './server-status.svelte';

	const locations = createResource((client) => client.listLocations(), ['snapshot', 'platform_events', 'warehouse_sync', 'location_health']);
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
	const tableColumns = $derived<TableColumn[]>([
		{ id: 'location', label: 'Location', minWidth: 220 },
		{ id: 'health', label: 'Health', width: '170px' },
		{ id: 'version', label: 'Deployed version', width: '160px' },
		{ id: 'warehouse', label: 'Warehouse', width: '200px' },
		{ id: 'processes', label: 'Location processes', width: '180px' },
		...(knowsPolicy ? [{ id: 'policy', label: 'If a deploy fails', width: '220px', title: 'What happens when a deploy fails to load' }] : [])
	]);
</script>

<AppTopbar title="Locations">
	<RefreshButton onclick={() => (locations.refresh(), status?.refresh())} />
</AppTopbar>

{#if showsServer}<ServerStatus bind:this={status} />{/if}

<ListToolbar
	loading={locations.data === undefined}
	bind:query
	placeholder="Search locations"
	total="{filtered.length} of {all.length} locations"
	counts={[{ tone: 'var(--sb-warning)', count: unhealthy, label: 'need attention' }]}
/>

<DataTable columns={tableColumns} rows={filtered} key={(l) => l.id} resource={locations} empty={all.length ? 'No locations match the search.' : 'No locations yet.'}>
	{#snippet row(l)}
				{@const test = tested[l.id] ?? l.connectionTest}
				{@const opens = opensDetail && l.health !== null}
				<tr class:static={!opens} onclick={(e) => opens && openRow(e, `/locations/${l.id}`)}>
					<td>
						<div class="cell-name">
							<MapPinIcon size={15} strokeWidth={1.75} />
							<div class="min-w-0">
								{#if opens}<a href={to(`/locations/${l.id}`)}>{l.name}</a>{:else}{l.name}{/if}
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
							<div class="cell-desc"><a class="link" href={to(`/locations/${l.id}#problems`)}>{plural(l.activeProblems, 'open problem')}</a></div>
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
							<div class="cell-desc" title={test.message}>{test.trigger === 'manual' ? `tested ${formatAgo(test.at, Math.max(app.now, Date.parse(test.at)))} by you` : `tested ${formatAgo(test.at, app.now)} ${test.trigger === 'startup' ? 'on startup' : 'after deploying'}`}</div>
						{:else if l.health === null}
							<span class="muted" title={partialTitle}>—</span>
						{:else}
							<StatusLabel tone="var(--sb-text-faint)" label="Not tested yet" muted />
						{/if}
						{#if testFailures[l.id]}<div class="cell-desc err" title={testFailures[l.id]}>{testFailures[l.id]}</div>{/if}
						{#if mayTest(l.id)}
							<Button variant="link" size="inline" class="mt-1 flex text-[12px]" disabled={testing[l.id]} onclick={(e: MouseEvent) => (e.stopPropagation(), testConnection(l.id))} title="Ask the location to connect to its warehouse, as sqb debug does">Test connection</Button>
						{/if}
					</td>
					<td>
						{#if l.health === null}<span class="muted" title={partialTitle}>—</span>{:else}<div>{l.liveInstances === 1 ? 'Running' : l.liveInstances === 0 ? 'Not running' : `${l.liveInstances} running`}</div>
						{#if l.lastSeenAt}<RelativeTime value="last check-in {formatAgo(l.lastSeenAt, app.now)}" exact={formatStamp(l.lastSeenAt)} />{/if}{/if}
					</td>
					{#if knowsPolicy}<td class="muted" title={l.brokenDeployPolicy ? brokenDeployPolicyHelp[l.brokenDeployPolicy === 'fallback' ? 'fallback' : 'hold'] : undefined}>{l.brokenDeployPolicy ? brokenDeployPolicyLabel[l.brokenDeployPolicy === 'fallback' ? 'fallback' : 'hold'] : '—'}</td>{/if}
				</tr>
	{/snippet}
</DataTable>

<style>
	tr.static {
		cursor: default;
	}
	.err { color: var(--sb-error) !important; }
</style>
