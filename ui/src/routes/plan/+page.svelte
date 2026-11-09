<script lang="ts">
	// The plan: what a command picks, what each picked node's state is now, what it updates, the options
	// it runs with and the permissions it needs, all from the server's dry run, before anything runs.
	// Every Run button opens this page with its command; nothing is queued until Run is pressed here.
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import CopyIcon from '@lucide/svelte/icons/copy';
	import type { AdhocRunCheck, AdhocRunRequest, CatalogNode, LaunchVerb } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import { kindLine } from '$lib/build-area/labels';
	import { formatAgo } from '$lib/format/time';
	import ScopeGraph from '$lib/lineage/scope-graph.svelte';
	import { PERMISSIONS } from '$lib/platform/permission-catalog';
	import { VERBS } from '$lib/run/command-table';
	import { applySuggestion, commandTokens, parseCommand, suggest, withVerb, type Suggestion } from '$lib/run/command';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	const snapshotNodes = createResource((client, scope) => client.listSnapshotNodes(scope), ['snapshot']);
	const catalog = createResource((client, scope) => client.listCatalogNodes(scope), ['snapshot', 'runs', 'node_status']);

	let cmd = $state(page.url.searchParams.get('cmd') ?? 'sqb build');
	let cmdEl = $state<HTMLInputElement>();

	const parts = $derived(parseCommand(cmd));
	const verb = $derived(parts.verb);
	const request = $derived<AdhocRunRequest>({ command: commandTokens(cmd), priority: 0 });
	const requestKey = $derived(JSON.stringify(request));
	const mayRun = $derived(app.can('run.adhoc'));

	// The command stays in the address, so a plan can be shared or reopened.
	$effect(() => {
		const href = `/plan?cmd=${encodeURIComponent(cmd.trim())}`;
		const timer = setTimeout(() => {
			if (page.url.pathname === '/plan' && page.url.pathname + page.url.search !== href) void goto(href, { replaceState: true, keepFocus: true, noScroll: true });
		}, 400);
		return () => clearTimeout(timer);
	});

	// ---- server dry run (debounced; nothing is queued or recorded) ----
	let check = $state<AdhocRunCheck | null>(null);
	let checkedKey = $state('');
	let checking = $state(false);
	let checkError = $state<string | null>(null);
	let seq = 0;
	$effect(() => {
		if (!mayRun) return;
		const key = requestKey;
		const body = request;
		const scope = app.scope;
		if (!scope) return;
		const mine = ++seq;
		checking = true;
		const timer = setTimeout(async () => {
			try {
				const result = await app.client.checkAdhocRun(scope, body);
				if (mine !== seq) return;
				check = result;
				checkedKey = key;
				checkError = null;
			} catch (e) {
				if (mine === seq) checkError = e instanceof Error ? e.message : String(e);
			} finally {
				if (mine === seq) checking = false;
			}
		}, 250);
		return () => clearTimeout(timer);
	});
	const fresh = $derived(!!check && checkedKey === requestKey && !checking);

	// ---- each picked node's state now, from the catalog ----
	// Where the server doesn't report staleness, a node is grouped by its last build instead.
	type NodeState = 'failed' | 'never' | 'changed' | 'upstream' | 'behind' | 'current' | 'built' | 'source';
	const STATES: { value: NodeState; label: string; tone: string }[] = [
		{ value: 'failed', label: 'Failed last time', tone: 'var(--sb-error)' },
		{ value: 'never', label: 'Never built', tone: 'var(--sb-warning)' },
		{ value: 'changed', label: 'Changed', tone: 'var(--sb-warning)' },
		{ value: 'upstream', label: 'Upstream changed', tone: 'var(--sb-warning)' },
		{ value: 'behind', label: 'Behind', tone: 'var(--sb-warning)' },
		{ value: 'current', label: 'Up to date', tone: 'var(--sb-success)' },
		{ value: 'built', label: 'Built before', tone: 'var(--sb-success)' },
		{ value: 'source', label: 'Sources', tone: 'var(--sb-text-faint)' }
	];
	const QUIET = new Set<NodeState>(['current', 'built', 'source']);
	function stateOf(n: CatalogNode | undefined): NodeState {
		if (n?.kind === 'source') return 'source';
		if (n?.lastOutcome === 'failed') return 'failed';
		if (!n?.lastBuild) return 'never';
		if (n.stale === null) return 'built';
		if (n.stale.some((r) => r.reason === 'changed')) return 'changed';
		if (n.stale.some((r) => r.reason === 'changed_upstream')) return 'upstream';
		if (n.stale.some((r) => r.reason === 'behind')) return 'behind';
		return 'current';
	}
	const byKey = $derived(new Map((catalog.data ?? []).map((n) => [n.key, n])));
	const picked = $derived(check?.selection ?? []);
	const writes = $derived(new Set(check?.writeSet ?? []));
	const groups = $derived(
		STATES.map((s) => ({ ...s, nodes: picked.filter((k) => stateOf(byKey.get(k)) === s.value).map((k) => ({ key: k, node: byKey.get(k) })) })).filter((g) => g.nodes.length)
	);

	// ---- autocomplete ----
	let caret = $state(0);
	let acOpen = $state(false);
	let acIndex = $state(0);
	const suggestions = $derived<Suggestion[]>(acOpen ? suggest(cmd, caret, snapshotNodes.data ?? []) : []);

	function track() {
		caret = cmdEl?.selectionStart ?? cmd.length;
		acOpen = true;
		acIndex = 0;
	}

	function pick(s: Suggestion) {
		const next = applySuggestion(cmd, caret, s.value);
		cmd = next.text;
		acOpen = false;
		queueMicrotask(() => {
			cmdEl?.focus();
			cmdEl?.setSelectionRange(next.caret, next.caret);
			caret = next.caret;
		});
	}

	function onKeydown(e: KeyboardEvent) {
		if (!acOpen || !suggestions.length) return;
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			acIndex = (acIndex + 1) % suggestions.length;
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			acIndex = (acIndex - 1 + suggestions.length) % suggestions.length;
		} else if (e.key === 'Enter' || e.key === 'Tab') {
			e.preventDefault();
			pick(suggestions[acIndex]);
		} else if (e.key === 'Escape') {
			acOpen = false;
		}
	}

	function setVerb(v: LaunchVerb) {
		cmd = withVerb(cmd, v);
		cmdEl?.focus();
	}

	/** Appends an option and lists the options that can follow, as typing `--` does. */
	function insertFlag(flag: string) {
		cmd = `${cmd.trimEnd()} ${flag}`;
		queueMicrotask(() => {
			cmdEl?.focus();
			cmdEl?.setSelectionRange(cmd.length, cmd.length);
			track();
		});
	}

	let copied = $state(false);
	async function copy() {
		await navigator.clipboard.writeText(`${cmd.trim()} --target ${app.target?.name ?? ''}`);
		copied = true;
		setTimeout(() => (copied = false), 1500);
	}

	// ---- run ----
	let launching = $state(false);
	let launchError = $state<string | null>(null);
	const canLaunch = $derived(fresh && !!check?.launchable);
	const blockedReason = $derived(
		!mayRun
			? "You don't have permission to start runs by hand in this target (run.adhoc)"
			: !fresh
				? undefined
				: (check?.error ?? check?.permissions.find((p) => !p.allowed)?.reason ?? undefined)
	);

	async function launch() {
		const scope = app.scope;
		if (!scope || !canLaunch) return;
		launching = true;
		launchError = null;
		try {
			const run = await app.client.launchAdhocRun(scope, request);
			await goto(`/runs/${run.id}`);
		} catch (e) {
			launchError = e instanceof Error ? e.message : String(e);
		} finally {
			launching = false;
		}
	}
