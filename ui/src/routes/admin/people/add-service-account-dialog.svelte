<script lang="ts">
	import type { BuiltInRole } from '$lib/api';
	import MenuSelect from '$lib/components/platform/menu-select.svelte';
	import Modal from '$lib/components/platform/modal.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import { errorMessage } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import KeyReveal from './key-reveal.svelte';
	import { BUILTIN_ROLES, nameProblem } from './utils';

	// Add in a dialog: a name, what uses it and its role; then its first key is shown here once.
	type Props = { open: boolean; onadded: (name: string) => void; onclose: () => void };
	let { open, onadded, onclose }: Props = $props();

	const ROLE_OPTIONS = BUILTIN_ROLES.map((r) => ({ value: r, label: r }));
	let name = $state('');
	let description = $state('');
	// The least access by default; whoever adds it raises it deliberately.
	let role = $state<BuiltInRole>('viewer');
	let busy = $state(false);
	let error = $state<string | null>(null);
	let secret = $state<string | null>(null);
	let added = $state('');
	const problem = $derived(nameProblem(name));

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (problem || !name.trim()) return;
		busy = true;
		error = null;
		try {
			const result = await app.client.createServiceAccount({ name: name.trim(), description: description.trim() || null, role });
			added = result.token.name;
			secret = result.secret;
			onadded(added);
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	function close() {
		name = '';
		description = '';
		role = 'viewer';
		error = null;
		secret = null;
		onclose();
	}
</script>

<Modal {open} title={secret ? `${added} is ready` : 'Add service account'} description={secret ? 'Here is its API key.' : 'For a script or system that calls SQLBuild, such as a deploy that republishes after each release.'} onclose={close} testid="add-service-account-dialog">
	{#if secret}
		<KeyReveal {secret} />
	{:else}
		<form id="add-service-account" onsubmit={submit}>
			<label>
				<span>Name</span>
				<input name="name" autocomplete="off" required placeholder="orders-deploy" bind:value={name} aria-invalid={problem ? 'true' : undefined} />
				{#if problem}<span class="err">{problem}</span>{:else}<span class="hint">Shown in the audit log. Can't be changed later.</span>{/if}
			</label>
			<label>
				<span>What uses it <span class="hint">optional</span></span>
				<input name="description" autocomplete="off" placeholder="Release pipeline" bind:value={description} />
			</label>
			<div class="field">
				<span>Role everywhere</span>
				<MenuSelect value={role} options={ROLE_OPTIONS} onchange={(v) => (role = v)} label="Role" block testid="new-service-role" />
				<span class="hint">Give it only what the script does. Republishing needs developer.</span>
			</div>
			{#if error}<p class="err" role="alert">{error}</p>{/if}
		</form>
	{/if}
	{#snippet footer()}
		{#if secret}
			<Button size="sm" onclick={close}>Done</Button>
		{:else}
			<Button size="sm" variant="ghost" onclick={close}>Cancel</Button>
			<Button size="sm" type="submit" form="add-service-account" disabled={busy || !!problem || !name.trim()}>{busy ? 'Adding…' : 'Add and create key'}</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	form {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}
	label,
	.field {
		display: flex;
		flex-direction: column;
		gap: 5px;
		font-size: 12.5px;
	}
	input {
		height: 32px;
		padding: 0 10px;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		background: transparent;
		font-size: 13px;
	}
	.hint {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.err {
		margin: 0;
		color: var(--sb-error);
		font-size: 12px;
	}
</style>
