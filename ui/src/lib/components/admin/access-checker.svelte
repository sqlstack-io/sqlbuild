<script lang="ts">
	import { to } from '$lib/paths';
	import { untrack } from 'svelte';
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
	import CircleXIcon from '@lucide/svelte/icons/circle-x';
	import Button from '$lib/components/ui/button/button.svelte';
	import type { AccessDecision, AccessExplanation, Account, Capability } from '$lib/api';
	import { builtInLabel, roleHref, scopeWords, type CheckInput } from '$lib/platform/access';
	import { errorMessage, locationName } from '$lib/platform/labels';
	import { PERMISSIONS, PERMISSIONS_BY_RISK, RISK_LABEL } from '$lib/platform/permission-catalog';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import PermissionName from './permission-name.svelte';
	import SourceChip from './source-chip.svelte';

	type Props = {
		/** Accounts to choose from; when empty, only the current user can be checked. */
		principals?: Account[];
		/** Pre-filled values; checked straight away when they include an action or a decision ID. */
		initial?: CheckInput | null;
		/** Keep the account fixed (person pages). */
		lockPrincipal?: boolean;
		/** Sample decision ID offered as "Try it". */
		sampleDecision?: string | null;
	};
	let { principals = [], initial = null, lockPrincipal = false, sampleDecision = null }: Props = $props();

	let principal = $state('');
	let action = $state<Capability>('job.run');
	let locationId = $state(app.locationId);
	let subject = $state('');
	let decisionId = $state('');
	let running = $state(false);
	let result = $state<AccessExplanation | null>(null);
	let decision = $state<AccessDecision | null>(null);
	let failure = $state<string | null>(null);

	const locations = $derived(app.bootstrap?.locations ?? []);
	const who = $derived(principal || app.user?.name || '');
	const whoName = (name: string) => principals.find((p) => p.name === name)?.displayName ?? (name === app.user?.name ? app.user.displayName : name);
	const canSeeRoles = $derived(app.can('role.assign'));

	$effect(() => {
		const input = initial;
		if (!input) return;
		untrack(() => {
			if (input.principal) principal = input.principal === app.user?.name ? '' : input.principal;
			if (input.action) action = input.action;
			if (input.locationId) locationId = input.locationId;
			subject = input.subject ?? '';
			decisionId = input.decision ?? '';
			if (input.decision) void lookup();
			else if (input.action) void check();
		});
	});

	async function check() {
		running = true;
		failure = null;
		try {
			result = await app.client.explainAccess({ principal: who, action, locationId: locationId || app.locationId, subject: subject.trim() || null });
		} catch (error) {
			result = null;
			failure = errorMessage(error);
		} finally {
			running = false;
		}
	}

	async function lookup() {
		const id = decisionId.trim();
		if (!id) return;
		running = true;
		failure = null;
		try {
			decision = await app.client.lookupDecision(id);
			const q = decision.query;
			principal = q.principal === app.user?.name ? '' : q.principal;
			action = q.action;
			locationId = q.locationId;
			subject = q.subject ?? '';
		} catch (error) {
			decision = null;
			result = null;
			failure = errorMessage(error);
			running = false;
			return;
		}
		running = false;
		await check();
	}

	function clearDecision() {
		decision = null;
		decisionId = '';
	}
</script>

