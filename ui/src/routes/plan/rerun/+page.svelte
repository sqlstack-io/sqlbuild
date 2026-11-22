<script lang="ts">
	import { noun, plural } from '$lib/format/plural';
	import HelpTip from '$lib/components/help-tip.svelte';
	import PageBody from '$lib/components/page-body.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import { to } from '$lib/paths';
	import PlayIcon from '@lucide/svelte/icons/play';
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
	import { useCatalogNodes } from '$lib/state/catalog.svelte';
	import { runStatusLabel } from '$lib/status';
	import { rerunChoices } from '../../runs/[id]/utils';

	const runIds = page.url.searchParams.getAll('run');
	const picked = page.url.searchParams.getAll('node');
	const requested = (page.url.searchParams.get('mode') ?? 'all') as RerunMode;
	const single = runIds.length === 1;
	// Runs whose rerun already started; the page drops them so pressing Run again never repeats one.
	let done = $state(new Set<string>());
	const remaining = $derived(runIds.filter((id) => !done.has(id)));

	const details = createResource(async (client, scope) => (await Promise.all(runIds.map((id) => client.getRunDetail(scope, id)))).filter((d): d is RunDetail => !!d), runIds.map((id) => `run:${id}`));
	const catalog = useCatalogNodes();
	const byKey = $derived(new Map((catalog.data ?? []).map((n) => [n.key, n])));

	const can = (c: Capability) => app.can(c);
	const first = $derived(details.data?.[0] ?? null);
	/** One run's choices, as its rerun button offers them; reruns of several runs only rerun all or from failure. */
	const choices = $derived(single && first ? rerunChoices(first, app.client.supports('rerunSelectedNodes') ? picked : [], can) : []);
	const offered = (m: RerunMode) => (single ? choices.some((c) => c.mode === m && !c.disabledReason) : m === 'all' || m === 'from_failure');
	let mode = $state<RerunMode>(requested);
	// A mode this run can't rerun, typed into the address, falls back to rerunning all.
	$effect(() => {
		if (details.data && !offered(mode)) mode = 'all';
	});
	const nodesOf = (d: RunDetail): string[] => (mode === 'selected' ? picked : mode === 'from_failure' ? d.rerunFromFailureNodes : d.selection);
	const blockedOf = (d: RunDetail): string | null => rerunChoices(d, [], can).find((c) => c.mode === (mode === 'selected' ? 'all' : mode))?.disabledReason ?? null;
	const rows = $derived((details.data ?? []).filter((d) => !done.has(d.run.id)).map((d) => ({ detail: d, nodes: nodesOf(d), blocked: blockedOf(d) })));
	const ready = $derived(rows.filter((r) => !r.blocked && r.nodes.length));
	const nodes = $derived(single && rows[0] ? rows[0].nodes : []);
	const needs = $derived<Capability[]>(['run.rerun', ...((details.data ?? []).some((d) => d.run.rewriteHistory) ? (['run.rewrite_history'] as const) : [])]);
	let view = $state<'nodes' | 'graph'>('nodes');
	const drawable = $derived(nodes.filter((k) => byKey.has(k)));

	function choose(next: RerunMode) {
		mode = next;
		void goto(to(rerunHref(remaining, next, next === 'selected' ? picked : [])), { replaceState: true, keepFocus: true, noScroll: true });
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
		const failed = ready.filter((_, i) => outcomes[i].status === 'rejected').map((r) => r.detail.run.id);
		const startedFrom = ready.filter((_, i) => outcomes[i].status === 'fulfilled').map((r) => r.detail.run.id);
		const started = outcomes.flatMap((o) => (o.status === 'fulfilled' ? [o.value] : []));
		if (failed.length) {
			const first = outcomes.find((o): o is PromiseRejectedResult => o.status === 'rejected')?.reason;
			launchError = `${started.length ? `Started ${started.length}; ` : ''}${failed.length} failed: ${first instanceof Error ? first.message : String(first)}`;
			// Only the failed ones stay, here and in the address, so pressing Run again never repeats a rerun that started.
			if (started.length) {
				done = new Set([...done, ...startedFrom]);
				await goto(to(rerunHref(remaining, mode, mode === 'selected' ? picked : [])), { replaceState: true, keepFocus: true, noScroll: true });
			}
			return;
		}
		await goto(to(started.length === 1 ? `/runs/${started[0].id}` : '/runs'));
	}

	const what = (d: RunDetail) => d.run.jobKey ?? 'ad-hoc run';
	const nodeColumns: TableColumn[] = [
		{ id: 'node', label: 'Node', minWidth: 200 },
		{ id: 'kind', label: 'Kind', width: '30%' },
		{ id: 'last', label: 'Last time', width: '160px' }
	];
	const runColumns: TableColumn[] = [
		{ id: 'run', label: 'Run', minWidth: 220 },
		{ id: 'status', label: 'Ended', width: '200px' },
		{ id: 'rerun', label: 'Reruns', width: '30%' }
	];
