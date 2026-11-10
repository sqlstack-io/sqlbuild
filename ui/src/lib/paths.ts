import { resolve } from '$app/paths';

/** The path the UI is served under: empty when `sqb server` serves it, `/demo` for the static demo. */
const base = resolve('/').replace(/\/$/, '');

/** An app path, such as `/runs/42`, as the address to link to. */
export const to = (path: string): string => `${base}${path}`;

/** The app path of an address's pathname, without the path the UI is served under. */
export const appPath = (pathname: string): string => (pathname.startsWith(`${base}/`) ? pathname.slice(base.length) : pathname);
