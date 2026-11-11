<script lang="ts">
	import Loading from '$lib/components/loading.svelte';
	import type { IssuedToken, LocationToken } from '$lib/api';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { errorMessage, tokenLabel, tokenTone } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import GatedButton from './gated-button.svelte';
	import IssuedSecret from './issued-secret.svelte';

	let { locationId }: { locationId: string } = $props();

	let tokens = $state<LocationToken[] | null>(null);
	let loadError = $state<string | null>(null);
	let actionError = $state<string | null>(null);
	let busy = $state<string | null>(null);
	let issued = $state<IssuedToken<LocationToken> | null>(null);

	const canManage = $derived(app.can('location.manage'));

	async function load(id: string) {
		try {
			tokens = await app.client.listLocationTokens(id);
			loadError = null;
		} catch (error) {
			loadError = errorMessage(error);
		}
	}

	$effect(() => {
		if (canManage) void load(locationId);
	});

	async function act(key: string, action: () => Promise<unknown>) {
		busy = key;
		actionError = null;
		try {
			await action();
			await load(locationId);
		} catch (error) {
			actionError = errorMessage(error);
		} finally {
			busy = null;
		}
	}

	const create = () => act('create', async () => (issued = await app.client.createLocationToken(locationId)));
	const rotate = (t: LocationToken) => act(t.id, async () => (issued = await app.client.rotateLocationToken(t.id)));
	function revoke(t: LocationToken) {
		if (!confirm(`Revoke ${t.prefix}…? Location processes using it can't connect or deploy any more.`)) return;
		void act(t.id, () => app.client.revokeLocationToken(t.id));
	}
</script>

{#if !canManage}
	<p class="muted">You don't have permission to see location tokens (<code>location.manage</code>).</p>
{:else}
	{#if issued}
		<div class="issued-wrap">
			<IssuedSecret label="New location token {issued.token.prefix}…" secret={issued.secret} ondismiss={() => (issued = null)} />
		</div>
	{/if}
	{#if loadError}<p class="err">Couldn't load tokens: {loadError}</p>{/if}
	{#if actionError}<p class="err">{actionError}</p>{/if}
	<ul class="tokens">
		{#each tokens ?? [] as t (t.id)}
			<li class:off={t.state !== 'active'}>
				<code>{t.prefix}…</code>
				<StatusLabel tone={tokenTone[t.state]} label={tokenLabel[t.state]} muted={t.state !== 'active'} />
				<span class="meta" title="Created {formatStamp(t.createdAt)} by {t.createdBy.name}">
					created {formatAgo(t.createdAt, app.now)} ·
					{t.revokedAt ? `revoked ${formatAgo(t.revokedAt, app.now)}` : t.lastUsedAt ? `used ${formatAgo(t.lastUsedAt, app.now)}` : 'never used'}
				</span>
				{#if t.state === 'active'}
					<span class="actions">
						<GatedButton requires="location.manage" size="xs" busy={busy === t.id} onclick={() => rotate(t)}>Rotate</GatedButton>
						<GatedButton requires="location.manage" size="xs" variant="destructive" busy={busy === t.id} onclick={() => revoke(t)}>Revoke</GatedButton>
					</span>
				{/if}
			</li>
		{:else}
			<li class="muted">{#if tokens}No tokens yet. Create one and give it to the location so it can connect.{:else}<Loading lines={2} />{/if}</li>
		{/each}
	</ul>
	<div class="create">
		<GatedButton requires="location.manage" busy={busy === 'create'} onclick={create}>Create token</GatedButton>
	</div>
{/if}

<style>
	.tokens {
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--border);
		font-size: 12.5px;
	}
	.tokens li {
		display: flex;
		min-height: 40px;
		align-items: center;
		gap: 12px;
		padding: 6px 10px;
		border-bottom: 1px solid var(--border);
	}
	.tokens li.off {
		opacity: 0.65;
	}
	.actions {
		display: flex;
		gap: 4px;
		margin-left: auto;
	}
	.meta {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.err {
		margin-bottom: 6px;
		color: var(--sb-error);
		font-size: 12.5px;
	}
	.create {
		margin-top: 8px;
	}
	.issued-wrap :global(.issued) {
		margin: 0 0 8px;
	}
</style>
