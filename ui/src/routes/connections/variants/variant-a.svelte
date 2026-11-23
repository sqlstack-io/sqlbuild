<script lang="ts">
	import type { WarehouseConnection } from '$lib/api';
	import PageBody from '$lib/components/page-body.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import Loading from '$lib/components/loading.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import ConnectionTest from '$lib/components/platform/connection-test.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import HelpTip from '$lib/components/help-tip.svelte';
	import { to } from '$lib/paths';
	import { detailRows } from '$lib/platform/connections';
	import { app } from '$lib/state/app.svelte';
	import CheckStatus from './check-status.svelte';
	import SyncTables from './sync-tables.svelte';
	import { createConnectionsState } from './state.svelte';
	import { syncFacts, targetName, where } from './utils';

	// Variant A: the detail-page pattern. One panel per connection, named after what uses it, with the
	// last check and Test connection in its header, settings and use side by side as key-value facts,
	// and the state history sync under them, its clean tables folded into one line.
	const conn = createConnectionsState();
	const SYNC_HELP = 'The server mirrors SQLBuild’s own tables from the warehouse, for history and lineage.';
</script>

<AppTopbar title="Connections">
	<RefreshButton onclick={conn.refresh} />
</AppTopbar>

{#snippet sync(c: WarehouseConnection, named: boolean)}
	<div class="sync">
		<div class="sub-head">
			<h3>State history sync</h3>
			<HelpTip text={SYNC_HELP} label="About state history sync" />
			{#if named}<span class="muted">{targetName(c)}</span>{/if}
		</div>
		{#if named}<KeyValues rows={[{ label: 'State tables', value: c.stateLocation ? where(c.stateLocation) : 'Not reported yet', mono: !!c.stateLocation }, ...syncFacts(c, app.now)]} />{/if}
		<SyncTables tables={c.tables} />
	</div>
{/snippet}

<PageBody padding="20px 24px 48px">
	<div class="content">
		{#if conn.resource.error}
			<p class="err">Couldn't load connections: {conn.resource.error} <Button variant="link" size="inline" class="ml-2" onclick={conn.refresh}>Retry</Button></p>
		{/if}

		{#each conn.groups as g (g.key)}
			{@const single = g.users.length === 1 ? g.users[0] : null}
			<Panel title={single ? targetName(single) : `${g.users.length} targets`} note={g.details?.adapter}>
				{#snippet actions()}
					{#if single}
						<CheckStatus test={single.lastTest} />
						<ConnectionTest scope={{ locationId: single.id }} connection={single.target} last={single.lastTest} showLast={false} ontested={conn.refresh} />
					{/if}
				{/snippet}
				<div class="card">
					<div class="facts">
						<div>
							<h3 class="col-head">Settings</h3>
							<KeyValues rows={detailRows(g.details).map((r) => ({ label: r.label, value: r.value ?? '—', mono: r.value !== null && r.label !== 'Authentication', tone: r.value === null ? 'var(--sb-text-faint)' : undefined }))} />
							{#if !g.details}<p class="faint" title="Secrets are never shown">Settings not published</p>{/if}
						</div>
						<div>
							<h3 class="col-head">Use</h3>
							{#snippet usedBy()}
								{#each g.users as c, i (c.id)}{#if i}, {/if}<a class="link" href={to(`/locations/${encodeURIComponent(c.id)}`)}>{targetName(c)}</a>{/each}
							{/snippet}
							<KeyValues
								rows={single
									? [
											{ label: 'Used by', content: usedBy },
											{ label: 'State tables', value: single.stateLocation ? where(single.stateLocation) : 'Not reported yet', mono: !!single.stateLocation },
											...syncFacts(single, app.now)
										]
									: [{ label: 'Used by', content: usedBy }]}
							/>
						</div>
					</div>
					{#each g.users as c (c.id)}
						{@render sync(c, !single)}
					{/each}
				</div>
			</Panel>
		{:else}
			{#if conn.resource.data}<p class="muted" title="Only locations you can see in full are listed">Nothing reported yet.</p>{:else if !conn.resource.error}<Loading />{/if}
		{/each}
	</div>
</PageBody>

<style>
	.content {
		max-width: 960px;
	}
	.card {
		display: flex;
		flex-direction: column;
		padding: 16px 18px;
		border: 1px solid var(--panel-border, var(--border));
		border-radius: 2px;
		background: var(--card);
		box-shadow: var(--panel-shadow);
	}
	.facts {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
		gap: 16px 40px;
		padding-bottom: 16px;
	}
	.facts :global(.key-values) {
		grid-template-columns: 116px minmax(0, 1fr);
	}
	.col-head {
		margin-bottom: 8px;
		color: var(--sb-text-faint);
		font-size: 11.5px;
		font-weight: 500;
	}
	.sync {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding-top: 14px;
		border-top: 1px solid var(--border);
	}
	.sync + .sync {
		margin-top: 16px;
	}
	.sub-head {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	h3 {
		font-size: 12.5px;
		font-weight: 600;
	}
	.col-head {
		font-weight: 500;
	}
	.link {
		color: var(--primary);
	}
	.link:hover {
		text-decoration: underline;
	}
	.err {
		margin-bottom: 16px;
		color: var(--sb-error);
		font-size: 12.5px;
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.faint {
		margin-top: 6px;
		color: var(--sb-text-faint);
		font-size: 12px;
	}
</style>
