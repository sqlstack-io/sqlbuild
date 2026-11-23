<script lang="ts">
	import PageBody from '$lib/components/page-body.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import Loading from '$lib/components/loading.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import ConnectionTest from '$lib/components/platform/connection-test.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import DatabaseIcon from '@lucide/svelte/icons/database';
	import { to } from '$lib/paths';
	import { plural } from '$lib/format/plural';
	import { detailRows } from '$lib/platform/connections';
	import { app } from '$lib/state/app.svelte';
	import CheckStatus from './check-status.svelte';
	import SyncTables from './sync-tables.svelte';
	import { createConnectionsState } from './state.svelte';
	import { failing, syncFacts, targetName, where } from './utils';

	// Variant B: two columns on wide screens. The connection on the left, its state history sync on the
	// right, both cards the same height with header rows of the same height; one column below 1400 px.
	const conn = createConnectionsState();
	const SYNC_HELP = 'The server mirrors SQLBuild’s own tables from the warehouse, for history and lineage.';
	const setting = (r: { label: string; value: string | null }) => ({ label: r.label, value: r.value ?? '—', mono: r.value !== null && r.label !== 'Authentication', tone: r.value === null ? 'var(--sb-text-faint)' : undefined });
</script>

<AppTopbar title="Connections">
	<RefreshButton onclick={conn.refresh} />
</AppTopbar>

<PageBody padding="20px 24px 48px">
	{#if conn.resource.error}
		<p class="err">Couldn't load connections: {conn.resource.error} <Button variant="link" size="inline" class="ml-2" onclick={conn.refresh}>Retry</Button></p>
	{/if}

	{#each conn.groups as g (g.key)}
		{@const single = g.users.length === 1 ? g.users[0] : null}
		{@const rows = detailRows(g.details)}
		<div class="pair">
			<div class="col">
				<Panel title="Connection" note={g.details?.adapter}>
					<div class="card">
						<div class="card-head">
							<DatabaseIcon size={15} strokeWidth={1.75} />
							<span class="name">{single ? targetName(single) : `${g.users.length} targets`}</span>
							{#if single}
								<span class="right">
									<CheckStatus test={single.lastTest} />
									<ConnectionTest scope={{ locationId: single.id }} connection={single.target} last={single.lastTest} showLast={false} ontested={conn.refresh} />
								</span>
							{/if}
						</div>
						{#snippet usedBy()}
							{#each g.users as c, i (c.id)}{#if i}, {/if}<a class="link" href={to(`/locations/${encodeURIComponent(c.id)}`)}>{targetName(c)}</a>{/each}
						{/snippet}
						<div class="facts">
							<KeyValues rows={rows.slice(0, 4).map(setting)} />
							<KeyValues
								rows={[
									...rows.slice(4).map(setting),
									{ label: 'Used by', content: usedBy },
									...(single ? [{ label: 'State tables', value: single.stateLocation ? where(single.stateLocation) : 'Not reported yet', mono: !!single.stateLocation }] : [])
								]}
							/>
						</div>
						{#if !g.details}<p class="faint" title="Secrets are never shown">Settings not published</p>{/if}
					</div>
				</Panel>
			</div>

			<div class="col">
				{#each g.users as c (c.id)}
					{@const bad = failing(c.tables)}
					<Panel title="State history sync" help={SYNC_HELP} note={single ? plural(c.tables.length, 'table') : targetName(c)}>
						<div class="card">
							<div class="card-head">
								{#if !c.tables.length}
									<StatusLabel tone="var(--sb-text-faint)" label="Not reported yet" muted />
								{:else if bad}
									<StatusLabel tone="var(--sb-error)" label="{bad} failing" />
								{:else}
									<StatusLabel tone="var(--sb-success)" label="All synced" />
								{/if}
							</div>
							<dl class="inline-facts">
								{#each syncFacts(c, app.now) as f (f.label)}
									<div><dt>{f.label}</dt><dd style:color={f.tone} title={f.title}>{f.value}</dd></div>
								{/each}
							</dl>
							<SyncTables tables={c.tables} />
						</div>
					</Panel>
				{/each}
			</div>
		</div>
	{:else}
		{#if conn.resource.data}<p class="muted" title="Only locations you can see in full are listed">Nothing reported yet.</p>{:else if !conn.resource.error}<Loading />{/if}
	{/each}
</PageBody>

<style>
	.pair {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 0 24px;
		max-width: 1800px;
	}
	@media (min-width: 1500px) {
		.pair {
			grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		}
	}
	.col {
		display: flex;
		min-width: 0;
		flex-direction: column;
	}
	.col > :global(section) {
		display: flex;
		flex: 1;
		flex-direction: column;
	}
	.card {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 14px;
		padding: 14px 16px 16px;
		border: 1px solid var(--panel-border, var(--border));
		border-radius: 2px;
		background: var(--card);
		box-shadow: var(--panel-shadow);
	}
	.card-head {
		display: flex;
		min-height: 32px;
		align-items: center;
		gap: 9px;
		font-size: 12.5px;
	}
	.card-head > :global(svg) {
		color: var(--muted-foreground);
	}
	.name {
		font-size: 13px;
		font-weight: 500;
	}
	.right {
		display: flex;
		align-items: center;
		gap: 12px;
		margin-left: auto;
	}
	.facts {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
		align-items: start;
		gap: 7px 32px;
	}
	.facts :global(.key-values) {
		grid-template-columns: 116px minmax(0, 1fr);
	}
	.inline-facts {
		display: flex;
		flex-wrap: wrap;
		gap: 8px 32px;
		margin: 0;
		font-size: 12.5px;
	}
	.inline-facts dt {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.inline-facts dd {
		margin: 2px 0 0;
		font-variant-numeric: tabular-nums;
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
		color: var(--sb-text-faint);
		font-size: 12px;
	}
</style>
