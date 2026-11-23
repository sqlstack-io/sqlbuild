<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Dialog } from 'bits-ui';
	import XIcon from '@lucide/svelte/icons/x';

	// The UI's one dialog: a title, a short description, the body, and the buttons in `footer`.
	type Props = {
		open: boolean;
		title: string;
		description?: string;
		/** Called when it closes by Escape, the close button or a click outside. */
		onclose: () => void;
		children: Snippet;
		footer?: Snippet;
		testid?: string;
	};
	let { open, title, description, onclose, children, footer, testid }: Props = $props();
</script>

<Dialog.Root {open} onOpenChange={(next) => !next && onclose()}>
	<Dialog.Portal>
		<Dialog.Overlay class="data-open:animate-in data-open:fade-in-0 data-closed:animate-out data-closed:fade-out-0 fixed inset-0 z-50 bg-black/45 duration-100" />
		<Dialog.Content
			class="data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95 bg-popover text-popover-foreground ring-foreground/10 fixed top-1/2 left-1/2 z-50 flex w-[min(460px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 flex-col gap-4 rounded-md p-5 shadow-lg ring-1 duration-100 outline-none"
			data-testid={testid}
		>
			<div class="head">
				<div class="min-w-0 flex-1">
					<Dialog.Title class="title">{title}</Dialog.Title>
					{#if description}<Dialog.Description class="description">{description}</Dialog.Description>{/if}
				</div>
				<Dialog.Close class="close" aria-label="Close"><XIcon size={15} /></Dialog.Close>
			</div>
			{@render children()}
			{#if footer}<div class="footer">{@render footer()}</div>{/if}
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>

<style>
	.head {
		display: flex;
		align-items: flex-start;
		gap: 12px;
	}
	.head :global(.title) {
		margin: 0;
		font-family: var(--font-display);
		font-size: 15px;
		font-weight: 600;
	}
	.head :global(.description) {
		margin: 4px 0 0;
		color: var(--muted-foreground);
		font-size: 12.5px;
		line-height: 1.5;
	}
	.head :global(.close) {
		display: grid;
		width: 26px;
		height: 26px;
		place-items: center;
		border-radius: 4px;
		color: var(--muted-foreground);
	}
	.head :global(.close:hover) {
		background: var(--accent);
		color: var(--foreground);
	}
	.footer {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}
</style>
