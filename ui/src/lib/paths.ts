import { goto } from '$app/navigation';
import { resolve } from '$app/paths';

/** The path the UI is served under: empty when `sqb server` serves it, `/demo` for the static demo. */
const base = resolve('/').replace(/\/$/, '');

/** An app path, such as `/runs/42`, as the address to link to. */
export const to = (path: string): string => `${base}${path}`;

/** The app path of an address's pathname, without the path the UI is served under. */
export const appPath = (pathname: string): string => (pathname.startsWith(`${base}/`) ? pathname.slice(base.length) : pathname);

/** The parts of a row that do their own thing when clicked, such as a link to another page. */
const ROW_CONTROL = 'a, button, input, label, [data-control]';

/**
 * Opens the page a table row stands for, in a new tab with Ctrl, ⌘ or the middle button. A click on
 * a link or control in the row is left to it, so links keep SvelteKit's in-app navigation.
 */
export function openRow(e: MouseEvent, path: string): void {
	if ((e.target as Element).closest(ROW_CONTROL)) return;
	if (e.metaKey || e.ctrlKey || e.button === 1) window.open(to(path), '_blank', 'noopener');
	else void goto(to(path));
}
