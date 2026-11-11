<script lang="ts">
	import { to } from '$lib/paths';
	// Every rerun opens here first, like every Run button opens the plan page: it shows what the rerun repeats
	// and nothing reruns until Run is pressed. A rerun repeats its run's frozen version and settings, so it is
	// not planned against the current project.
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import RotateCwIcon from '@lucide/svelte/icons/rotate-cw';
	import type { Capability, RerunMode, RunDetail } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import { kindLine } from '$lib/build-area/labels';
	import { formatAgo } from '$lib/format/time';
	import ScopeGraph from '$lib/lineage/scope-graph.svelte';
	import { shortRunId } from '$lib/operate/labels';
	import { PERMISSIONS } from '$lib/platform/permission-catalog';
	import { rerunHref } from '$lib/run/command';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { runStatusLabel } from '$lib/status';
	import { rerunChoices } from '../../runs/[id]/utils';

	const runIds = page.url.searchParams.getAll('run');
	const picked = page.url.searchParams.getAll('node');
	const requested = (page.url.searchParams.get('mode') ?? 'all') as RerunMode;
	const single = runIds.length === 1;

	const details = createResource(async (client, scope) => (await Promise.all(runIds.map((id) => client.getRunDetail(scope, id)))).filter((d): d is RunDetail => !!d), runIds.map((id) => `run:${id}`));
	const catalog = createResource((client, scope) => client.listCatalogNodes(scope), ['snapshot']);
	const byKey = $derived(new Map((catalog.data ?? []).map((n) => [n.key, n])));

	const can = (c: Capability) => app.can(c);
	const first = $derived(details.data?.[0] ?? null);
	/** One run's choices, as its rerun button offers them; reruns of several runs only rerun all or from failure. */
	const choices = $derived(single && first ? rerunChoices(first, app.client.supports('rerunSelectedNodes') ? picked : [], can) : []);
	let mode = $state<RerunMode>(requested);
	const nodesOf = (d: RunDetail): string[] => (mode === 'selected' ? picked : mode === 'from_failure' ? d.rerunFromFailureNodes : d.selection);
	const blockedOf = (d: RunDetail): string | null => rerunChoices(d, [], can).find((c) => c.mode === (mode === 'selected' ? 'all' : mode))?.disabledReason ?? null;
	const rows = $derived((details.data ?? []).map((d) => ({ detail: d, nodes: nodesOf(d), blocked: blockedOf(d) })));
	const ready = $derived(rows.filter((r) => !r.blocked && r.nodes.length));
	const nodes = $derived(single && rows[0] ? rows[0].nodes : []);
	const needs = $derived<Capability[]>(['run.rerun', ...((details.data ?? []).some((d) => d.run.rewriteHistory) ? (['run.rewrite_history'] as const) : [])]);
	let view = $state<'nodes' | 'graph'>('nodes');
	const drawable = $derived(nodes.filter((k) => byKey.has(k)));

	function choose(next: RerunMode) {
		mode = next;
		void goto(to(rerunHref(runIds, next, next === 'selected' ? picked : [])), { replaceState: true, keepFocus: true, noScroll: true });
	}

	const loadError = $derived(details.error ? String(details.error) : details.data && !details.data.length ? 'These runs were not found in this location.' : null);
	const blockedReason = $derived(
		!details.data ? 'Loading the runs' : !ready.length ? (rows.find((r) => r.blocked)?.blocked ?? 'Nothing to rerun') : rows.length > ready.length ? `${rows.length - ready.length} of these runs can't be rerun this way` : undefined
	);
	const canRun = $derived(ready.length > 0);

	let launching = $state(false);
	let launchError = $state<string | null>(null);
	async function launch() {
		const scope = app.scope;
		if (!scope || !canRun) return;
		launching = true;
		launchError = null;
		const outcomes = await Promise.allSettled(
			ready.map(({ detail }) => (mode === 'selected' ? app.client.rerunSelectedNodes(scope, detail.run.id, picked) : app.client.rerunRun(scope, detail.run.id, mode as Exclude<RerunMode, 'selected'>)))
		);
		launching = false;
		const failed = outcomes.flatMap((o) => (o.status === 'rejected' ? [o.reason] : []));
		const started = outcomes.flatMap((o) => (o.status === 'fulfilled' ? [o.value] : []));
		if (failed.length) {
			const reason = failed[0] instanceof Error ? failed[0].message : String(failed[0]);
			launchError = `${started.length ? `Started ${started.length}; ` : ''}${failed.length} failed: ${reason}`;
			return;
		}
		await goto(to(started.length === 1 ? `/runs/${started[0].id}` : '/runs'));
	}

	const what = (d: RunDetail) => d.run.jobKey ?? 'ad-hoc run';