</script>

<AppTopbar title="Plan" />

<div class="flex min-h-0 flex-1 flex-col">
	<div class="cmd">
		<div class="verbs" role="group" aria-label="Command">
			{#each VERBS as v (v.name)}
				<button class:on={verb === v.name} onclick={() => setVerb(v.name)} title={v.description}>{v.name}</button>
			{/each}
		</div>
		<div class="line">
			<span class="faint">$</span>
			<input
				bind:this={cmdEl}
				bind:value={cmd}
				oninput={track}
				onclick={track}
				onkeyup={(e) => (e.key === 'ArrowLeft' || e.key === 'ArrowRight') && track()}
				onkeydown={onKeydown}
				onblur={() => setTimeout(() => (acOpen = false), 120)}
				spellcheck="false"
				aria-label="Command"
				placeholder="sqb build --select +orders"
			/>
			<button class="copy" onclick={() => insertFlag('--')} title="Add an option to the command">＋ Option</button>
			<button class="copy" onclick={copy} title="Copy the command, to run it with sqb yourself"><CopyIcon size={12} /> {copied ? 'Copied' : 'Copy'}</button>
			{#if acOpen && suggestions.length}
				<div class="ac">
					{#each suggestions as s, i (s.value)}
						<button class:on={i === acIndex} onmouseenter={() => (acIndex = i)} onmousedown={(e) => (e.preventDefault(), pick(s))}>
							<code>{s.label}</code><span>{s.hint}</span>
						</button>
					{/each}
				</div>
			{/if}
		</div>
	</div>

	<div class="body">
		<div class="main sb-scroll" class:stale={check && !fresh}>
			{#if !mayRun}
				<p class="msg">You don't have permission to start runs by hand in this target (run.adhoc).</p>
			{:else if checkError}
				<p class="msg problem">{checkError}</p>
			{:else if check}
				{#if check.error}<p class="msg problem" data-testid="adhoc-error">{check.error}</p>{/if}
				{#if check.verb && picked.length}
					<div class="summary">
						<b>{picked.length}</b> node{picked.length === 1 ? '' : 's'}{parts.select.length ? '' : ' · whole project'}
						<span>·</span> updates <b>{check.writeSet.length}</b>
						{#each groups.filter((g) => !QUIET.has(g.value)) as g (g.value)}
							<span>·</span> <i style:background={g.tone}></i>{g.nodes.length} {g.label.toLowerCase()}
						{/each}
					</div>
					{#if catalog.data}
						<div class="graph"><ScopeGraph keys={picked} catalog={catalog.data} onselect={() => {}} /></div>
					{/if}
					<table class="nodes">
						{#each groups as g (g.value)}
							<tbody>
								<tr class="group"><td colspan="4"><i style:background={g.tone}></i>{g.label} · {g.nodes.length}</td></tr>
								{#each g.nodes as { key, node } (key)}
									<tr>
										<td><a href="/catalog/{key}" class="mono">{key}</a></td>
										<td class="faint">{node ? kindLine(node) : ''}</td>
										<td class="faint">{node?.lastBuild ? `built ${formatAgo(node.lastBuild.builtAt, app.now)}` : ''}</td>
										<td class="faint right">{writes.has(key) ? 'updates' : 'reads'}</td>
									</tr>
								{/each}
							</tbody>
						{/each}
					</table>
				{/if}
			{:else if checking}
				<p class="msg faint">Checking the command…</p>
			{/if}
		</div>

		{#if check?.verb}
		<aside class="rail sb-scroll">
				<h3>Permissions</h3>
				{#each check.permissions as p (p.action)}
					<p class="decision" data-testid="adhoc-permission">
						<span class={p.allowed ? 'ok-text' : 'bad-text'}>{p.allowed ? '✓' : '✕'}</span>
						<span title={p.action}>{PERMISSIONS[p.action].label}</span>
					</p>
					{#if p.reason}<p class="bad-text reason">{p.reason}</p>{/if}
				{/each}
				{#if Object.keys(check.settings).length}
					<h3>Options</h3>
					<div class="chips">
						{#each Object.entries(check.settings) as [name, value] (name)}<span class="sb-tag code">{value === true ? name : `${name}=${value}`}</span>{/each}
					</div>
				{/if}
		</aside>
		{/if}
	</div>

	<div class="foot">
		<div class="why">
			{#if launchError}<span class="problem">{launchError}</span>{:else if !canLaunch && blockedReason}<span class="faint">{blockedReason}</span>{/if}
		</div>
		<GatedButton requires="run.adhoc" variant={canLaunch ? 'default' : 'outline'} disabled={!canLaunch} reason={blockedReason} busy={launching} onclick={launch}>▷ Run</GatedButton>
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
	.line {
		position: relative;
		display: flex;
		min-width: 0;
		flex: 1;
		align-items: center;
		gap: 10px;
		padding: 5px 10px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--sb-inset);
	}
	.line:focus-within {
		border-color: var(--primary);
	}
	.line input {
		min-width: 0;
		flex: 1;
		background: transparent;
		color: var(--foreground);
		font-family: var(--font-mono);
		font-size: 12.5px;
		outline: none;
	}
	.copy {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.copy:hover {
		color: var(--foreground);
	}
	.ac {
		position: absolute;
		top: 100%;
		left: 0;
		z-index: 60;
		width: 520px;
		max-height: 300px;
		overflow: auto;
		margin-top: 4px;
		padding: 4px 0;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--popover);
		box-shadow: var(--sb-elev);
	}
	.ac button {
		display: flex;
		width: 100%;
		align-items: baseline;
		gap: 8px;
		padding: 5px 12px;
		text-align: left;
	}
	.ac button.on {
		background: var(--sb-hover);
	}
	.ac span {
		overflow: hidden;
		color: var(--sb-text-faint);
		font-size: 11px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.body {
		display: flex;
		min-height: 0;
		flex: 1;
	}
	.main {
		min-width: 0;
		flex: 1;
		overflow: auto;
		padding: 14px 24px;
		transition: opacity 0.15s;
	}
	.main.stale {
		opacity: 0.55;
	}
	.msg {
		font-size: 13px;
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
	i {
		display: inline-block;
		width: 7px;
		height: 7px;
		margin-right: 6px;
		border-radius: 50%;
	}
	.summary i {
		margin-right: 0;
	}
	.graph {
		height: 340px;
		margin-bottom: 14px;
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
	.nodes tr.group td {
		padding-top: 14px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	.nodes tbody:first-child tr.group td {
		padding-top: 4px;
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
		margin: 16px 0 6px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	.rail h3:first-child {
		margin-top: 0;
	}
	.decision {
		display: flex;
		align-items: baseline;
		gap: 8px;
		margin: 3px 0;
		font-size: 12.5px;
	}
	.reason {
		margin: 0 0 6px 18px;
		font-size: 11.5px;
	}
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 5px;
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
	code,
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
	.sb-scroll {
		scrollbar-width: thin;
		scrollbar-color: var(--border-strong) transparent;
	}
</style>
