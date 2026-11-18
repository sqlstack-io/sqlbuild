// Copies text. `navigator.clipboard` exists only in secure contexts (HTTPS or localhost), and the UI is also
// served over plain HTTP behind a proxy, so without it the text is copied from a hidden textarea.

/** Copies `text`; whether it was copied. Call it from a click, which the fallback needs. */
export async function copyText(text: string): Promise<boolean> {
	if (navigator.clipboard?.writeText) {
		try {
			await navigator.clipboard.writeText(text);
			return true;
		} catch {
			// Denied or unavailable: try the fallback.
		}
	}
	const area = document.createElement('textarea');
	area.value = text;
	area.setAttribute('readonly', '');
	area.style.cssText = 'position:fixed;top:0;left:0;width:1px;height:1px;opacity:0;';
	const focused = document.activeElement as HTMLElement | null;
	document.body.append(area);
	area.select();
	try {
		return document.execCommand('copy');
	} catch {
		return false;
	} finally {
		area.remove();
		focused?.focus?.();
	}
}
