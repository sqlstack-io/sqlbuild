<script lang="ts">
	import { noun, plural } from '$lib/format/plural';
	import HelpTip from '$lib/components/help-tip.svelte';
	import PageBody from '$lib/components/page-body.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import { to } from '$lib/paths';
	import PlayIcon from '@lucide/svelte/icons/play';
	// Every rerun opens here first, like every Run button opens the plan page: nothing reruns until Run is
	// pressed. One run's rerun is the selection in the box, run as `sqb build <selection>` against the current
	// publication; reruns of several runs repeat each run's frozen version and settings.
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
	import { rerunChoices, runFailures } from '../../runs/[id]/utils';
	import { rerunRows } from '../../runs/[id]/utils/rerun-rows';
	import SelectionBar from '$lib/components/selection/selection-bar.svelte';
	import { createSelection } from '$lib/state/selection.svelte';
	import type { SuggestionItem } from '$lib/selection/suggest';
	import { nodeLabel } from '$lib/operate/labels';
	import { buildRequest, copyCommand, runsNamed } from '$lib/selection/command';
	import { namesRun, openingText, presetSuggestions, rerunItems, type RerunItem } from './utils/rerun-plan';

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
	// Reruns of several runs rerun all or from failure; any other mode in the address falls back to all.
	let mode = $state<RerunMode>(requested === 'from_failure' ? 'from_failure' : 'all');
	// One run: what it failed on, the presets (the run page's Rerun menu), and an editable selection they fill.
	const runEvents = createResource((client, scope) => (single && client.supports('listRunEvents') ? client.listRunEvents(scope, runIds[0], { limit: 5000 }).then((p) => p.events) : Promise.resolve([])), runIds.map((id) => `run:${id}`));
	const runStatements = createResource((client, scope) => (single && client.supports('listRunStatements') ? client.listRunStatements(scope, runIds[0]) : Promise.resolve([])), runIds.map((id) => `run:${id}`));
	const failures = $derived(first ? runFailures(first.nodes, runEvents.data ?? [], runStatements.data ?? [], first.run.target, { node: () => null, audit: () => null }) : []);
	const presets = $derived(first ? rerunRows({ failures, unfinished: first.rerunFromFailureNodes, total: first.nodes.length, jobKey: first.run.jobKey }) : []);
	const selection = createSelection({ initial: { select: '', exclude: '' } });
	let filled = false;
	$effect(() => {
		if (filled || !first || !runEvents.data) return;
		filled = true;
		selection.select = openingText(requested, picked, first.run.id, failures);
	});
	const items = $derived<RerunItem[]>(first && selection.keys ? rerunItems([...selection.keys], first, failures) : []);
	/** What Run submits for one run, and the command shown for it: the same tokens. */
	const typed = $derived({ select: selection.select, exclude: selection.exclude });
	const command = $derived(copyCommand(typed));
	const auditCount = $derived(items.filter((i) => i.kind === 'audit').length);
	const nodeCount = $derived(items.length - auditCount);
	/** Where the plan came from, read from the selection alone: the runs its `run:`, `failed:` and `skipped:` name. */
	const fromRuns = $derived(runsNamed(selection.select));
	const nodesOf = (d: RunDetail): string[] => (mode === 'from_failure' ? d.rerunFromFailureNodes : d.selection);
	const blockedOf = (d: RunDetail): string | null => rerunChoices(d, [], can).find((c) => c.mode === mode)?.disabledReason ?? null;
	const rows = $derived((details.data ?? []).filter((d) => !done.has(d.run.id)).map((d) => ({ detail: d, nodes: nodesOf(d), blocked: blockedOf(d) })));
	const ready = $derived(rows.filter((r) => !r.blocked && r.nodes.length));
	const nodes = $derived(items.filter((i) => i.kind === 'node').map((i) => i.key));
	const needs = $derived<Capability[]>(single ? ['run.adhoc'] : ['run.rerun', ...((details.data ?? []).some((d) => d.run.rewriteHistory) ? (['run.rewrite_history'] as const) : [])]);
	let view = $state<'nodes' | 'graph'>('nodes');
	const drawable = $derived(nodes.filter((k) => byKey.has(k)));

	function choose(next: RerunMode) {
		mode = next;
		void goto(to(rerunHref(remaining, next)), { replaceState: true, keepFocus: true, noScroll: true });
	}

	const loadError = $derived(details.error ? String(details.error) : details.data && !details.data.length ? 'These runs were not found in this location.' : null);
	const singleBlocked = $derived(
		!details.data ? 'Loading the run' : selection.problem ? 'Fix the selection first' : !selection.select.trim() ? 'Select what to rerun' : !selection.evaluation || selection.evaluating ? 'Resolving the selection' : !items.length ? 'Nothing to rerun' : undefined
	);
	const blockedReason = $derived(
		single
			? singleBlocked
			: !details.data
				? 'Loading the runs'
				: !ready.length
					? (rows.find((r) => r.blocked)?.blocked ?? 'Nothing to rerun')
					: rows.length > ready.length
						? `${rows.length - ready.length} of these runs can't be rerun this way`
						: undefined
	);
	const canRun = $derived(single ? !blockedReason : ready.length > 0);

	let launching = $state(false);
	let launchError = $state<string | null>(null);
	async function launch() {
		const scope = app.scope;
		if (!scope || !canRun) return;
		launching = true;
		launchError = null;
		// One run: the box's selection, as the command shown beside Run.
		if (single) {
			try {
				const run = await app.client.launchAdhocRun(scope, buildRequest(typed));
				selection.remember();
				await goto(to(`/runs/${run.id}`));
			} catch (error) {
				launchError = error instanceof Error ? error.message : String(error);
			} finally {
				launching = false;
			}
			return;
		}
		const outcomes = await Promise.allSettled(
			ready.map(({ detail }) => app.client.rerunRun(scope, detail.run.id, mode as Exclude<RerunMode, 'selected'>))
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
				await goto(to(rerunHref(remaining, mode)), { replaceState: true, keepFocus: true, noScroll: true });
			}
			return;
		}
		await goto(to(started.length === 1 ? `/runs/${started[0].id}` : '/runs'));
	}

	const what = (d: RunDetail) => d.run.jobKey ?? 'ad-hoc run';
	/** The presets, then the run selectors, the box suggests beside the usual ones; each only writes text. */
	const runSuggestions = (id: string): SuggestionItem[] => [
		...presetSuggestions(presets, id),
		{ insert: `skipped:${id}`, type: 'skipped', label: id, hint: `what run ${shortRunId(id)} skipped`, complete: true }
	];
	const nodeColumns: TableColumn[] = [
		{ id: 'node', label: 'Node or audit', minWidth: 200 },
		{ id: 'kind', label: 'Kind', width: '30%' },
		{ id: 'last', label: 'Last result', width: '160px' }
	];
	const runColumns: TableColumn[] = [
		{ id: 'run', label: 'Run', minWidth: 220 },
		{ id: 'status', label: 'Ended', width: '200px' },
		{ id: 'rerun', label: 'Reruns', width: '30%' }
	];
