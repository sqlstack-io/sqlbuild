<script lang="ts">
	import type { Account, RoleAssignment } from '$lib/api';
	import { to } from '$lib/paths';
	import { personHref } from '$lib/platform/access';
	import { errorMessage } from '$lib/platform/labels';
	import { changeRole } from './api';
	import type { BuiltinRole } from './types';
	import { BUILTIN_ROLES, roleChange, roleChoice } from './utils';

	// A person's one role everywhere, changed inline; roles that differ by location are "Mixed" and managed on their page.
	type Props = { account: Account; assignments: RoleAssignment[]; defaultRole: string | null; lastAdmin: boolean; canAssign: boolean; onchange: () => void };
	let { account, assignments, defaultRole, lastAdmin, canAssign, onchange }: Props = $props();

	const choice = $derived(roleChoice(assignments, defaultRole));
	const LAST_ADMIN = 'The last admin can’t be demoted; make someone else an admin first.';
	let busy = $state(false);
	let error = $state<string | null>(null);

	async function pick(e: Event & { currentTarget: HTMLSelectElement }) {
		const select = e.currentTarget;
		const role = select.value as BuiltinRole | 'none';
		if (choice.kind !== 'single' || role === choice.role) return;
		if (role === 'admin' && !confirm(`Make ${account.displayName} (${account.name}) an admin? Admins can manage people, roles, locations and the audit log everywhere.`)) {
			select.value = choice.role;
			return;
		}
		busy = true;
		error = null;
		try {
			await changeRole(account.name, roleChange(choice, role, defaultRole));
		} catch (err) {
			error = errorMessage(err);
			select.value = choice.role;
		} finally {
			busy = false;
			onchange();
		}
	}
</script>

{#if choice.kind === 'mixed'}
	<a class="mixed" href={to(personHref(account))} title="Different roles in different locations; change them on their page" data-testid="role-mixed">Mixed</a>
{:else if choice.kind === 'locked'}
	<span class="locked" title="From {choice.from}; change it there">{choice.role} <span class="from">· {choice.from}</span></span>
{:else if !canAssign || account.disabled}
	<span>{choice.role === 'none' ? 'No role' : choice.role}</span>
{:else}
	<span class="pick" data-control>
		<select aria-label="Role of {account.displayName}" value={choice.role} disabled={busy} onchange={pick} data-testid="role-select">
			{#if defaultRole === null}<option value="none">No role</option>{/if}
			{#each BUILTIN_ROLES as role (role)}
				<option value={role} disabled={lastAdmin && role !== 'admin'} title={lastAdmin && role !== 'admin' ? LAST_ADMIN : undefined}>{role}{role === defaultRole ? ' (default)' : ''}</option>
			{/each}
		</select>
		{#if lastAdmin}<span class="hint" title={LAST_ADMIN}>last admin</span>{/if}
	</span>
	{#if error}<div class="err" role="alert">{error}</div>{/if}
{/if}

<style>
	.mixed {
		color: var(--primary);
		font-size: 12.5px;
	}
	.locked .from,
	.hint {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.pick {
		display: inline-flex;
		align-items: center;
		gap: 8px;
	}
	select {
		height: 26px;
		padding: 0 6px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--foreground);
		font-size: 12.5px;
	}
	.err {
		margin-top: 3px;
		color: var(--sb-error);
		font-size: 12px;
	}
</style>