<div class="checker">
	<div class="denial">
		<label class="grow">
			<span>Denial ID <i>(optional; from a “permission denied” message)</i></span>
			<input bind:value={decisionId} placeholder="e.g. dec-4k7q2m" spellcheck="false" onkeydown={(e) => e.key === 'Enter' && lookup()} />
		</label>
		<Button size="sm" variant="outline" disabled={running || !decisionId.trim()} onclick={lookup}>Look up</Button>
		{#if sampleDecision && !decisionId}
			<button class="link" onclick={() => ((decisionId = sampleDecision ?? ''), lookup())}>Try {sampleDecision}</button>
		{/if}
	</div>
	{#if decision}
		<p class="decision">
			<b>{decision.decisionId}</b>: {whoName(decision.query.principal)} was denied <PermissionName action={decision.query.action} /> {formatAgo(decision.at, app.now)}
			<span class="faint" title={formatStamp(decision.at)}>· “{decision.summary}”</span>. Checked again below with their access today.
			<button class="link" onclick={clearDecision}>Clear</button>
		</p>
	{/if}

	<div class="form">
		<label>
			<span>Who</span>
			{#if principals.length && !lockPrincipal}
				<select bind:value={principal}>
					<option value="">{app.user?.displayName} (me)</option>
					{#each principals.filter((p) => p.name !== app.user?.name) as p (p.name)}
						<option value={p.name}>{p.displayName} · {p.name}</option>
					{/each}
				</select>
			{:else}
				<input value={whoName(who)} disabled />
			{/if}
		</label>
		<label>
			<span>Can they</span>
			<select bind:value={action}>
				{#each PERMISSIONS_BY_RISK as group (group.risk)}
					<optgroup label={RISK_LABEL[group.risk]}>
						{#each group.actions as c (c)}<option value={c}>{PERMISSIONS[c].label} ({c})</option>{/each}
					</optgroup>
				{/each}
			</select>
		</label>
		<label>
			<span>In</span>
			<select bind:value={locationId}>
				{#each locations as l (l.id)}<option value={l.id}>{l.name}</option>{/each}
			</select>
		</label>
		<label class="grow">
			<span>On <i>(optional)</i></span>
			<input bind:value={subject} placeholder="Job, group, node or selector, e.g. daily_revenue, tag:finance" onkeydown={(e) => e.key === 'Enter' && check()} />
		</label>
		<Button size="sm" disabled={running || !who} onclick={check}>{running ? 'Checking…' : 'Check'}</Button>
	</div>

	{#if failure}<p class="err">{failure}</p>{/if}

	{#if result}
		<div class="result" class:allowed={result.allowed}>
			<div class="headline">
				{#if result.allowed}<CircleCheckIcon size={16} />{:else}<CircleXIcon size={16} />{/if}
				<span>
					<b>{result.allowed ? 'Yes' : 'No'}</b>: {whoName(result.query.principal)} {result.allowed ? 'can' : 'can’t'}
					<PermissionName action={result.query.action} risk />
					{#if result.query.subject}on <code>{result.query.subject}</code>{/if} in {locationName(locations, result.query.locationId)}.
				</span>
			</div>

			{#if result.grantedBy.length}
				<div class="group-title" title="Permissions add up across all their roles">Because</div>
				<ol class="chains">
					{#each result.grantedBy as g (g.assignmentId + g.role)}
						<li>
							<span class="step"><SourceChip assignment={{ source: g.assignmentSource, ssoGroup: g.ssoGroup, definedIn: g.definedIn }} /> gives</span>
							<ArrowRightIcon size={12} />
							<span class="step">role {#if canSeeRoles}<a href={to(roleHref(g.role))}>{g.role}</a>{:else}<b>{g.role}</b>{/if}</span>
							<ArrowRightIcon size={12} />
							<span class="step">
								{#if g.via === 'base' && g.baseRole && g.baseRole !== g.role}which includes {builtInLabel(g.baseRole)}{:else if g.via === 'base'}a built-in role{:else}an addition{/if}
								{g.selector ? 'on' : 'on every node'}
								{#if g.selector}<code>{g.selector}</code>{/if}
							</span>
							<ArrowRightIcon size={12} />
							<span class="step">in {scopeWords(g.scope.locationId, locations)}</span>
						</li>
					{/each}
				</ol>
			{/if}

			{#if !result.allowed}
				{#if result.nearMisses.length}
					<div class="group-title">Nearest miss</div>
					<ul>
						{#each result.nearMisses as m, i (i)}<li>{m.detail}</li>{/each}
					</ul>
				{/if}
				{#if result.wouldBeGrantedBy.length}
					<div class="group-title">Roles that would allow it here</div>
					<p>
						{#each result.wouldBeGrantedBy as r, i (r)}{#if i},&nbsp;{/if}{#if canSeeRoles}<a href={to(roleHref(r))}>{r}</a>{:else}{r}{/if}{/each}
					</p>
				{/if}
				{#if !result.nearMisses.length && !result.wouldBeGrantedBy.length}
					<p class="faint">No role in this location allows it.</p>
				{/if}
			{/if}
		</div>
	{/if}
</div>

<style>
	.form,
	.denial {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		gap: 10px;
	}
	.denial {
		margin-bottom: 12px;
		max-width: 620px;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 12px;
	}
	label span {
		color: var(--muted-foreground);
	}
	label i {
		color: var(--sb-text-faint);
		font-style: normal;
	}
	label.grow {
		min-width: 240px;
		flex: 1;
	}
	select,
	input {
		height: 32px;
		padding: 0 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--foreground);
		font-size: 12.5px;
	}
	select {
		max-width: 300px;
	}
	input:disabled {
		color: var(--muted-foreground);
	}
	.decision {
		margin-bottom: 12px;
		font-size: 12.5px;
	}
	.err {
		margin-top: 10px;
		color: var(--sb-error);
		font-size: 12.5px;
	}
	.result {
		--tone: var(--sb-error);
		margin-top: 14px;
		padding: 12px 14px;
		border: 1px solid color-mix(in srgb, var(--tone) 40%, var(--border));
		border-radius: 2px;
		background: color-mix(in srgb, var(--tone) 6%, var(--background));
		font-size: 12.5px;
	}
	.result.allowed {
		--tone: var(--sb-success);
	}
	.headline {
		display: flex;
		align-items: flex-start;
		gap: 8px;
	}
	.headline :global(svg) {
		flex: 0 0 auto;
		margin-top: 1px;
		color: var(--tone);
	}
	.group-title {
		margin: 12px 0 5px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.chains,
	ul {
		display: flex;
		flex-direction: column;
		gap: 5px;
	}
	ul {
		padding-left: 16px;
		list-style: disc;
	}
	.chains li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
	}
	.chains :global(svg) {
		color: var(--sb-text-faint);
	}
	a,
	.link {
		color: var(--primary);
	}
	.link {
		font-size: 12px;
	}
	b {
		font-weight: 600;
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.faint {
		color: var(--sb-text-faint);
	}
</style>
