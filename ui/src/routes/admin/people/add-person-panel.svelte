<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import type { Account } from '$lib/api';
	import MenuSelect from '$lib/components/platform/menu-select.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import { errorMessage } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import type { BuiltinRole } from './types';
	import { BUILTIN_ROLES } from './utils';

	// Under password sign-in, a person is added here with their first password; they change it after signing in.
	type Props = { defaultRole: string | null; onadded: (account: Account) => void; onclose: () => void };
	let { defaultRole, onadded, onclose }: Props = $props();

	const MIN_PASSWORD = 12;
	const canAssign = $derived(app.can('role.assign'));
	let name = $state('');
	let password = $state('');
	let role = $state<BuiltinRole | 'default'>('default');
	const roleOptions = $derived([
		{ value: 'default' as const, label: defaultRole ? `Default role only (${defaultRole})` : 'No role' },
		...BUILTIN_ROLES.map((r) => ({ value: r as BuiltinRole | 'default', label: r }))
	]);
	let busy = $state(false);
	let error = $state<string | null>(null);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = null;
		try {
			onadded(await app.client.createPasswordAccount({ name: name.trim(), password, role: role === 'default' ? null : role }));
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<aside class="panel sb-scroll" aria-label="Add person" data-testid="add-person-panel">
	<header>
		<div class="title">Add person</div>
		<Button variant="ghost" size="icon-sm" aria-label="Close" onclick={onclose}><XIcon size={15} /></Button>
	</header>
	<form onsubmit={submit}>
		<label>
			Sign-in name
			<input name="name" autocomplete="off" required bind:value={name} />
		</label>
		<label>
			First password
			<input name="password" type="password" autocomplete="new-password" required minlength={MIN_PASSWORD} bind:value={password} />
			<span class="hint">At least {MIN_PASSWORD} characters. Share it with them; they can change it after signing in.</span>
		</label>
		{#if canAssign}
			<div class="field">
				Role everywhere
				<MenuSelect value={role} options={roleOptions} onchange={(r) => (role = r)} label="Role everywhere" block testid="new-person-role" />
			</div>
		{/if}
		{#if error}<p class="err" role="alert">{error}</p>{/if}
		<div class="actions">
			<Button type="submit" size="sm" disabled={busy}>{busy ? 'Adding…' : 'Add person'}</Button>
			<Button type="button" size="sm" variant="ghost" onclick={onclose}>Cancel</Button>
		</div>
	</form>
</aside>

<style>
	.panel {
		display: flex;
		width: 360px;
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
	label,
	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 12.5px;
	}
	input {
		padding: 6px 10px;
		border: 1px solid var(--border);
		border-radius: var(--radius-md, 6px);
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
		font-size: 12.5px;
	}
	.actions {
		display: flex;
		gap: 8px;
	}
</style>
