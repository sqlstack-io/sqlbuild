<script lang="ts">
	import { plural } from '$lib/format/plural';
	import PageBody from '$lib/components/page-body.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Button from '$lib/components/ui/button/button.svelte';
	import PageTabs from '$lib/components/platform/page-tabs.svelte';
	import Loading from '$lib/components/loading.svelte';
	import PlayIcon from '@lucide/svelte/icons/play';
	import PanelRightIcon from '@lucide/svelte/icons/panel-right';
	import { createDetailsPanel } from '$lib/state/details-panel.svelte';
	import { openRow, to } from '$lib/paths';
	import { goto } from '$app/navigation';
	import type { Trigger } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { useCatalogNodes } from '$lib/state/catalog.svelte';
	import { formatAgo, formatDuration, formatIn, formatStamp } from '$lib/format/time';
	import { formatNodeCounts, nodeOutcomeLabel, nodeOutcomeTone, runOriginLabel, runStatusLabel, runStatusTone } from '$lib/status';
	import { kindLine, shortName } from '$lib/build-area/labels';
	import { planHref } from '$lib/run/command';
	import { formatCost, narrowingText, onFailureText, overlapHelp, overlapPlain, shortRunId, slotLabel } from '$lib/operate/labels';
	import ScopeGraph from '$lib/lineage/scope-graph.svelte';
	import MemberGraph from '$lib/lineage/member-graph.svelte';
	import { memberLinks, waitReasons, type MemberInfo, type MemberSelection } from '$lib/lineage/member-graph';
	import InspectorHead from '$lib/components/lineage/inspector-head.svelte';
	import InspectorSection from '$lib/components/lineage/inspector-section.svelte';
	import OutcomeStrip from '$lib/components/build-area/outcome-strip.svelte';
	import { formatMs } from '$lib/build-area/labels';

	let { name }: { name: string } = $props();

	const job = createResource((client, scope) => client.getJob(scope, name), ['jobs', 'runs', 'snapshot']);
	const jobRuns = createResource((client, scope) => client.listRuns(scope, { jobKey: name, limit: 50 }), ['runs']);
	const triggers = createResource((client, scope) => client.listTriggers(scope), ['triggers', 'ticks']);
	const catalog = useCatalogNodes();
	// A group's members as the jobs list reports them: node count and last run, for the member graph.
	// Reading `job.data` here reloads it with the job; it stays null for a plain job or until the job has loaded.
	const memberJobs = createResource(async (client, scope) => {
		if (job.data?.kind !== 'group') return null;
		const jobs = await client.listJobs(scope);
		return new Map<string, MemberInfo>(jobs.map((j) => [j.key, { nodeCount: j.nodeCount, lastRun: j.lastRun, description: j.description, nodes: j.nodes }]));
	}, ['jobs', 'runs', 'snapshot']);

	const def = $derived(job.data ?? null);
	// Why each member waits for another, node by node, from the catalog's lineage.
	const links = $derived(def?.groupMembers && memberJobs.data && catalog.data ? memberLinks(def.groupMembers, memberJobs.data, catalog.data) : new Map());
	const runs = $derived(jobRuns.data ?? []);
	const jobTriggers = $derived((triggers.data ?? []).filter((t) => def?.triggerIds.includes(t.id)));
	const showCost = $derived(runs.some((r) => r.cost !== null));
	const canToggle = $derived(app.can('trigger.toggle'));
	// A run is always reviewed on the plan page before it starts.
	function run(keys: string[]) {
		void goto(to(planHref(keys, 'build', { job: name })));
	}
	let tab = $state<'overview' | 'runs' | 'code'>('overview');
	// Nodes picked in the graph, by catalog key. A job runs with or without them; a group's pick runs as an ad-hoc command.
	let picked = $state<string[]>([]);
	let graph = $state<{ clear: () => void }>();
	// On a group's graph: the member job, or one of its nodes, selected.
	let memberSelection = $state<MemberSelection | null>(null);
	$effect(() => {
		if (def?.kind === 'group') picked = memberSelection?.node ? [memberSelection.node] : [];
	});
	function clearSelection() {
		graph?.clear();
		memberSelection = null;
	}
	// A group's sidebar works as Lineage's: closed until something is selected, then as the person last left it.
	const details = createDetailsPanel('job-group', () => !!memberSelection);
	const railOpen = $derived(def?.kind !== 'group' || details.open);
	const selectedMember = $derived(memberSelection && !memberSelection.node ? memberSelection.member : null);
	const memberFacts = $derived(selectedMember ? (memberJobs.data?.get(selectedMember) ?? null) : null);
	const memberWaits = $derived(selectedMember && def?.groupMembers ? waitReasons(selectedMember, def.groupMembers, links) : []);
	const byKey = $derived(new Map((catalog.data ?? []).map((n) => [n.key, n])));
	const pickedNodes = $derived(picked.map((k) => byKey.get(k)).filter((n) => !!n));
	const canRunPicked = $derived(app.client.supports('launchAdhocRun'));
	const latest = $derived(runs[0] ?? null);
	// Recent finished runs, oldest first, for the sidebar's strip and typical duration.
	const finished = $derived(runs.filter((r) => r.endedAt).slice(0, 20).reverse());
	const succeeded = $derived(finished.filter((r) => r.status === 'success').length);
	const typicalMs = $derived.by(() => {
		const took = finished.flatMap((r) => (r.startedAt && r.endedAt ? [Date.parse(r.endedAt) - Date.parse(r.startedAt)] : [])).sort((a, b) => a - b);
		return took.length ? took[Math.floor(took.length / 2)] : null;
	});
	const hasSettings = $derived(!!def && (!!def.overlap || !!def.onFailure || def.priority !== null || !!def.targets || !!(def.settings && Object.keys(def.settings).length) || !!def.concurrency?.length || (def.concurrency === null && def.concurrencyGroups.length > 0)));
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
	const runColumns = $derived<TableColumn[]>([
		{ id: 'status', label: 'Status', width: '140px' },
		{ id: 'run', label: 'Run', width: '150px' },
		{ id: 'by', label: 'Started by', minWidth: 200 },
		{ id: 'nodes', label: 'Nodes', width: '22%' },
		{ id: 'took', label: 'Took', width: '90px' },
		{ id: 'started', label: 'Started', width: '120px' },
		{ id: 'who', label: 'Who', width: '140px' },
		...(showCost ? [{ id: 'cost', label: 'Cost', width: '90px', align: 'right' as const }] : [])
	]);
