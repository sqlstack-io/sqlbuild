// Copies text. `navigator.clipboard` exists only in secure contexts (HTTPS or localhost), and the UI is also
// served over plain HTTP behind a proxy, so without it the text is copied from a hidden textarea.

/**
 * Copies `text`; whether it was copied. Call it from a click, which the fallback needs, and pass the
 * clicked element: the fallback's textarea goes next to it, so it can be selected inside a modal
 * dialog, which makes everything outside it inert.
 */
export async function copyText(text: string, near?: Element | null): Promise<boolean> {
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
	// Selecting the textarea replaces the person's own selection and focus; both are put back afterwards.
	const focused = document.activeElement as HTMLElement | null;
	const selection = document.getSelection();
	const ranges = selection ? Array.from({ length: selection.rangeCount }, (_, i) => selection.getRangeAt(i)) : [];
	(near?.parentElement ?? document.body).append(area);
	area.focus({ preventScroll: true });
	area.select();
	try {
		// An inert textarea can't take the selection, and `execCommand` may still report success.
		const selected = document.activeElement === area && area.selectionEnd - area.selectionStart === text.length;
		return selected && document.execCommand('copy');
	} catch {
		return false;
	} finally {
		area.remove();
		focused?.focus?.({ preventScroll: true });
		if (selection) {
			selection.removeAllRanges();
			for (const range of ranges) selection.addRange(range);
		}
	}
}