</script>

<AppTopbar title="Plan" />

<div class="flex min-h-0 flex-1 flex-col">
	{#if single && first}
		<!-- The box is the whole plan: Rerun opens it on failed:<run>+, and the suggestions only rewrite its text. -->
		<div class="pick">
			<SelectionBar {selection} names={first.nodes.map((n) => ({ value: nodeLabel(n.nodeKey), hint: n.kind ?? '' }))} total={first.nodes.length} build={false} runs={runSuggestions(first.run.id)} />
		</div>
	{:else}
	<div class="cmd">
			<span class="jobchip" data-testid="rerun-of">
				<RotateCwIcon size={12} />
				<span class="faint">Rerun</span><span>{remaining.length} runs</span>
			</span>
			<SegmentedFilter
				label="What to rerun"
				value={mode}
				options={[
					{ value: 'all' as RerunMode, label: 'Rerun all' },
					{ value: 'from_failure' as RerunMode, label: 'Rerun from failure' }
				]}
				onselect={choose}
			/>
			<HelpTip text="Repeats each run's own version and settings, as frozen when it started" label="About rerunning" />
		</div>

	{/if}

	{#if loadError}<p class="problem-bar">{loadError}</p>{/if}

	<div class="body">
		<PageBody padding="14px 24px" inner="flex min-h-full flex-col">
			{#if !details.data}
				<div class="loading" role="status"><RotateCwIcon size={14} class="animate-spin" /> Loading the run…</div>
			{:else if single && first}
				<div class="summary">
					{#if nodeCount || !auditCount}<b>{nodeCount}</b> {noun(nodeCount, 'node')}{/if}{#if nodeCount && auditCount}<span>·</span>{/if}{#if auditCount}<b>{auditCount}</b> {noun(auditCount, 'audit')}{/if}{#each fromRuns as written (written)}{@const known = namesRun(written, first.run.id)}<span>·</span><span data-testid="rerun-from">from run {#if known}<a class="mono" href={to(`/runs/${first.run.id}`)}>{shortRunId(first.run.id)}</a> <span class="faint">({what(first)}, {runStatusLabel[first.run.status].toLowerCase()} {formatAgo(first.run.endedAt, app.now)})</span>{:else}<span class="mono">{written}</span>{/if}</span>{/each}
					<div class="tabs"><SegmentedFilter label="View" bind:value={view} options={[{ value: 'nodes', label: 'List' }, { value: 'graph', label: 'Graph' }]} /></div>
				</div>
				{#if view === 'graph' && catalog.data}
					{#if drawable.length}<div class="graph"><ScopeGraph keys={drawable} catalog={catalog.data} onselect={() => {}} /></div>{/if}
				{:else}
					<DataTable class="static compact" embedded columns={nodeColumns} rows={items} key={(i) => i.key} rowHeight={32} empty="Nothing to rerun.">
						{#snippet row(item)}
							{@const node = byKey.get(item.key)}
							<tr>
								<td>{#if item.kind === 'audit'}<a href={to(`/checks?q=${item.name}`)} class="mono">{item.name}</a>{:else}<a href={to(`/catalog/${item.key}`)} class="mono">{item.name}</a>{/if}</td>
								<td class="faint">{item.kind === 'audit' ? `Audit${item.where ? ` · ${item.where}` : ''}` : node ? kindLine(node) : ''}</td>
								<td class:bad-text={item.last === 'failed'} class:faint={item.last !== 'failed'}>{item.last === 'success' ? 'built' : item.last ? item.last.replace('_', ' ') : '—'}</td>
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
			{#if launchError}<span class="problem">{launchError}</span>{:else if blockedReason}<span class="faint">{blockedReason}</span>{:else if single}<code class="mono" data-testid="rerun-command">{command}</code>{/if}
		</div>
		<GatedButton requires={single ? 'run.adhoc' : 'run.rerun'} variant={canRun ? 'default' : 'outline'} disabled={!canRun} reason={blockedReason} busy={launching} onclick={launch}><PlayIcon class="size-3.5 fill-current stroke-none" />Run</GatedButton>
	</div>
</div>

<style>
	.mono {
		font-family: var(--font-mono);
		font-size: 12.5px;
	}
	.pick {
		padding: 4px 14px;
		border-bottom: 1px solid var(--border-subtle);
	}
	.pick :global(.selection-bar) {
		border-bottom: none;
		background: transparent;
	}
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
