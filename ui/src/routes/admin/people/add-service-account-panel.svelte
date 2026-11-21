<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import type { BuiltInRole, IssuedToken, ServiceAccount } from '$lib/api';
	import Button from '$lib/components/ui/button/button.svelte';
	import { errorMessage } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { BUILTIN_ROLES, nameProblem } from './utils';

	// A service account is added with a name and one role everywhere; its first key comes back once and shows in its panel.
	type Props = { onadded: (result: IssuedToken<ServiceAccount>) => void; onclose: () => void };
	let { onadded, onclose }: Props = $props();

	let name = $state('');
	let description = $state('');
	// The least access by default; the person adding it raises it deliberately.
	let role = $state<BuiltInRole>('viewer');
	let busy = $state(false);
	let error = $state<string | null>(null);
	const problem = $derived(nameProblem(name));

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (problem) return;
		if (role === 'admin' && !confirm(`Make ${name.trim()} an admin? Anyone holding its key can then manage people, roles, locations and the audit log everywhere.`)) return;
		busy = true;
		error = null;
		try {
			onadded(await app.client.createServiceAccount({ name: name.trim(), description: description.trim() || null, role }));
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<aside class="panel sb-scroll" aria-label="Add service account" data-testid="add-service-account-panel">
	<header>
		<div class="title">Add service account</div>
		<Button variant="ghost" size="icon-sm" aria-label="Close" onclick={onclose}><XIcon size={15} /></Button>
	</header>
	<form onsubmit={submit}>
		<label>
			Name
			<input name="name" autocomplete="off" required placeholder="orders-deploy" bind:value={name} aria-invalid={problem ? 'true' : undefined} />
			{#if problem}<span class="err">{problem}</span>{:else}<span class="hint">Shown in the audit log, e.g. “Republished, by orders-deploy”. Can't be changed later.</span>{/if}
		</label>
		<label>
			What uses it <span class="optional">optional</span>
			<input name="description" autocomplete="off" placeholder="Release pipeline: republishes after each deploy" bind:value={description} />
		</label>
		<label>
			Role everywhere
			<select name="role" bind:value={role}>
				{#each BUILTIN_ROLES as r (r)}<option value={r}>{r}</option>{/each}
			</select>
			<span class="hint">Republishing needs developer or above. Give it only what the script does.</span>
		</label>
		<p class="hint">Adding it issues its API key, shown once on the next screen.</p>
		{#if error}<p class="err" role="alert">{error}</p>{/if}
		<div class="actions">
			<Button type="submit" size="sm" disabled={busy || !!problem}>{busy ? 'Adding…' : 'Add and issue key'}</Button>
			<Button type="button" size="sm" variant="ghost" onclick={onclose}>Cancel</Button>
		</div>
		<p class="hint cli">From a terminal: <code>sqb admin create-service-account {name.trim() || '<name>'} --role {role}</code></p>
	</form>
</aside>

<style>
	.panel {
		display: flex;
		width: 380px;
		min-height: 0;
		flex-shrink: 0;
		flex-direction: column;
		gap: 16px;
		overflow-y: auto;
		padding: 14px 18px 24px;
		border-left: 1px solid var(--border);
		background: var(--card);
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}
	.title {
		font-family: var(--font-display);
		font-size: 16px;
		font-weight: 600;
	}
	form {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 12.5px;
	}
	.optional {
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	input,
	select {
		padding: 6px 10px;
		border: 1px solid var(--border);
		border-radius: var(--radius-md, 6px);
		background: transparent;
		font-size: 13px;
	}
	.hint {
		margin: 0;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.cli {
		overflow-wrap: anywhere;
	}
	code {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.err {
		margin: 0;
		color: var(--sb-error);
		font-size: 12px;
	}
	.actions {
		display: flex;
		gap: 8px;
	}
</style>
