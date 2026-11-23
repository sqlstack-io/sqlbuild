<script lang="ts">
	import PageBody from '$lib/components/page-body.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import Loading from '$lib/components/loading.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import ConnectionTest from '$lib/components/platform/connection-test.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import HelpTip from '$lib/components/help-tip.svelte';
	import DatabaseIcon from '@lucide/svelte/icons/database';
	import { to } from '$lib/paths';
	import { plural } from '$lib/format/plural';
	import { detailRows } from '$lib/platform/connections';
	import { app } from '$lib/state/app.svelte';
	import CheckStatus from './check-status.svelte';
	import SyncTables from './sync-tables.svelte';
	import { createConnectionsState } from './state.svelte';
	import { failing, syncFacts, targetName, where } from './utils';

	// Variant D: status first. A strip of counts, then one compact card per target in a grid, two or
	// three a row on wide screens; targets with a failed check or a failing table come first, marked.
	const conn = createConnectionsState(true);
	const SYNC_HELP = 'The server mirrors SQLBuild’s own tables from the warehouse, for history and lineage.';
	const cards = $derived(conn.groups.flatMap((g) => g.users.map((c) => ({ c, details: g.details }))));
	const targets = $derived(cards.length);
	const checked = $derived(cards.filter(({ c }) => c.lastTest).length);
	const passing = $derived(cards.filter(({ c }) => c.lastTest?.outcome === 'ok').length);
	const tables = $derived(cards.reduce((n, { c }) => n + c.tables.length, 0));
	const failingTables = $derived(cards.reduce((n, { c }) => n + failing(c.tables), 0));
	const setting = (r: { label: string; value: string | null }) => ({ label: r.label, value: r.value ?? '—', mono: r.value !== null && r.label !== 'Authentication', tone: r.value === null ? 'var(--sb-text-faint)' : undefined });
</script>

<AppTopbar title="Connections">
	<RefreshButton onclick={conn.refresh} />
</AppTopbar>

<PageBody padding="20px 24px 48px">
	{#if conn.resource.error}
		<p class="err">Couldn't load connections: {conn.resource.error} <Button variant="link" size="inline" class="ml-2" onclick={conn.refresh}>Retry</Button></p>
	{/if}

	{#if cards.length}
		<section class="metric-strip">
			<div class="metric">
				<span class="metric-label">Targets</span>
				<div><strong>{targets}</strong><small>{conn.groups.length} distinct connections</small></div>
			</div>
			<div class="metric">
				<span class="metric-label">Checks passing</span>
				<div><strong>{passing}</strong><small>of {targets}</small></div>
				<span class:bad={passing < checked}>{checked - passing} failing</span>
			</div>
			<div class="metric">
				<span class="metric-label">Sync tables failing <HelpTip text={SYNC_HELP} label="About state history sync" /></span>
				<div><strong>{failingTables}</strong><small>of {tables}</small></div>
				<span class:bad={failingTables > 0}>{failingTables ? 'newest sync failed' : 'all synced'}</span>
			</div>
		</section>
	{/if}

	<div class="grid">
		{#each cards as { c, details } (c.id)}
			{@const bad = failing(c.tables)}
			{@const problem = c.lastTest?.outcome === 'failed' || bad > 0}
			<article class="card" class:problem>
				<header>
					<DatabaseIcon size={15} strokeWidth={1.75} />
					<div class="min-w-0">
						<a class="name" href={to(`/locations/${encodeURIComponent(c.id)}`)}>{targetName(c)}</a>
						<div class="sub">{details ? [details.adapter, details.account ?? details.host].filter(Boolean).join(' · ') : 'Settings not published'}</div>
					</div>
					<span class="right"><ConnectionTest scope={{ locationId: c.id }} connection={c.target} last={c.lastTest} showLast={false} ontested={conn.refresh} /></span>
				</header>
				<div class="facts">
					<KeyValues rows={[{ label: 'Last check', content: check, title: '' }, ...detailRows(details).slice(2).map(setting)]} />
					<KeyValues rows={[{ label: 'State tables', value: c.stateLocation ? where(c.stateLocation) : 'Not reported yet', mono: !!c.stateLocation }, ...syncFacts(c, app.now)]} />
				</div>
				{#snippet check()}<CheckStatus test={c.lastTest} brief />{/snippet}
				<SyncTables tables={c.tables} compact />
			</article>
		{/each}
	</div>
	{#if !cards.length}
		{#if conn.resource.data}<p class="muted" title="Only locations you can see in full are listed">Nothing reported yet.</p>{:else if !conn.resource.error}<Loading />{/if}
	{/if}
</PageBody>

<style>
	.metric-strip {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		margin-bottom: 20px;
		overflow: hidden;
		border: 1px solid var(--panel-border, var(--border));
		border-radius: 2px;
		background: var(--card);
		box-shadow: var(--panel-shadow);
	}
	.metric {
		min-width: 0;
		padding: 13px 16px 12px;
		border-right: 1px solid var(--border);
	}
	.metric:last-child {
		border-right: 0;
	}
	.metric-label {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--muted-foreground);
		font: 500 12px var(--font-sans);
	}
	.metric > div {
		display: flex;
		align-items: baseline;
		gap: 8px;
		margin-top: 7px;
	}
	.metric strong {
		font: 600 22px/1 var(--font-sans);
		font-variant-numeric: tabular-nums;
	}
	.metric > span:last-child {
		display: block;
		margin-top: 8px;
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.metric > span.bad {
		color: var(--sb-error);
	}
	.metric small {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(640px, 1fr));
		gap: 16px;
		align-items: start;
	}
	.card {
		display: flex;
		min-width: 0;
		flex-direction: column;
		gap: 14px;
		padding: 14px 16px 16px;
		border: 1px solid var(--panel-border, var(--border));
		border-radius: 2px;
		background: var(--card);
		box-shadow: var(--panel-shadow);
	}
	.card.problem {
		border-color: color-mix(in srgb, var(--sb-error) 30%, var(--border));
		border-left: 3px solid var(--sb-error);
	}
	header {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	header > :global(svg) {
		flex: 0 0 auto;
		color: var(--muted-foreground);
	}
	.name {
		font-size: 13px;
		font-weight: 500;
	}
	.name:hover {
		color: var(--primary);
	}
	.sub {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.right {
		margin-left: auto;
	}
	.facts {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		align-items: start;
		gap: 7px 28px;
	}
	.facts :global(.key-values) {
		grid-template-columns: 116px minmax(0, 1fr);
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
</style>