</script>

<AppTopbar title="Jobs">
	{#if def?.kind === 'group'}
		<Button variant="ghost" size="icon-bar" aria-pressed={details.open} onclick={details.toggle} title={details.open ? 'Hide details' : 'Show details'} aria-label="Toggle details"><PanelRightIcon /></Button>
	{/if}
	{#if picked.length && def?.kind === 'job'}
		<Button variant="ghost" onclick={clearSelection}>Clear selection</Button>
		<GatedButton size="bar" requires="job.run" variant="outline" onclick={() => goto(to(planHref([], 'build', { job: name, exclude: picked })))}><PlayIcon class="size-3.5 fill-current stroke-none" />Run all but {picked.length}</GatedButton>
		<GatedButton size="bar" requires="job.run" variant="default" onclick={() => run(picked)}><PlayIcon class="size-3.5 fill-current stroke-none" />Run {picked.length} selected</GatedButton>
	{:else if picked.length && canRunPicked}
		<Button variant="ghost" onclick={clearSelection}>Clear selection</Button>
		<GatedButton size="bar" requires="run.adhoc" variant="default" onclick={() => goto(to(planHref(pickedNodes.map((n) => n.key))))}><PlayIcon class="size-3.5 fill-current stroke-none" />Run {picked.length} selected</GatedButton>
	{:else}
		<GatedButton
			size="bar"
			requires="job.run"
			variant="default"
			disabled={!def || targetBlocked}
			reason={targetBlocked ? `This job only runs on ${def?.targets?.join(', ')}` : undefined}
			onclick={() => run([])}
		><PlayIcon class="size-3.5 fill-current stroke-none" />Run</GatedButton>
	{/if}
</AppTopbar>


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
		<PageTabs bind:value={tab} tabs={[{ value: 'overview', label: 'Overview' }, { value: 'runs', label: 'Runs', count: runs.length || undefined }, ...(def.declaration ? [{ value: 'code' as const, label: 'Code' }] : [])]} />

		{#if tab === 'overview'}
			<div class="flex min-h-0 flex-1">
				<div class="canvas">
					{#if def.kind === 'group' && !def.groupMembers?.length}
						<p class="faint graph-wait">{def.groupMembers ? 'This group has no member jobs.' : "The server doesn't report this group's member jobs."}</p>
					{:else if def.kind === 'group'}
						<!-- A group draws its member jobs; each member's page draws that job's nodes. -->
						<MemberGraph members={def.groupMembers ?? []} info={memberJobs.data ?? new Map()} catalog={catalog.data ?? []} {links} bind:selection={memberSelection} />
					{:else if catalog.data}
						<ScopeGraph bind:this={graph} keys={def.resolvedNodes} catalog={catalog.data} onselect={(keys) => (picked = keys)} />
					{:else}
						{#if catalog.error}<p class="faint graph-wait">{catalog.error}</p>{:else}<div class="graph-wait"><Loading /></div>{/if}
					{/if}
				</div>

				{#if railOpen}
				<aside class="rail sb-scroll" class:sections={!pickedNodes.length}>
					{#if selectedMember}
						<InspectorHead title={selectedMember} href={to(`/jobs/${selectedMember}`)} linkLabel="Open job ↗" linkTitle="Open the page of {selectedMember}">
							{#snippet meta()}
								<span>Job in {name}</span>
								{#if memberFacts?.nodeCount !== null && memberFacts?.nodeCount !== undefined}<span>{plural(memberFacts.nodeCount, 'node')}</span>{/if}
							{/snippet}
						</InspectorHead>
						<InspectorSection title="Description">
							{#if memberFacts?.description}<p class="about-text">{memberFacts.description}</p>{:else}<p class="faint">No description.</p>{/if}
						</InspectorSection>
						<InspectorSection title="Waits for">
							{#if memberWaits.length}
								{#each memberWaits as w (w.upstream)}
									<div class="wait" class:implied={w.through !== undefined}>
										{#if w.through !== undefined}<span class="faint">Also after</span>{/if}
										<a class="mono" href={to(`/jobs/${w.upstream}`)}>{w.upstream}</a>
										{#if w.through !== undefined}
											{#if w.through}<span class="faint">(through <a class="mono" href={to(`/jobs/${w.through}`)}>{w.through}</a>)</span>{/if}
										{:else if w.reasons.length}
											<ul class="reasons">
												{#each w.reasons as r (`${r.from}>${r.to}`)}
													<li><a class="mono" href={to(`/catalog/${r.to}`)} title={r.to}>{shortName(r.to)}</a> reads <a class="mono" href={to(`/catalog/${r.from}`)} title={r.from}>{shortName(r.from)}</a></li>
												{/each}
												{#if w.more}<li class="faint">+{w.more} more</li>{/if}
											</ul>
										{:else}
											<p class="faint">Runs after it; no lineage between their nodes explains the wait.</p>
										{/if}
									</div>
								{/each}
							{:else}
								<p class="faint">Doesn't wait for another job in this group.</p>
							{/if}
						</InspectorSection>
						<InspectorSection title="Last run">
							{#if memberFacts?.lastRun}
								{@const r = memberFacts.lastRun}
								<dl>
									<div><dt>Status</dt><dd><a href={to(`/runs/${r.id}`)}><i style:background={runStatusTone[r.status]}></i>{runStatusLabel[r.status]}{r.startedAt ? ` · ${formatAgo(r.startedAt, app.now)}` : ''}</a></dd></div>
								</dl>
							{:else}
								<p class="faint">No runs yet.</p>
							{/if}
						</InspectorSection>
					{:else if pickedNodes.length === 1}
						{@const n = pickedNodes[0]}
						<p class="label">{kindLine(n)}</p>
						<h2 class="mono">{n.key}</h2>
						<a class="open" href={to(`/catalog/${n.key}`)}>Open in Catalog ↗</a>
						{#if n.description}<p class="desc">{n.description}</p>{/if}
						<dl>
							{#if n.lastOutcome || n.lastBuild}<div><dt>Last build</dt><dd><i style:background={nodeOutcomeTone[n.lastOutcome ?? 'success']}></i>{n.lastOutcome ? nodeOutcomeLabel[n.lastOutcome] : 'Built'}{n.lastBuild ? ` · ${formatAgo(n.lastBuild.builtAt, app.now)}` : ''}</dd></div>{/if}
							{#if n.relation}<div><dt>Writes</dt><dd class="mono">{[n.relation.database, n.relation.schema, n.relation.name].filter(Boolean).join('.')}</dd></div>{/if}
							{#if n.path}<div><dt>Defined in</dt><dd class="mono">{n.path}</dd></div>{/if}
						</dl>
					{:else if pickedNodes.length > 1}
						<h2>{pickedNodes.length} nodes selected</h2>
						<ul class="picked">
							{#each pickedNodes as n (n.key)}<li><a href={to(`/catalog/${n.key}`)} class="mono">{n.key}</a></li>{/each}
						</ul>
					{:else}
						{#if def.description}<InspectorSection title="Description"><p class="about-text">{def.description}</p></InspectorSection>{/if}
						<InspectorSection title="Recent runs">
							{#snippet meta()}{#if runs.length}<Button variant="link" size="inline" class="text-[12px]" onclick={() => (tab = 'runs')}>All runs →</Button>{/if}{/snippet}
							{#if finished.length}
								<OutcomeStrip outcomes={finished.map((r) => r.status)} tones={runStatusTone} caption="Last {finished.length} runs" />
								<dl class="spaced">
									<div><dt>Succeeded</dt><dd>{succeeded} of {finished.length}</dd></div>
									{#if typicalMs !== null}<div><dt>Typically takes</dt><dd>{formatMs(typicalMs)}</dd></div>{/if}
									{#if latest}<div><dt>Latest</dt><dd><a href={to(`/runs/${latest.id}`)}><i style:background={runStatusTone[latest.status]}></i>{runStatusLabel[latest.status]} · {formatAgo(latest.startedAt ?? latest.queuedAt, app.now)}</a></dd></div>{/if}
								</dl>
							{:else if jobRuns.data === undefined && !jobRuns.error}
								<Loading lines={2} />
							{:else}
								<p class="faint">No finished runs yet.</p>
							{/if}
						</InspectorSection>
						<InspectorSection title="Starts">
							{#if jobTriggers.length}
								<dl>
									{#each jobTriggers as t (t.id)}
										<div>
											<dt><a href={to(`/triggers/${t.id}`)}>{t.name}</a></dt>
											<dd class:faint={t.state === 'paused'}>{t.state === 'paused' ? 'Paused' : t.nextDueAt ? `Next ${formatIn(t.nextDueAt, app.now)}` : t.cadence}</dd>
										</div>
									{/each}
								</dl>
							{:else}
								<p class="faint">Manual only</p>
							{/if}
						</InspectorSection>
						<InspectorSection title="Definition">
							{#if def.select}<p class="label">Selection</p><p class="code">{def.select}</p>{/if}
							{#if def.exclude.length}<p class="label">Excludes</p><p class="code">{def.exclude.join(' ')}</p>{/if}
							<dl>
								{#if def.owner}<div><dt>Owner</dt><dd>{def.owner}</dd></div>{/if}
								{#if def.sourceFile}<div><dt>Defined in</dt><dd class="mono">{def.sourceFile}</dd></div>{/if}
								{#if def.memberOf?.length}
									<div><dt>In groups</dt><dd>{#each def.memberOf as g, i (g)}{i ? ', ' : ''}<a href={to(`/jobs/${g}`)}>{g}</a>{/each}</dd></div>
								{/if}
							</dl>
						</InspectorSection>
						{#if hasSettings}
							<InspectorSection title="Settings">
								<dl>
									{#if def.overlap}<div><dt>Overlap</dt><dd title="{overlapHelp[def.overlap]} (overlap = {def.overlap})">{overlapPlain[def.overlap]}</dd></div>{/if}
									{#if def.onFailure}<div><dt>Retries</dt><dd title="[retry] attempts = {def.onFailure.retries + 1}, delay_seconds = {def.onFailure.retryDelaySeconds}{def.memberOf?.length ? '. Inside a group run, the group retries this job while the jobs that depend on it wait' : ''}">{onFailureText(def.onFailure)}</dd></div>{/if}
									{#if def.priority !== null}<div><dt>Priority</dt><dd>{def.priority}</dd></div>{/if}
									{#if def.targets}<div><dt>Targets</dt><dd>{def.targets.join(', ')}</dd></div>{/if}
								</dl>
								{#if def.settings && Object.keys(def.settings).length}
									<p class="label">Options</p>
									{#each Object.entries(def.settings) as [flag, value] (flag)}
										<p class="setting"><code>{flag}{value === true ? '' : ` ${value}`}</code></p>
									{/each}
								{/if}
								{#if def.concurrency?.length}
									<p class="label">Shared slots</p>
									{#each def.concurrency as g (g.key)}
										<p class="setting" title="Concurrency group {g.key}: at most {g.limit} runs at once">
											<span>{g.holders.length}/{g.limit} {slotLabel(g.key)}</span>
											{#each g.holders as h (h)}<a class="mono" href={to(`/runs/${h}`)} title={h}>{shortRunId(h)}</a>{/each}
										</p>
									{/each}
								{:else if def.concurrency === null && def.concurrencyGroups.length}
									<p class="label">Shared slots</p>
									{#each def.concurrencyGroups as g (g)}<p class="setting" title="Concurrency group {g}">{slotLabel(g)}</p>{/each}
								{/if}
							</InspectorSection>
						{/if}
					{/if}
				</aside>
				{/if}
			</div>
		{:else if tab === 'runs'}
			<DataTable columns={runColumns} rows={runs} key={(r) => r.id} resource={jobRuns} empty="No runs yet.">
				{#snippet row(r)}
					<tr onclick={(e) => openRow(e, `/runs/${r.id}`)}>
						<td><i class="dot" style:background={runStatusTone[r.status]}></i>{runStatusLabel[r.status]}</td>
						<td><a href={to(`/runs/${r.id}`)} class="mono" title={r.id}>{shortRunId(r.id)}</a>{#if r.narrowing}<span class="part" title="Ran part of the job: sqb build {narrowingText(r.narrowing)}">part</span>{/if}</td>
						<td class="faint">{runOriginLabel[r.origin]}{r.parentRunId ? ` in group run ${shortRunId(r.parentRunId)}` : r.triggerName ? ` · ${r.triggerName}` : ''}</td>
						<td class="faint">{formatNodeCounts(r.nodeCounts)}</td>
						<td class="faint">{r.startedAt ? formatDuration(r.startedAt, r.endedAt, app.now) : '—'}</td>
						<td class="faint" title={formatStamp(r.queuedAt)}>{formatAgo(r.startedAt ?? r.queuedAt, app.now)}</td>
						<td class="faint" title={r.requestedBy.name}>{r.requestedBy.displayName}</td>
						{#if showCost}<td class="faint" title={r.cost ? `Estimated warehouse cost: ${r.cost.source}` : 'No cost reported'}>{formatCost(r.cost)}</td>{/if}
					</tr>
				{/snippet}
			</DataTable>
		{:else if def.declaration}
			<PageBody padding="12px 24px">
				<CodePreview file="{def.declaration.sourceFile}:{def.declaration.line}" code={def.declaration.code} language="text" maxLines={200} />
			</PageBody>
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
	.part {
		margin-left: 8px;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	i.dot {
		display: inline-block;
		width: 7px;
		height: 7px;
		margin-right: 6px;
		border-radius: 50%;
	}
	.chip i {
		margin-right: 0;
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
	.rail {
		width: 320px;
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
	.rail.sections {
		padding: 0;
	}
	.about-text {
		color: var(--muted-foreground);
		font-size: 13px;
		line-height: 1.5;
	}
	.wait + .wait {
		margin-top: 10px;
	}
	.wait.implied + .wait.implied {
		margin-top: 3px;
	}
	.reasons {
		margin-top: 4px;
		padding-left: 12px;
		border-left: 1px solid var(--border);
	}
	.reasons li {
		padding: 1px 0;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.spaced {
		margin-top: 12px;
	}
	.code {
		margin-bottom: 10px;
		padding: 6px 8px;
		border-radius: 4px;
		background: var(--sb-inset);
		font-family: var(--font-mono);
		font-size: 11.5px;
		overflow-wrap: anywhere;
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
	a:hover {
		color: var(--primary);
	}
	.sb-scroll {
		scrollbar-width: thin;
		scrollbar-color: var(--border-strong) transparent;
	}
</style>
