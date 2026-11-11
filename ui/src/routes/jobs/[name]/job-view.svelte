<script lang="ts">
	import Loading from '$lib/components/loading.svelte';
	import { to } from '$lib/paths';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import type { Trigger } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo, formatDuration, formatStamp } from '$lib/format/time';
	import { formatNodeCounts, nodeOutcomeLabel, nodeOutcomeTone, runOriginLabel, runStatusLabel, runStatusTone } from '$lib/status';
	import { kindLine } from '$lib/build-area/labels';
	import { planHref } from '$lib/run/command';
	import { formatCost, narrowingText, onFailurePlain, overlapPlain, shortRunId, slotLabel } from '$lib/operate/labels';
	import ScopeGraph from '$lib/lineage/scope-graph.svelte';
	import RunPanel from './run-panel.svelte';

	let { name }: { name: string } = $props();

	const job = createResource((client, scope) => client.getJob(scope, name), ['jobs', 'runs', 'snapshot']);
	const jobRuns = createResource((client, scope) => client.listRuns(scope, { jobKey: name, limit: 50 }), ['runs']);
	const triggers = createResource((client, scope) => client.listTriggers(scope), ['triggers', 'ticks']);
	const catalog = createResource((client, scope) => client.listCatalogNodes(scope), ['snapshot', 'runs', 'warehouse']);
	// Each member's nodes and last run, so a group's graph can box every member job around what it builds.
	// Reading `job.data` here reloads it with the job; it stays null until the job has loaded.
	const members = createResource(async (client, scope) => {
		const detail = job.data;
		if (!detail) return null;
		const keys = detail.groupMembers?.map((m) => m.jobKey) ?? [];
		if (!keys.length) return { nodes: new Map<string, string[]>(), lastRuns: new Map() };
		const jobs = await client.listJobs(scope);
		const details = await Promise.all(keys.map((k) => client.getJob(scope, k)));
		return {
			nodes: new Map(keys.map((k, i) => [k, details[i]?.resolvedNodes ?? []])),
			lastRuns: new Map(jobs.map((j) => [j.key, j.lastRun]))
		};
	}, ['jobs', 'runs', 'snapshot']);

	const def = $derived(job.data ?? null);
	const runs = $derived(jobRuns.data ?? []);
	const jobTriggers = $derived((triggers.data ?? []).filter((t) => def?.triggerIds.includes(t.id)));
		const showCost = $derived(runs.some((r) => r.cost !== null));
	const canToggle = $derived(app.can('trigger.toggle'));
	// Two ways to start a run, side by side while one is chosen: the plan page (default) or a panel here (?flow=panel).
	const flow = $derived(page.url.searchParams.get('flow') === 'panel' ? 'panel' : 'review');
	let panelOpen = $state(false);
	function run(keys: string[]) {
		if (flow === 'panel') panelOpen = true;
		else void goto(to(planHref(keys, 'build', { job: name })));
	}
	let tab = $state<'overview' | 'runs' | 'code'>('overview');
	// Nodes picked in the graph, by catalog key. A job runs with or without them; a group's pick runs as an ad-hoc command.
	let picked = $state<string[]>([]);
	let graph = $state<{ clear: () => void }>();
	const byKey = $derived(new Map((catalog.data ?? []).map((n) => [n.key, n])));
	const pickedNodes = $derived(picked.map((k) => byKey.get(k)).filter((n) => !!n));
	const canRunPicked = $derived(app.client.supports('launchAdhocRun'));
	const latest = $derived(runs[0] ?? null);
	// A group draws each member job, in run order, as a box around the nodes it builds.
	const boxes = $derived(
		def?.kind === 'group'
			? [...(def.groupMembers ?? [])].sort((a, b) => a.stage - b.stage).map((m) => ({ jobKey: m.jobKey, nodes: members.data?.nodes.get(m.jobKey) ?? [], lastRun: members.data?.lastRuns.get(m.jobKey) }))
			: []
	);
	let actionError = $state<string | null>(null);

	async function toggleTrigger(t: Trigger) {
		const scope = app.scope;
		if (!scope) return;
		actionError = null;
		try {
			await app.client.setTriggerStates(scope, [t.id], t.state === 'running' ? 'paused' : 'running');
		} catch (e) {
			actionError = e instanceof Error ? e.message : String(e);
		}
	}

	const targetBlocked = $derived(!!def?.targets && !def.targets.includes(app.target?.name ?? ''));
</script>