</script>

<AppTopbar title="Plan" />

<div class="flex min-h-0 flex-1 flex-col">
	<div class="cmd">
		<span class="jobchip" data-testid="rerun-of">
			<RotateCwIcon size={12} />
			{#if single && first}
				<span class="faint">Rerun of</span><a href={to(`/runs/${first.run.id}`)}>{what(first)} · {shortRunId(first.run.id)}</a>
			{:else}
				<span class="faint">Rerun</span><span>{remaining.length} runs</span>
			{/if}
		</span>
		<SegmentedFilter
			label="What to rerun"
			value={mode}
			options={single
				? choices.map((c) => ({ value: c.mode, label: c.label, title: c.disabledReason ?? c.help, disabled: !!c.disabledReason }))
				: [
						{ value: 'all' as RerunMode, label: 'Rerun all' },
						{ value: 'from_failure' as RerunMode, label: 'Rerun from failure' }
					]}
			onselect={choose}
		/>
		<HelpTip text="Repeats each run's own version and settings, as frozen when it started" label="About rerunning" />
	</div>

	{#if loadError}<p class="problem-bar">{loadError}</p>{/if}

	<div class="body">
		<PageBody padding="14px 24px" inner="flex min-h-full flex-col">
			{#if !details.data}
				<div class="loading" role="status"><RotateCwIcon size={14} class="animate-spin" /> Loading the run…</div>
			{:else if single && first}
				<div class="summary">
					<b>{nodes.length}</b> {noun(nodes.length, 'node')}<span>·</span>{runStatusLabel[first.run.status].toLowerCase()} {formatAgo(first.run.endedAt, app.now)}
					<div class="tabs"><SegmentedFilter label="View" bind:value={view} options={[{ value: 'nodes', label: 'Nodes' }, { value: 'graph', label: 'Graph' }]} /></div>
				</div>
				{#if view === 'graph' && catalog.data}
					{#if drawable.length}<div class="graph"><ScopeGraph keys={drawable} catalog={catalog.data} onselect={() => {}} /></div>{/if}
				{:else}
					<DataTable class="static compact" embedded columns={nodeColumns} rows={nodes} key={(key) => key} rowHeight={32} empty="Nothing to rerun.">
						{#snippet row(key)}
							{@const node = byKey.get(key)}
							{@const outcome = first.nodes.find((n) => n.nodeKey === key)?.status}
							<tr>
								<td><a href={to(`/catalog/${key}`)} class="mono">{key.slice(key.indexOf(':') + 1)}</a></td>
								<td class="faint">{node ? kindLine(node) : ''}</td>
								<td class="faint">{outcome ? `was ${outcome.replace('_', ' ')}` : ''}</td>
							</tr>
						{/snippet}
					</DataTable>
				{/if}
			{:else}
				<div class="summary"><b>{ready.length}</b> of {rows.length} runs<span>·</span><b>{ready.reduce((n, r) => n + r.nodes.length, 0)}</b> nodes</div>
				<DataTable class="static compact" embedded columns={runColumns} rows={rows} key={(r) => r.detail.run.id} empty="No runs to rerun.">
					{#snippet row(r)}
						<tr class:off={!!r.blocked || !r.nodes.length}>
							<td><a href={to(`/runs/${r.detail.run.id}`)}>{what(r.detail)}</a> <span class="faint mono">{shortRunId(r.detail.run.id)}</span></td>
							<td class="faint">{runStatusLabel[r.detail.run.status].toLowerCase()} {formatAgo(r.detail.run.endedAt, app.now)}</td>
							<td class="faint">{r.blocked ?? (r.nodes.length ? `${plural(r.nodes.length, 'node')}` : 'Nothing to rerun')}</td>
						</tr>
					{/snippet}
				</DataTable>
			{/if}
		</PageBody>
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
		<GatedButton requires="run.rerun" variant={canRun ? 'default' : 'outline'} disabled={!canRun} reason={blockedReason} busy={launching} onclick={launch}><PlayIcon class="size-3.5 fill-current stroke-none" />Run</GatedButton>
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
		margin-left: 12px;
	}
	/* One grid row that fills the box, so the graph's height: 100% has a definite height to fill; in a plain block it collapsed to its toolbar. */
	.graph {
		display: grid;
		grid-template-rows: minmax(0, 1fr);
		min-height: 320px;
		flex: 1;
		border: 1px solid var(--border);
		border-radius: 2px;
	}
	tr.off {
		opacity: 0.5;
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
