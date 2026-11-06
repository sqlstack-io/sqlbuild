<script lang="ts">
	// Run panel: one editable command line is the interface. Verbs and flags come
	// from the CLI's command table; selectors autocomplete from the snapshot. The
	// server checks the request before launch: what it resolves to, what it
	// writes, and whether the caller may write all of it.
	import { goto } from '$app/navigation';
	import LockIcon from '@lucide/svelte/icons/lock';
	import XIcon from '@lucide/svelte/icons/x';
	import type { AdhocRunCheck, LaunchVerb } from '$lib/api';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { VERBS, verbSpec } from './command-table';
	import { applySuggestion, parseCommand, suggest, withVerb, type Suggestion } from './command';

	let {
		open = $bindable(false),
		selection = [],
		verb: initialVerb = 'build'
	}: {
		open?: boolean;
		selection?: string[];
		/** The verb the command line opens with, e.g. `audit` for a check-only run. */
		verb?: LaunchVerb;
	} = $props();

	const snapshotNodes = createResource((client, scope) => client.listSnapshotNodes(scope), ['snapshot']);

	let cmd = $state('sqb build ');
	let cmdEl = $state<HTMLInputElement>();
	let wasOpen = false;
	$effect(() => {
		if (open && !wasOpen) {
			cmd = `sqb ${initialVerb} ${selection.length ? `--select ${selection.join(' ')} ` : ''}`;
			check = null;
			launchError = null;
			queueMicrotask(() => cmdEl?.focus());
		}
		wasOpen = open;
	});

	const parsed = $derived(parseCommand(cmd));
	const verb = $derived(parsed.request.verb);
	const spec = $derived(verbSpec(verb));
	const requestKey = $derived(JSON.stringify(parsed.request));

	// ---- server check (debounced) ----
	let check = $state<AdhocRunCheck | null>(null);
	let checkedKey = $state('');
	let checking = $state(false);
	let checkError = $state<string | null>(null);
	let seq = 0;
	$effect(() => {
		if (!open) return;
		const key = requestKey;
		const request = parsed.request;
		const scope = app.scope;
		if (!scope) return;
		const mine = ++seq;
		checking = true;
		const timer = setTimeout(async () => {
			try {
				const result = await app.client.checkAdhocRun(scope, request);
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
		if (e.key === 'Enter' && (!acOpen || !suggestions.length)) {
			e.preventDefault();
			void launch();
			return;
		}
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

	function insertFlag(flag: string) {
		cmd = `${cmd.trimEnd()} ${flag} `;
		queueMicrotask(() => {
			cmdEl?.focus();
			cmdEl?.setSelectionRange(cmd.length, cmd.length);
		});
	}

	// ---- launch ----
	let launching = $state(false);
	let launchError = $state<string | null>(null);
	const canLaunch = $derived(fresh && !!check?.ok && parsed.errors.length === 0);
	const blockedReason = $derived(
		parsed.errors[0] ??
			(!fresh ? 'Checking with the server…' : check && !check.adhoc.allowed ? 'You need permission to build every node this run would update (run.adhoc)' : check?.rewriteHistory.required && !check.rewriteHistory.allowed ? "You don't have permission to rebuild history (run.rewrite_history)" : check?.problems[0]?.message)
	);

	async function launch() {
		const scope = app.scope;
		if (!scope || !canLaunch) return;
		launching = true;
		launchError = null;
		try {
			const run = await app.client.launchAdhocRun(scope, parsed.request);
			open = false;
			await goto(`/runs/${run.id}`);
		} catch (e) {
			launchError = e instanceof Error ? e.message : String(e);
		} finally {
			launching = false;
		}
	}

	const CHIP_CAP = 40;
</script>

{#snippet chips(keys: string[], tone: 'plain' | 'bad' = 'plain')}
	<div class="chips">
		{#each keys.slice(0, CHIP_CAP) as k (k)}<span class="sb-tag code" class:bad={tone === 'bad'}>{k}</span>{/each}
		{#if keys.length > CHIP_CAP}<span class="faint">+{keys.length - CHIP_CAP} more</span>{/if}
	</div>
{/snippet}

{#if open}
	<button class="fixed inset-0 z-40 bg-black/40" aria-label="Close" onclick={() => (open = false)}></button>
	<div class="panel">
		<div class="head">
			<span class="font-display text-[15px] font-semibold">Run</span>
			<div class="verbs">
				{#each VERBS as v (v.name)}
					<button class:on={verb === v.name} onclick={() => setVerb(v.name)} title={v.description}>{v.name}</button>
				{/each}
			</div>
			<span class="faint">{spec.description}</span>
			<span class="target" title="Runs launch in the selected location, against its one target">
				<LockIcon size={12} /> <b>{app.location?.name}</b> · target {app.target?.name}
			</span>
			<button class="close" onclick={() => (open = false)} aria-label="Close"><XIcon size={16} /></button>
		</div>

		<div class="body">
			<div class="check sb-scroll">
				<div class="section-title">
					Server check
					{#if checking}<span class="faint">checking…</span>{:else if checkError}<span class="bad-text">{checkError}</span>{/if}
				</div>
				{#if parsed.errors.length}
					{#each parsed.errors as e (e)}<p class="problem">{e}</p>{/each}
				{/if}
				{#if check}
					<div class="block" class:stale={!fresh}>
						<h5>Picks <span>{check.resolvedNodes.length} node{check.resolvedNodes.length === 1 ? '' : 's'}{parsed.request.select.length ? '' : ' · whole project'}</span></h5>
						{@render chips(check.resolvedNodes)}
					</div>
					<div class="block" class:stale={!fresh}>
						<h5 title="The write set: nodes this run updates and reserves while it runs">Updates <span>{check.writeSet.length ? `${check.writeSet.length} node${check.writeSet.length === 1 ? '' : 's'}` : 'nothing'}</span></h5>
						{#if check.writeSet.length}{@render chips(check.writeSet)}{:else}<p class="faint">sqb {verb} only reads, so it doesn't wait for other runs.</p>{/if}
					</div>
					<div class="block" class:stale={!fresh}>
						<h5 title="run.adhoc">
							Permission
							{#if check.adhoc.allowed}<span class="ok-text">you can update all of these</span>{:else}<span class="bad-text">the whole run would be refused</span>{/if}
						</h5>
						{#if !check.adhoc.allowed}
							<p class="faint">
								{app.can('run.adhoc') ? `You can't update ${check.adhoc.outOfScope.length} of the nodes this run would update. Narrow the selection, or ask for a role that covers them (run.adhoc).` : "You don't have permission to start runs by hand in this target (run.adhoc)."}
							</p>
							{@render chips(check.adhoc.outOfScope, 'bad')}
						{/if}
					</div>
					{#if check.rewriteHistory.required}
						<div class="block" class:stale={!fresh}>
							<h5 title="run.rewrite_history">
								Rebuilds history
								{#if check.rewriteHistory.allowed}<span class="ok-text">you're allowed to</span>{:else}<span class="bad-text">you need permission</span>{/if}
							</h5>
							<p class="faint">These options replace data that was already built: {check.rewriteHistory.reasons.join(', ')}</p>
						</div>
					{/if}
					{#if check.claimConflicts.length}
						<div class="block" class:stale={!fresh}>
							<h5 title="Write claims are held by other runs">Will wait <span>other runs are updating the same data</span></h5>
							{#each check.claimConflicts.slice(0, 6) as c (c.runId + c.nodeKey)}
								<p class="faint">Run <a href="/runs/{c.runId}" onclick={() => (open = false)}>{c.runId.replace(/^run-/, '')}</a> is updating <code>{c.nodeKey}</code></p>
							{/each}
						</div>
					{/if}
					{#each check.problems as p (p.code + p.message)}<p class="problem">{p.message}</p>{/each}
				{:else if !checkError}
					<p class="faint">Checking the request…</p>
				{/if}
			</div>

			<div class="flags sb-scroll">
				<div class="section-title">Options · sqb {verb}</div>
				{#each spec.flags.filter((f) => f.flag !== '--select' && f.flag !== '--exclude') as f (f.flag)}
					<button onclick={() => insertFlag(f.flag)} title={f.description}>
						<code class:rewrite={f.rewritesHistory}>{f.flag}</code>
						<span>{f.description}</span>
					</button>
				{/each}
				<p class="faint note">
					The same options as the command line. Type <code>--</code> for options, or a node name, <code>tag:</code>, <code>group:</code>, <code>kind:</code> or <code>path:</code> after <code>--select</code>; <code>+</code> before or after adds upstream or downstream nodes.
				</p>
			</div>
		</div>

		<div class="foot">
			{#if acOpen && suggestions.length}
				<div class="ac">
					{#each suggestions as s, i (s.value)}
						<button class:on={i === acIndex} onmouseenter={() => (acIndex = i)} onmousedown={(e) => (e.preventDefault(), pick(s))}>
							<code>{s.label}</code><span>{s.hint}</span>
						</button>
					{/each}
				</div>
			{/if}
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
					placeholder="sqb build --select +fact_orders"
				/>
				<span class="locked" title="Set by the location you're in"><LockIcon size={11} /> --target {app.target?.name}</span>
				<span title={canLaunch ? undefined : blockedReason}>
					<GatedButton requires="run.adhoc" variant="default" disabled={!canLaunch} reason={blockedReason} busy={launching} onclick={launch}>▷ Run</GatedButton>
				</span>
			</div>
			{#if launchError}<p class="problem">{launchError}</p>{:else if !canLaunch && blockedReason && fresh}<p class="faint hint">{blockedReason}</p>{/if}
		</div>
	</div>
{/if}

<style>
	.panel {
		position: fixed;
		inset: auto 0 0 0;
		z-index: 50;
		display: flex;
		max-height: 82vh;
		flex-direction: column;
		border-top: 1px solid var(--border);
		background: var(--card);
		box-shadow: var(--sb-elev);
	}
	.head {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 24px;
		border-bottom: 1px solid var(--border);
		font-size: 12px;
	}
	.verbs {
		display: flex;
		overflow: hidden;
		border: 1px solid var(--border);
		border-radius: 2px;
	}
	.verbs button {
		padding: 4px 12px;
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
	.target {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		margin-left: auto;
		color: var(--sb-text-faint);
	}
	.target b {
		color: var(--foreground);
		font-weight: 500;
	}
	.close {
		color: var(--muted-foreground);
	}
	.body {
		display: grid;
		min-height: 0;
		flex: 1;
		grid-template-columns: 1fr 400px;
	}
	.check,
	.flags {
		overflow: auto;
		padding: 14px 24px;
	}
	.check {
		border-right: 1px solid var(--border);
	}
	.section-title {
		display: flex;
		gap: 10px;
		margin-bottom: 8px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	.block {
		margin-bottom: 12px;
		transition: opacity 0.15s;
	}
	.block.stale {
		opacity: 0.55;
	}
	h5 {
		display: flex;
		align-items: baseline;
		gap: 8px;
		margin-bottom: 5px;
		font-size: 12.5px;
		font-weight: 500;
	}
	h5 span {
		color: var(--muted-foreground);
		font-weight: 400;
	}
	.chips {
		display: flex;
		max-height: 84px;
		flex-wrap: wrap;
		gap: 5px;
		overflow: auto;
	}
	.sb-tag.bad {
		background: color-mix(in srgb, var(--sb-error) 14%, transparent);
		color: var(--sb-error);
	}
	code {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.faint {
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.ok-text {
		color: var(--sb-success) !important;
	}
	.bad-text {
		color: var(--sb-error) !important;
	}
	.problem {
		margin: 4px 0;
		color: var(--sb-error);
		font-size: 12px;
	}
	.check a {
		color: var(--primary);
	}
	.flags button {
		display: flex;
		width: 100%;
		align-items: baseline;
		gap: 8px;
		padding: 3px 8px;
		border-radius: 2px;
		text-align: left;
	}
	.flags button:hover {
		background: var(--sb-hover);
	}
	.flags button code {
		flex-shrink: 0;
		color: var(--primary);
	}
	.flags button code.rewrite {
		color: var(--sb-warning);
	}
	.flags button span {
		overflow: hidden;
		color: var(--sb-text-faint);
		font-size: 11px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.note {
		margin-top: 12px;
		font-size: 11px;
		line-height: 1.6;
	}
	.foot {
		position: relative;
		padding: 12px 24px;
		border-top: 1px solid var(--border);
	}
	.line {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.line input {
		flex: 1;
		background: transparent;
		color: var(--foreground);
		font-family: var(--font-mono);
		font-size: 12.5px;
		outline: none;
	}
	.locked {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 2px 7px;
		border-radius: 2px;
		background: var(--sb-hover);
		color: var(--sb-text-faint);
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.hint {
		margin-top: 5px;
		padding-left: 18px;
	}
	.ac {
		position: absolute;
		bottom: 100%;
		left: 24px;
		z-index: 60;
		width: 520px;
		max-height: 260px;
		overflow: auto;
		margin-bottom: 4px;
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
	.sb-scroll {
		scrollbar-width: thin;
		scrollbar-color: var(--border-strong) transparent;
	}
</style>