<AppTopbar title="Jobs">
	{#if picked.length && def?.kind === 'job'}
		<button class="clear" onclick={() => graph?.clear()}>Clear selection</button>
		<GatedButton requires="job.run" variant="outline" onclick={() => goto(to(planHref([], 'build', { job: name, exclude: picked })))}>▷ Run all but {picked.length}</GatedButton>
		<GatedButton requires="job.run" variant="default" onclick={() => run(picked)}>▷ Run {picked.length} selected</GatedButton>
	{:else if picked.length && canRunPicked}
		<button class="clear" onclick={() => graph?.clear()}>Clear selection</button>
		<GatedButton requires="run.adhoc" variant="default" onclick={() => goto(to(planHref(pickedNodes.map((n) => n.key))))}>▷ Run {picked.length} selected</GatedButton>
	{:else}
		<GatedButton
			requires="job.run"
			variant="default"
			disabled={!def || targetBlocked}
			reason={targetBlocked ? `This job only runs on ${def?.targets?.join(', ')}` : undefined}
			onclick={() => run([])}
		>▷ Run</GatedButton>
	{/if}
</AppTopbar>

{#if panelOpen && def}<RunPanel job={def} picked={def.kind === 'job' ? picked : []} onclose={() => (panelOpen = false)} />{/if}

<div class="flex min-h-0 flex-1 flex-col">
	<div class="head">
		<a href={to(`/jobs`)} class="back">← Jobs</a>
		<span class="font-display text-[17px] font-semibold">{name}</span>
		{#if def}
			<span class="chip">{def.kind === 'group' ? `Group${def.members ? ` · ${def.members.length} jobs` : ''}` : 'Job'} · {def.nodeCount} nodes</span>
			{#each jobTriggers as t (t.id)}
				<span class="chip" class:off={t.state === 'paused'}>
					<a href={to(`/triggers/${t.id}`)} title={t.name}>{t.cadence}</a>
					<span title={canToggle ? (t.state === 'running' ? 'Pause' : 'Turn on') : "You don't have permission to pause or turn on schedules and sensors (trigger.toggle)"}>
						<button class="sb-toggle small" class:on={t.state === 'running'} disabled={!canToggle} onclick={() => toggleTrigger(t)} aria-label="{t.state === 'running' ? 'Pause' : 'Turn on'} {t.name}"></button>
					</span>
				</span>
			{/each}
			{#if latest}
				<a class="chip" href={to(`/runs/${latest.id}`)} title={formatStamp(latest.queuedAt)}>
					<i style:background={runStatusTone[latest.status]}></i>Latest run: {runStatusLabel[latest.status]} · {formatAgo(latest.startedAt ?? latest.queuedAt, app.now)}
				</a>
			{/if}
			{#if def.criticalNodes.length}
				<span class="chip" title="If one of these fails, the whole group stops">
					Critical: {#each def.criticalNodes as n, i (n)}{i ? ', ' : ''}<a href={to(`/catalog/${n}`)} class="mono">{n}</a>{/each}
				</span>
			{/if}
		{/if}
	</div>
	{#if job.error}
		<div class="msg err">Couldn't load this job: {job.error}</div>
	{:else if job.data === null}
		<div class="msg">There's no job or group named <code>{name}</code> in the version deployed to {app.location?.name}.</div>
	{/if}
	{#if actionError}<div class="msg err">{actionError}</div>{/if}

	{#if def}
		<nav class="tabs">
			<button class:on={tab === 'overview'} onclick={() => (tab = 'overview')}>Overview</button>
			<button class:on={tab === 'runs'} onclick={() => (tab = 'runs')}>Runs{runs.length ? ` · ${runs.length}` : ''}</button>
			{#if def.declaration}<button class:on={tab === 'code'} onclick={() => (tab = 'code')}>Code</button>{/if}
		</nav>

		{#if tab === 'overview'}
			<div class="flex min-h-0 flex-1">
				<div class="canvas">
					{#if catalog.data && (def.kind !== 'group' || members.data)}
						<ScopeGraph bind:this={graph} keys={def.resolvedNodes} catalog={catalog.data} {boxes} members={def.groupMembers ?? []} onselect={(keys) => (picked = keys)} />
					{:else}
						{#if catalog.error ?? members.error}<p class="faint graph-wait">{catalog.error ?? members.error}</p>{:else}<div class="graph-wait"><Loading /></div>{/if}
					{/if}
				</div>

				<aside class="rail sb-scroll">
					{#if pickedNodes.length === 1}
						{@const n = pickedNodes[0]}
						<p class="label">{kindLine(n)}</p>
						<h2 class="mono">{n.key}</h2>
						<a class="open" href={to(`/catalog/${n.key}`)}>Open in Catalog ↗</a>
						{#if n.description}<p class="desc">{n.description}</p>{/if}
						<dl>
							{#if n.lastOutcome || n.lastBuild}<div><dt>Last build</dt><dd><i style:background={nodeOutcomeTone[n.lastOutcome ?? 'success']}></i>{n.lastOutcome ? nodeOutcomeLabel[n.lastOutcome] : 'Built'}{n.lastBuild ? ` · ${formatAgo(n.lastBuild.builtAt, app.now)}` : ''}</dd></div>{/if}
							{#if n.relation}<div><dt>Writes</dt><dd class="mono">{[n.relation.database, n.relation.schema, n.relation.name].filter(Boolean).join('.')}</dd></div>{/if}
							{#if n.owner.length}<div><dt>Owner</dt><dd>{n.owner.join(', ')}</dd></div>{/if}
							{#if n.path}<div><dt>Defined in</dt><dd class="mono">{n.path}</dd></div>{/if}
						</dl>
					{:else if pickedNodes.length > 1}
						<h2>{pickedNodes.length} nodes selected</h2>
						<ul class="picked">
							{#each pickedNodes as n (n.key)}<li><a href={to(`/catalog/${n.key}`)} class="mono">{n.key}</a></li>{/each}
						</ul>
					{:else}
						{#if def.description}<h3 class="first">Description</h3><p class="desc">{def.description}</p>{/if}
						<dl>
							{#if def.select}<div><dt>Selection</dt><dd class="mono">{def.select}</dd></div>{/if}
							{#if def.exclude.length}<div><dt>Excludes</dt><dd class="mono">{def.exclude.join(' ')}</dd></div>{/if}
							{#if def.overlap}<div><dt>If already running</dt><dd title="overlap = {def.overlap}">{overlapPlain[def.overlap]}</dd></div>{/if}
							{#if def.onFailure}<div><dt>If something fails</dt><dd title="on_failure = {def.onFailure}">{onFailurePlain[def.onFailure]}</dd></div>{/if}
							{#if def.priority !== null}<div><dt>Priority</dt><dd>{def.priority}</dd></div>{/if}
							{#if def.targets}<div><dt>Targets</dt><dd>{def.targets.join(', ')}</dd></div>{/if}
							{#if def.owner}<div><dt>Owner</dt><dd>{def.owner}</dd></div>{/if}
							{#if def.sourceFile}<div><dt>Defined in</dt><dd class="mono">{def.sourceFile}</dd></div>{/if}
							{#if def.memberOf?.length}
								<div><dt>In groups</dt><dd>{#each def.memberOf as g, i (g)}{i ? ', ' : ''}<a href={to(`/jobs/${g}`)}>{g}</a>{/each}</dd></div>
							{/if}
						</dl>
						{#if def.settings && Object.keys(def.settings).length}
							<h3>Options</h3>
							{#each Object.entries(def.settings) as [flag, value] (flag)}
								<p class="setting"><code>{flag}{value === true ? '' : ` ${value}`}</code></p>
							{/each}
						{/if}
						{#if def.concurrency?.length}
							<h3>Shared slots</h3>
							{#each def.concurrency as g (g.key)}
								<p class="setting" title="Concurrency group {g.key}: at most {g.limit} runs at once">
									<span>{g.holders.length} of {g.limit} {slotLabel(g.key)} in use</span>
									{#each g.holders as h (h)}<a class="mono" href={to(`/runs/${h}`)} title={h}>{shortRunId(h)}</a>{/each}
								</p>
							{/each}
						{:else if def.concurrency === null && def.concurrencyGroups.length}
							<h3>Shared slots</h3>
							{#each def.concurrencyGroups as g (g)}<p class="setting" title="Concurrency group {g}">{slotLabel(g)}</p>{/each}
						{/if}
					{/if}
				</aside>
			</div>
		{:else if tab === 'runs'}
			<div class="main sb-scroll">
				{#if runs.length}
					<table class="runs">
						<thead>
							<tr><th>Status</th><th>Run</th><th>Started by</th><th>Nodes</th><th>Took</th><th>Started</th><th>Who</th>{#if showCost}<th class="num">Cost</th>{/if}</tr>
						</thead>
						<tbody>
							{#each runs as r (r.id)}
								<tr onclick={() => goto(to(`/runs/${r.id}`))}>
									<td><i style:background={runStatusTone[r.status]}></i>{runStatusLabel[r.status]}</td>
									<td><a href={to(`/runs/${r.id}`)} class="mono" title={r.id}>{shortRunId(r.id)}</a>{#if r.narrowing}<span class="part" title="Ran part of the job: sqb build {narrowingText(r.narrowing)}">part</span>{/if}</td>
									<td class="faint">{runOriginLabel[r.origin]}{r.parentRunId ? ` in group run ${shortRunId(r.parentRunId)}` : r.triggerName ? ` · ${r.triggerName}` : ''}</td>
									<td class="faint">{formatNodeCounts(r.nodeCounts)}</td>
									<td class="faint">{r.startedAt ? formatDuration(r.startedAt, r.endedAt, app.now) : '—'}</td>
									<td class="faint" title={formatStamp(r.queuedAt)}>{formatAgo(r.startedAt ?? r.queuedAt, app.now)}</td>
									<td class="faint" title={r.requestedBy.name}>{r.requestedBy.displayName}</td>
									{#if showCost}<td class="faint num" title={r.cost ? `Estimated warehouse cost: ${r.cost.source}` : 'No cost reported'}>{formatCost(r.cost)}</td>{/if}
								</tr>
							{/each}
						</tbody>
					</table>
				{:else}
					{#if jobRuns.data === undefined && !jobRuns.error}<Loading />{:else}<p class="faint">No runs yet.</p>{/if}
				{/if}
			</div>
		{:else if def.declaration}
			<div class="main sb-scroll">
				<CodePreview file="{def.declaration.sourceFile}:{def.declaration.line}" code={def.declaration.code} language="text" maxLines={200} />
			</div>
		{/if}
	{/if}
</div>

<style>
	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px 10px;
		padding: 10px 24px;
		font-size: 12.5px;
	}
	.back {
		margin-right: 2px;
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.chip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		height: 24px;
		padding: 0 8px;
		border: 1px solid var(--border);
		border-radius: 3px;
		background: var(--card);
		color: var(--muted-foreground);
		font-size: 12px;
		white-space: nowrap;
	}
	a.chip:hover,
	.chip a:hover {
		color: var(--primary);
	}
	.chip.off {
		opacity: 0.7;
	}
	.chip i,
	dd i,
	.runs td .part {
		margin-left: 8px;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.runs td i {
		display: inline-block;
		width: 7px;
		height: 7px;
		margin-right: 6px;
		border-radius: 50%;
	}
	.chip i {
		margin-right: 0;
	}
	.tabs {
		display: flex;
		gap: 18px;
		padding: 0 24px;
		border-bottom: 1px solid var(--border);
	}
	.tabs button {
		padding: 8px 0 7px;
		border-bottom: 2px solid transparent;
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.tabs button.on {
		border-color: var(--primary);
		color: var(--foreground);
	}
	.clear {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.clear:hover {
		color: var(--foreground);
	}
	.faint {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.msg {
		padding: 14px 24px;
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.err {
		color: var(--sb-error);
	}
	.canvas {
		position: relative;
		min-width: 0;
		flex: 1;
	}
	.graph-wait {
		height: 100%;
		padding: 14px 24px;
		background: var(--sb-inset);
	}
	.main {
		min-width: 0;
		flex: 1;
		overflow: auto;
		padding: 12px 24px;
	}
	.rail {
		width: 280px;
		flex-shrink: 0;
		overflow: auto;
		padding: 16px 18px;
		border-left: 1px solid var(--border);
		font-size: 12.5px;
	}
	.rail h2 {
		margin-bottom: 4px;
		font-size: 13.5px;
		font-weight: 600;
		word-break: break-all;
	}
	.rail h3 {
		margin: 16px 0 4px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	.rail h3.first {
		margin-top: 0;
	}
	.runs th {
		padding: 6px 12px 6px 0;
		border-bottom: 1px solid var(--border);
		color: var(--sb-text-faint);
		font-size: 11.5px;
		font-weight: 500;
		text-align: left;
	}
	.runs thead tr {
		cursor: default;
	}
	.runs thead tr:hover {
		background: none;
	}
	.label {
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
	.open {
		display: inline-block;
		margin-bottom: 10px;
		color: var(--primary);
		font-size: 12px;
	}
	.desc {
		margin-bottom: 12px;
		line-height: 1.5;
	}
	dl div {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		padding: 3px 0;
	}
	dt {
		flex-shrink: 0;
		color: var(--muted-foreground);
	}
	dd {
		min-width: 0;
		overflow-wrap: anywhere;
		text-align: right;
	}
	.picked li {
		padding: 2px 0;
	}
	code,
	.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.setting {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 8px;
		padding: 2px 0;
		font-size: 12px;
	}
	.runs {
		width: 100%;
		border-collapse: collapse;
		font-size: 12.5px;
	}
	.runs tr {
		cursor: pointer;
	}
	.runs tr:hover {
		background: var(--sb-hover);
	}
	.runs td {
		padding: 7px 12px 7px 0;
		border-bottom: 1px solid var(--border-subtle);
		white-space: nowrap;
	}
	.num {
		text-align: right;
	}
	a:hover {
		color: var(--primary);
	}
	.sb-scroll {
		scrollbar-width: thin;
		scrollbar-color: var(--border-strong) transparent;
	}
</style>
