<script lang="ts">
	import type { Account, RoleAssignment } from '$lib/api';
	import ConfirmDialog from '$lib/components/platform/confirm-dialog.svelte';
	import MenuSelect from '$lib/components/platform/menu-select.svelte';
	import { to } from '$lib/paths';
	import { personHref } from '$lib/platform/access';
	import { errorMessage } from '$lib/platform/labels';
	import { changeRole } from './api';
	import type { BuiltinRole } from './types';
	import { BUILTIN_ROLES, roleChange, roleChoice } from './utils';

	// A person's one role everywhere, changed inline; roles that differ by location are "Mixed" and managed on their page.
	type Props = { account: Account; assignments: RoleAssignment[]; defaultRole: string | null; lastAdmin: boolean; canAssign: boolean; onchange: () => void };
	let { account, assignments, defaultRole, lastAdmin, canAssign, onchange }: Props = $props();

	type Choice = BuiltinRole | 'none';
	const choice = $derived(roleChoice(assignments, defaultRole));
	const LAST_ADMIN = 'The last admin can’t be demoted; make someone else an admin first.';
	const options = $derived([
		...(defaultRole === null ? [{ value: 'none' as Choice, label: 'No role', disabled: lastAdmin, title: lastAdmin ? LAST_ADMIN : undefined }] : []),
		...BUILTIN_ROLES.map((role) => ({
			value: role as Choice,
			label: `${role}${role === defaultRole ? ' (default)' : ''}`,
			disabled: lastAdmin && role !== 'admin',
			title: lastAdmin && role !== 'admin' ? LAST_ADMIN : undefined
		}))
	]);
	let busy = $state(false);
	let error = $state<string | null>(null);
	/** A promotion to admin waiting for its confirmation. */
	let promoting = $state(false);

	async function apply(role: Choice) {
		if (choice.kind !== 'single') return;
		busy = true;
		error = null;
		try {
			await changeRole(account.name, roleChange(choice, role, defaultRole));
		} finally {
			busy = false;
			onchange();
		}
	}

	function pick(role: Choice) {
		if (choice.kind !== 'single' || role === choice.role) return;
		if (role === 'admin') {
			promoting = true;
			return;
		}
		apply(role).catch((e) => (error = errorMessage(e)));
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
		<MenuSelect value={choice.role as Choice} {options} onchange={pick} disabled={busy} label="Role of {account.displayName}" testid="role-select" />
		{#if lastAdmin}<span class="hint" title={LAST_ADMIN}>last admin</span>{/if}
	</span>
	{#if error}<div class="err" role="alert">{error}</div>{/if}
	{#if promoting}
		<ConfirmDialog
			open
			title="Make {account.displayName} an admin?"
			body="{account.displayName} ({account.name}) can then manage people, roles, locations and the audit log everywhere."
			confirm="Make admin"
			destructive
			onconfirm={() => apply('admin')}
			onclose={() => (promoting = false)}
		/>
	{/if}
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
	.err {
		margin-top: 3px;
		color: var(--sb-error);
		font-size: 12px;
	}
</style>
