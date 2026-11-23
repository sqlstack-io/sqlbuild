import { copyText } from './clipboard';

/** How long a copy button says "Copied" before it reads "Copy" again. */
export const COPIED_MS = 2000;

/**
 * A copy button's state: `copied` for a moment after each copy that worked, `failed` after one that
 * didn't. Each press copies again.
 */
export function createCopyState() {
	let copied = $state(false);
	let failed = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;
	return {
		get copied() {
			return copied;
		},
		get failed() {
			return failed;
		},
		/** Copy `text` from a click on `near`; whether it was copied. */
		async copy(text: string, near?: Element | null): Promise<boolean> {
			clearTimeout(timer);
			copied = await copyText(text, near);
			failed = !copied;
			if (copied) timer = setTimeout(() => (copied = false), COPIED_MS);
			return copied;
		}
	};
}
