<script lang="ts">
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
	import CircleXIcon from '@lucide/svelte/icons/circle-x';
	import Button from '$lib/components/ui/button/button.svelte';
	import { CAPABILITIES, type AccessExplanation, type Account, type Capability } from '$lib/api';
	import { errorMessage, formatAssignmentScope } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';

	/** Principals to choose from; when empty, only the current user can be explained. */
	let { principals = [] }: { principals?: Account[] } = $props();

	let principal = $state('');
	let action = $state<Capability>('job.run');
	let locationId = $state(app.locationId);
	let subject = $state('');
	let running = $state(false);
	let result = $state<AccessExplanation | null>(null);
	let failure = $state<string | null>(null);

	const locations = $derived(app.bootstrap?.locations ?? []);
	const who = $derived(principal || app.user?.name || '');

	async function explain() {
		running = true;
		failure = null;
		try {
			result = await app.client.explainAccess({ principal: who, action, locationId, subject: subject.trim() || null });
		} catch (error) {
			result = null;
			failure = errorMessage(error);
		} finally {
			running = false;
		}
	}
</script>

<div class="explain">
	<div class="form">
		<label>
			<span>Who</span>
			{#if principals.length}
				<select bind:value={principal}>
					<option value="">{app.user?.name} (me)</option>
					{#each principals.filter((p) => p.name !== app.user?.name) as p (p.name)}
						<option value={p.name}>{p.name}</option>
					{/each}
				</select>
			{:else}
				<input value={app.user?.name ?? ''} disabled />
			{/if}
		</label>
		<label>
			<span>Action</span>
			<select bind:value={action}>
				{#each CAPABILITIES as c (c)}<option value={c}>{c}</option>{/each}
			</select>
		</label>
		<label>
			<span>Location</span>
			<select bind:value={locationId}>
				{#each locations as l (l.id)}<option value={l.id}>{l.name} · {l.target.name}</option>{/each}
			</select>
		</label>
		<label class="grow">
			<span>On (optional)</span>
			<input bind:value={subject} placeholder="Job, group or node, e.g. daily_revenue" />
		</label>
		<Button size="sm" disabled={running || !who} onclick={explain}>{running ? 'Explaining…' : 'Explain'}</Button>
	</div>

	{#if failure}<p class="err">{failure}</p>{/if}

	{#if result}
		<div class="result" class:allowed={result.allowed}>
			<div class="headline">
				{#if result.allowed}<CircleCheckIcon size={16} />{:else}<CircleXIcon size={16} />{/if}
				<span>{result.summary}</span>
			</div>
			{#if result.grantedBy.length}
				<div class="group-title" title="Permissions add up across all their roles">Allowed by</div>
				<ul>
					{#each result.grantedBy as g (g.assignmentId)}
						<li>
							<b>{g.role}</b> · {formatAssignmentScope(g.scope, locations)} ·
							{g.assignmentSource === 'sso' ? `SSO group ${g.ssoGroup}` : 'manual assignment'}{#if g.nodes}&nbsp;· on <code>{g.nodes.join(' ')}</code>{/if}
						</li>
					{/each}
				</ul>
			{/if}
			{#if !result.allowed && result.wouldBeGrantedBy.length}
				<div class="group-title">Roles that would allow it</div>
				<p>{result.wouldBeGrantedBy.join(', ')}</p>
			{/if}
			{#if result.ignored.length}
				<div class="group-title">Their roles that don't apply here</div>
				<ul>
					{#each result.ignored as i (i.assignmentId)}
						<li><b>{i.role}</b> · {i.reason}</li>
					{/each}
				</ul>
			{/if}
		</div>
	{/if}
</div>

<style>
	.form {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		gap: 10px;
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
	input:disabled {
		color: var(--muted-foreground);
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
		align-items: center;
		gap: 8px;
		font-weight: 500;
	}
	.headline :global(svg) {
		flex: 0 0 auto;
		color: var(--tone);
	}
	.group-title {
		margin: 10px 0 4px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	ul {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	b {
		font-weight: 500;
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
</style>