</script>

<AppTopbar title="Plan" />

<div class="flex min-h-0 flex-1 flex-col">
	<div class="cmd">
		<span class="jobchip" data-testid="rerun-of">
			<RotateCwIcon size={12} />
			{#if single && first}
				<span class="faint">Rerun of</span><a href={to(`/runs/${first.run.id}`)}>{what(first)} · {shortRunId(first.run.id)}</a>
			{:else}
				<span class="faint">Rerun</span><span>{runIds.length} runs</span>
			{/if}
		</span>
		<div class="verbs" role="group" aria-label="What to rerun">
			{#if single}
				{#each choices as c (c.mode)}
					<button class:on={mode === c.mode} disabled={!!c.disabledReason} title={c.disabledReason ?? c.help} onclick={() => choose(c.mode)}>{c.label}</button>
				{/each}
			{:else}
				<button class:on={mode === 'all'} onclick={() => choose('all')}>Rerun all</button>
				<button class:on={mode === 'from_failure'} onclick={() => choose('from_failure')}>Rerun from failure</button>
			{/if}
		</div>
		<span class="faint note">Repeats each run's own version and settings, as frozen when it started</span>
	</div>

	{#if loadError}<p class="problem-bar">{loadError}</p>{/if}

	<div class="body">
		<div class="main sb-scroll">
			{#if !details.data}
				<div class="loading" role="status"><RotateCwIcon size={14} class="animate-spin" /> Loading the run…</div>
			{:else if single && first}
				<div class="summary">
					<b>{nodes.length}</b> node{nodes.length === 1 ? '' : 's'}<span>·</span>{runStatusLabel[first.run.status].toLowerCase()} {formatAgo(first.run.endedAt, app.now)}
					<div class="tabs" role="tablist">
						<button role="tab" aria-selected={view === 'nodes'} onclick={() => (view = 'nodes')}>Nodes</button>
						<button role="tab" aria-selected={view === 'graph'} onclick={() => (view = 'graph')}>Graph</button>
					</div>
				</div>
				{#if view === 'graph' && catalog.data}
					{#if drawable.length}<div class="graph"><ScopeGraph keys={drawable} catalog={catalog.data} onselect={() => {}} /></div>{/if}
				{:else}
					<table class="nodes">
						<tbody>
							{#each nodes as key (key)}
								{@const node = byKey.get(key)}
								{@const outcome = first.nodes.find((n) => n.nodeKey === key)?.status}
								<tr>
									<td><a href={to(`/catalog/${key}`)} class="mono">{key.slice(key.indexOf(':') + 1)}</a></td>
									<td class="faint">{node ? kindLine(node) : ''}</td>
									<td class="faint right">{outcome ? `was ${outcome.replace('_', ' ')}` : ''}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			{:else}
				<div class="summary"><b>{ready.length}</b> of {rows.length} runs<span>·</span><b>{ready.reduce((n, r) => n + r.nodes.length, 0)}</b> nodes</div>
				<table class="nodes">
					<tbody>
						{#each rows as r (r.detail.run.id)}
							<tr class:off={!!r.blocked || !r.nodes.length}>
								<td><a href={to(`/runs/${r.detail.run.id}`)}>{what(r.detail)}</a> <span class="faint mono">{shortRunId(r.detail.run.id)}</span></td>
								<td class="faint">{runStatusLabel[r.detail.run.status].toLowerCase()} {formatAgo(r.detail.run.endedAt, app.now)}</td>
								<td class="faint right">{r.blocked ?? (r.nodes.length ? `${r.nodes.length} node${r.nodes.length === 1 ? '' : 's'}` : 'Nothing to rerun')}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
		</div>
		<aside class="rail sb-scroll">
			<h3>Permissions</h3>
			{#each needs as action (action)}
				<p class="decision">
					<span class={app.can(action) ? 'ok-text' : 'bad-text'}>{app.can(action) ? '✓' : '✕'}</span>
					<span title={action}>{PERMISSIONS[action].label}</span>
				</p>
			{/each}
		</aside>
	</div>

	<div class="foot">
		<div class="why">
			{#if launchError}<span class="problem">{launchError}</span>{:else if blockedReason}<span class="faint">{blockedReason}</span>{/if}
		</div>
		<GatedButton requires="run.rerun" variant={canRun ? 'default' : 'outline'} disabled={!canRun} reason={blockedReason} busy={launching} onclick={launch}>▷ Run</GatedButton>
	</div>
</div>

<style>
	.cmd {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 10px 24px;
		border-bottom: 1px solid var(--border);
	}
	.jobchip {
		display: inline-flex;
		flex-shrink: 0;
		align-items: center;
		gap: 6px;
		padding: 4px 9px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--sidebar-accent);
		font-size: 11.5px;
	}
	.jobchip a {
		font-family: var(--font-mono);
		color: var(--foreground);
	}
	.verbs {
		display: flex;
		flex-shrink: 0;
		overflow: hidden;
		border: 1px solid var(--border);
		border-radius: 2px;
	}
	.verbs button {
		padding: 4px 11px;
		border-right: 1px solid var(--border);
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.verbs button:last-child {
		border-right: 0;
	}
	.verbs button.on {
		background: var(--sidebar-accent);
		color: var(--foreground);
	}
	.verbs button:disabled {
		opacity: 0.45;
	}
	.note {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.problem-bar {
		margin: 0;
		padding: 7px 24px;
		border-bottom: 1px solid color-mix(in srgb, var(--sb-error) 35%, var(--border));
		background: color-mix(in srgb, var(--sb-error) 7%, transparent);
		color: var(--sb-error);
		font-size: 12px;
	}
	.body {
		display: flex;
		min-height: 0;
		flex: 1;
	}
	.main {
		display: flex;
		min-width: 0;
		flex: 1;
		flex-direction: column;
		overflow: auto;
		padding: 14px 24px;
	}
	.loading {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.summary {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		margin-bottom: 10px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.summary b {
		color: var(--foreground);
		font-weight: 600;
	}
	.summary span {
		color: var(--sb-text-faint);
	}
	.tabs {
		display: flex;
		gap: 14px;
		margin-left: 18px;
	}
	.tabs button {
		padding: 2px 0;
		border-bottom: 1px solid transparent;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.tabs button[aria-selected='true'] {
		border-bottom-color: var(--foreground);
		color: var(--foreground);
	}
	.graph {
		min-height: 320px;
		flex: 1;
		border: 1px solid var(--border);
		border-radius: 2px;
	}
	.nodes {
		width: 100%;
		border-collapse: collapse;
		font-size: 12.5px;
	}
	.nodes td {
		padding: 5px 12px 5px 0;
		border-bottom: 1px solid var(--border-subtle);
		white-space: nowrap;
	}
	.nodes tr.off {
		opacity: 0.5;
	}
	.right {
		text-align: right;
	}
	.rail {
		width: 280px;
		flex-shrink: 0;
		overflow: auto;
		padding: 14px 18px;
		border-left: 1px solid var(--border);
	}
	.rail h3 {
		margin: 0 0 6px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	.decision {
		display: flex;
		align-items: baseline;
		gap: 8px;
		margin: 3px 0;
		font-size: 12.5px;
	}
	.foot {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 10px 24px;
		border-top: 1px solid var(--border);
		background: var(--card);
	}
	.why {
		min-width: 0;
		flex: 1;
		font-size: 12.5px;
	}
	.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.faint {
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.ok-text {
		color: var(--sb-success);
	}
	.bad-text {
		color: var(--sb-error);
	}
	.problem {
		color: var(--sb-error);
	}
	a:hover {
		color: var(--primary);
	}
</style>
