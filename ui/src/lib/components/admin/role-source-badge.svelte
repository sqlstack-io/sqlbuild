<script lang="ts">
	import type { RoleDefinition } from '$lib/api';
	import { roleSourceLabel } from '$lib/platform/access';

	let { role }: { role: Pick<RoleDefinition, 'source' | 'definedIn'> } = $props();
</script>

<span class="badge {role.source}">{roleSourceLabel[role.source]}</span>
{#if role.definedIn}<code>{role.definedIn}</code>{/if}

<style>
	.badge {
		display: inline-flex;
		margin-right: 6px;
		padding: 1px 6px;
		border-radius: 2px;
		background: var(--sb-hover);
		color: var(--muted-foreground);
		font-size: 11.5px;
		white-space: nowrap;
	}
	.badge.project {
		background: color-mix(in srgb, var(--primary) 10%, transparent);
		color: var(--foreground);
	}
	code {
		color: var(--muted-foreground);
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
</style>
