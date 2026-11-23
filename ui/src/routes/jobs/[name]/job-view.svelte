<script lang="ts">
	import SelectionChips from '$lib/components/selection/selection-chips.svelte';
	import { plural } from '$lib/format/plural';
	import PageBody from '$lib/components/page-body.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Button from '$lib/components/ui/button/button.svelte';
	import PageTabs from '$lib/components/platform/page-tabs.svelte';
	import Loading from '$lib/components/loading.svelte';
	import PlayIcon from '@lucide/svelte/icons/play';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import { createDetailsPanel } from '$lib/state/details-panel.svelte';
	import { openRow, to } from '$lib/paths';
	import { goto } from '$app/navigation';
	import type { CatalogNode, RunNode, Trigger } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { useCatalogNodes } from '$lib/state/catalog.svelte';
	import { formatAgo, formatDuration, formatIn, formatStamp } from '$lib/format/time';
	import { formatNodeCounts, isActiveRun, nodeOutcomeLabel, nodeOutcomeTone, runOriginLabel, runStatusLabel, runStatusTone } from '$lib/status';
	import { kindLine, shortName } from '$lib/build-area/labels';
	import { planHref } from '$lib/run/command';
	import { CONCURRENCY_DOCS, formatCost, narrowingText, onFailureText, overlapHelp, overlapPlain, poolText, runConcurrencyHelp, shortRunId } from '$lib/operate/labels';
	import ScopeGraph from '$lib/lineage/scope-graph.svelte';
	import MemberGraph from '$lib/lineage/member-graph.svelte';
	import { memberLinks, publishedLinks, waitReasons, type MemberInfo, type MemberSelection } from '$lib/lineage/member-graph';
	import InspectorHead from '$lib/components/lineage/inspector-head.svelte';
	import InspectorSection from '$lib/components/lineage/inspector-section.svelte';
	import HelpTip from '$lib/components/help-tip.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import OutcomeStrip from '$lib/components/build-area/outcome-strip.svelte';
	import { formatMs } from '$lib/build-area/labels';
	import BreakableName from '$lib/components/names/breakable-name.svelte';
	import PathTail from '$lib/components/names/path-tail.svelte';
	import DiscoveredFacts from './discovered/discovered-facts.svelte';
	import { discoveredBoxes, itemOf, setsInJob } from './discovered/utils';

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
		return new Map<string, MemberInfo>(jobs.map((j) => [j.key, { nodeCount: j.nodeCount, lastRun: j.lastRun, description: j.description, nodes: j.nodes, sourceFile: j.sourceFile }]));
	}, ['jobs', 'runs', 'snapshot']);

	const def = $derived(job.data ?? null);
	// The discovered sets with steps in this job, each framed in the graph under its asset.
	const discovered = createResource((client, scope) => client.readDiscovered(scope), ['snapshot']);
	const jobSets = $derived(def ? setsInJob(discovered.data ?? [], def.resolvedNodes) : []);
	const boxes = $derived(discoveredBoxes(jobSets, def?.resolvedNodes ?? []));
	// Why each member waits for another, node by node, from the catalog's lineage.
	// Why members wait, node by node: as the server publishes it, or worked out from lineage for a
	// snapshot that predates published reasons.
	const published = $derived(def?.groupMembers ? publishedLinks(def.groupMembers) : null);
	const links = $derived(published ?? (def?.groupMembers && memberJobs.data && catalog.data ? memberLinks(def.groupMembers, memberJobs.data, catalog.data) : new Map()));
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
	// While the job's latest run is active, its nodes show that run's progress: running, succeeded, failed.
	const liveRunId = $derived(latest && isActiveRun(latest.status) ? latest.id : null);
	const liveRun = createResource((client, scope) => (liveRunId ? client.getRunDetail(scope, liveRunId) : Promise.resolve(null)), ['runs']);
	const LIVE: Partial<Record<RunNode['status'], NonNullable<CatalogNode['liveStatus']>>> = { running: 'run', success: 'ok', warn: 'warn', failed: 'fail' };
	const liveCatalog = $derived.by(() => {
		const states = liveRunId && liveRun.data?.run.id === liveRunId ? new Map(liveRun.data.nodes.flatMap((n) => (LIVE[n.status] ? [[n.nodeKey, LIVE[n.status]!] as const] : []))) : null;
		if (!states?.size || !catalog.data) return catalog.data;
		return catalog.data.map((n) => (states.has(n.key) ? { ...n, liveStatus: states.get(n.key) } : n));
	});
	// Recent finished runs, oldest first, for the sidebar's strip and typical duration.
	const finished = $derived(runs.filter((r) => r.endedAt).slice(0, 20).reverse());
	const succeeded = $derived(finished.filter((r) => r.status === 'success').length);
	const typicalMs = $derived.by(() => {
		const took = finished.flatMap((r) => (r.startedAt && r.endedAt ? [Date.parse(r.endedAt) - Date.parse(r.startedAt)] : [])).sort((a, b) => a - b);
		return took.length ? took[Math.floor(took.length / 2)] : null;
	});
	// Each pool the job takes, as published.
	const pools = $derived(def?.pools ?? []);
	/** Runs holding a pool's slots now, where the server reports them. */
	const holdersOf = (pool: string) => def?.poolHolders?.find((g) => g.pool === pool)?.holders ?? [];
	const hasSettings = $derived(!!def && (!!def.onFailure || !!def.ignoreAuditFailures || def.priority !== null || !!def.targets || !!(def.settings && Object.keys(def.settings).length)));
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

