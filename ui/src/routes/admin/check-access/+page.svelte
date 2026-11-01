<script lang="ts">
	import { page } from '$app/state';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import AccessChecker from '$lib/components/admin/access-checker.svelte';
	import { CAPABILITIES } from '$lib/api';
	import { isCapability, type CheckInput } from '$lib/platform/access';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	const canCheckOthers = $derived(app.can('role.assign') || app.can('audit.view'));
	const accounts = createResource((client) => (app.can('user.manage') ? client.listAccounts() : Promise.resolve([])));

	const initial = $derived.by((): CheckInput | null => {
		const q = page.url.searchParams;
		const action = q.get('action');
		const input: CheckInput = {
			principal: q.get('principal') ?? undefined,
			action: isCapability(action, CAPABILITIES) ? action : undefined,
			locationId: q.get('location') ?? undefined,
			subject: q.get('subject') ?? undefined,
			decision: q.get('decision') ?? undefined
		};
		return Object.values(input).some(Boolean) ? input : null;
	});
</script>

<AppTopbar title="Check access" />

<div class="min-h-0 flex-1 overflow-auto px-6 pt-5 pb-8">
	<p class="lede">
		Can someone do something, and why? Pick a person or service account, an action and where. Or paste the denial ID from a “permission denied” message to check that request again.
		{#if !canCheckOthers}You can check your own access; checking others needs “Give roles” or “See the audit log”.{/if}
	</p>
	<div class="max-w-[1100px]">
		<AccessChecker principals={canCheckOthers ? (accounts.data ?? []) : []} {initial} />
	</div>
	<p class="back"><a href="/admin/people">← People</a></p>
</div>

<style>
	.lede {
		max-width: 860px;
		margin-bottom: 18px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.back {
		margin-top: 24px;
		font-size: 12.5px;
	}
	a {
		color: var(--primary);
	}
</style>
