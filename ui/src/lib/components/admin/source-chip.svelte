<script lang="ts">
	import LockIcon from '@lucide/svelte/icons/lock';
	import type { RoleAssignment } from '$lib/api';
	import { lockedHint, sourceChip } from '$lib/platform/access';

	// Where an assignment comes from. Derived ones (SSO, policy file, built-in) are locked.
	let { assignment }: { assignment: Pick<RoleAssignment, 'source' | 'ssoGroup' | 'definedIn'> } = $props();
	const hint = $derived(lockedHint(assignment));
</script>

<span class="chip {assignment.source}" title={hint ?? 'Given in this UI; can be removed here'}>
	{#if hint}<LockIcon size={10} />{/if}
	{sourceChip(assignment)}
	{#if assignment.source === 'policy' && assignment.definedIn}<code>{assignment.definedIn}</code>{/if}
</span>

<style>
	.chip {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 1px 6px;
		border-radius: 2px;
		background: var(--sb-hover);
		color: var(--muted-foreground);
		font-size: 11.5px;
		white-space: nowrap;
	}
	.chip.manual {
		color: var(--foreground);
	}
	.chip.sso {
		background: color-mix(in srgb, var(--primary) 10%, transparent);
	}
	code {
		font-family: var(--font-mono);
		font-size: 11px;
	}
</style>