<AppTopbar title="Jobs" subject="{name} · Job">
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
		<div class="msg">No job or group <code>{name}</code> in {app.location?.name}.</div>
	{/if}
	{#if actionError}<div class="msg err">{actionError}</div>{/if}

	{#if def}
		<PageTabs bind:value={tab} tabs={[{ value: 'overview', label: 'Overview' }, { value: 'runs', label: 'Runs', count: runs.length || undefined }, ...(def.declaration ? [{ value: 'code' as const, label: 'Code' }] : [])]} />

		{#if tab === 'overview'}
			<div class="flex min-h-0 flex-1">
				<div class="canvas">
					{#if def.kind === 'group' && !def.groupMembers?.length}
						<p class="faint graph-wait">{def.groupMembers ? 'This group has no member jobs.' : "The server doesn't report this group's member jobs."}</p>
					{:else if def.kind === 'group' && !memberJobs.data}
						<!-- Members are drawn once the jobs list says where each is declared, so the graph opens by folder rather than redrawing. -->
						<div class="graph-wait">{#if memberJobs.error}<p class="msg err">Couldn't load the member jobs: {memberJobs.error}</p>{:else}<Loading />{/if}</div>
					{:else if def.kind === 'group'}
						<!-- A group draws its member jobs; each member's page draws that job's nodes. -->
						<MemberGraph members={def.groupMembers ?? []} info={memberJobs.data ?? new Map()} catalog={catalog.data ?? []} {links} bind:selection={memberSelection} details={{ open: details.open, toggle: details.toggle }} />
					{:else if catalog.data}
						{#key boxes.length}
							<ScopeGraph bind:this={graph} keys={def.resolvedNodes} catalog={liveCatalog ?? catalog.data} {boxes} onselect={(keys) => (picked = keys)} />
						{/key}
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
						{@const found = itemOf(jobSets, n.key)}
						<p class="label">{kindLine(n)}</p>
						<h2 class="mono wrap-name"><BreakableName text={n.key} /></h2>
						<a class="open" href={to(`/catalog/${n.key}`)}>Open in Catalog ↗</a>
						{#if n.description}<p class="desc">{n.description}</p>{/if}
						<dl>
							{#if n.lastOutcome || n.lastBuild}<div><dt>Last build</dt><dd><i style:background={nodeOutcomeTone[n.lastOutcome ?? 'success']}></i>{n.lastOutcome ? nodeOutcomeLabel[n.lastOutcome] : 'Built'}{n.lastBuild ? ` · ${formatAgo(n.lastBuild.builtAt, app.now)}` : ''}</dd></div>{/if}
							{#if n.relation}<div><dt>Writes</dt><dd class="mono">{[n.relation.database, n.relation.schema, n.relation.name].filter(Boolean).join('.')}</dd></div>{/if}
							{#if n.path}<div><dt>Defined in</dt><dd class="path"><PathTail path={n.path} /></dd></div>{/if}
						</dl>
						{#if found}
							<p class="label sub">Item</p>
							<dl>
								<div><dt>Step</dt><dd class="mono">{found.item.step}</dd></div>
								<div><dt>Asset</dt><dd class="mono"><BreakableName text={found.item.asset} /></dd></div>
								{#each Object.entries(found.item.args).filter(([k]) => k !== 'step') as [k, v] (k)}<div><dt class="mono">{k}</dt><dd class="mono">{v}</dd></div>{/each}
							</dl>
							<p class="label sub">Discovered set</p>
							<DiscoveredFacts set={found.set} />
						{/if}
					{:else if pickedNodes.length > 1}
						<h2>{pickedNodes.length} nodes selected</h2>
						<ul class="picked">
							{#each pickedNodes as n (n.key)}<li><a href={to(`/catalog/${n.key}`)} class="mono">{n.key}</a></li>{/each}
						</ul>
					{:else}
						{#each jobSets as set (set.name)}<InspectorSection title="Discovered set"><DiscoveredFacts {set} /></InspectorSection>{/each}
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
							{#if def.select || def.exclude.length}<p class="label">Selection</p><div class="chips"><SelectionChips select={def.select ?? ''} exclude={def.exclude.join(' ')} /></div>{/if}
							<dl>
								{#if def.owner}<div><dt>Owner</dt><dd>{def.owner}</dd></div>{/if}
								{#if def.sourceFile}<div><dt>Defined in</dt><dd class="path"><PathTail path={def.sourceFile} /></dd></div>{/if}
								{#if def.memberOf?.length}
									<div><dt>In groups</dt><dd>{#each def.memberOf as g, i (g)}{i ? ', ' : ''}<a href={to(`/jobs/${g}`)}>{g}</a>{/each}</dd></div>
								{/if}
							</dl>
						</InspectorSection>
						<!-- Concurrency as facts, each explained in a help tip: nodes at once, shared tables, overlap and pools. -->
						<InspectorSection title="Concurrency">
							{#snippet meta()}<HelpTip text="How this job's runs share the warehouse with other runs." docs={CONCURRENCY_DOCS} label="How concurrency works" />{/snippet}
							{@const resolved = def.resolvedConcurrency}
							<div class="concurrency">
								{#snippet poolList()}
									<ul class="pools">
										{#each pools as use (use.pool)}
											{@const text = poolText(use)}
											{@const holders = holdersOf(use.pool)}
											<li class="pool" class:clamped={text.clamped}>
												<span
													>{#if text.clamped}<TriangleAlertIcon size={12} class="inline-icon" />{/if}{text.text}{#if text.detail}&nbsp;<span class:muted={!text.clamped}>({text.detail})</span>{/if}{#if text.shared.length}, shared with {#each text.shared as other, i (other)}{i ? ', ' : ''}<a href={to(`/jobs/${other}`)}>{other}</a>{/each}{#if text.more}&nbsp;+{text.more} more{/if}{/if}</span
												>
												{#if holders.length}
													<span class="muted">Running now: {#each holders as h, i (h)}{i ? ', ' : ''}<a class="mono" href={to(`/runs/${h}`)} title={h}>{shortRunId(h)}</a>{/each}</span>
												{/if}
											</li>
										{/each}
									</ul>
								{/snippet}
								<KeyValues
									rows={[
										...(resolved ? [{ label: 'Nodes at once', value: String(resolved.limit), help: runConcurrencyHelp(def.kind) }] : []),
										{ label: 'Shared tables', value: 'Exclusive · automatic', help: 'Never runs alongside runs writing the same tables.' },
										...(def.overlap ? [{ label: 'When already running', value: overlapPlain[def.overlap], title: `${overlapHelp[def.overlap]} (overlap = ${def.overlap})` }] : []),
										pools.length ? { label: 'Pools', content: poolList } : { label: 'Pools', value: 'None' }
									]}
								/>
							</div>
						</InspectorSection>
						{#if hasSettings}
							<InspectorSection title="Settings">
								<dl>
									{#if def.onFailure}<div><dt>Retries</dt><dd title="[retry] attempts = {def.onFailure.retries + 1}, delay_seconds = {def.onFailure.retryDelaySeconds}{def.memberOf?.length ? '. Inside a group run, the group retries this job while the jobs that depend on it wait' : ''}">{onFailureText(def.onFailure)}</dd></div>{/if}
									{#if def.priority !== null}<div><dt>Priority</dt><dd>{def.priority}</dd></div>{/if}
									{#if def.targets}<div><dt>Targets</dt><dd>{def.targets.join(', ')}</dd></div>{/if}
								</dl>
								{#if def.ignoreAuditFailures}<p class="audits" data-testid="job-ignores-audit-failures" title="ignore_audit_failures = true: an error-severity audit that fails is recorded, and the run goes on">Audit failures don't stop runs</p>{/if}
								{#if def.settings && Object.keys(def.settings).length}
									<p class="label">Options</p>
									{#each Object.entries(def.settings) as [flag, value] (flag)}
										<p class="setting"><code>{flag}{value === true ? '' : ` ${value}`}</code></p>
									{/each}
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
	/* Names wrap between their words only. */
	.rail h2.wrap-name {
		word-break: normal;
		overflow-wrap: normal;
	}
	dd.path {
		flex: 1;
		min-width: 0;
	}
	.label.sub {
		margin: 14px 0 4px;
		padding-top: 10px;
		border-top: 1px solid var(--border);
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
	.chips {
		margin: 3px 0 10px;
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
	/* The Concurrency panel: key-value facts; pools list one per line, any detail quieter beside it. */
	.concurrency :global(dd) {
		white-space: normal;
	}
	.pools {
		display: flex;
		flex-direction: column;
		gap: 6px;
		margin: 0;
		padding: 0;
		list-style: none;
		line-height: 1.4;
	}
	.pools .muted {
		color: var(--muted-foreground);
	}
	.pools .pool {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	/* A pool this server caps below what the job asks: it takes the whole pool, so it runs alone there. */
	.pools .pool.clamped {
		padding: 4px 8px;
		border-left: 3px solid var(--sb-warning);
		border-radius: 2px;
		background: color-mix(in srgb, var(--sb-warning) 10%, transparent);
		color: var(--sb-warning);
	}
	.pools .pool.clamped a {
		color: inherit;
		text-decoration: underline;
	}
	.pools :global(.inline-icon) {
		display: inline;
		margin-right: 4px;
		vertical-align: -1px;
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
	.audits {
		margin: 6px 0 0;
		color: color-mix(in srgb, var(--sb-warning) 70%, var(--foreground));
		font-size: 12px;
	}
</style>
