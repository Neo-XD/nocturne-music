/**
 * Copy plain text to the system clipboard.
 *
 * WebKitGTK (Tauri's webview on Linux) rejects `navigator.clipboard.writeText` with a
 * NotAllowedError whenever it decides the click's user activation doesn't count, so the standard
 * API can't be the only path. The old selection copy is synchronous and gesture-bound, which is
 * exactly what survives that, so it goes first and the modern API is the fallback for whenever
 * `execCommand` finally disappears.
 *
 * Call it straight from a click handler: anything awaited before it spends the user gesture.
 * Rejects only when both paths failed.
 */
export async function copyText(text: string): Promise<void> {
	if (selectionCopy(text)) return;
	await navigator.clipboard.writeText(text);
}

function selectionCopy(text: string): boolean {
	// A real, on-page element: `display:none` and `hidden` can't hold a selection. Off-screen and
	// readonly so it never steals a caret the user can see or pops the on-screen keyboard.
	const ta = document.createElement('textarea');
	ta.value = text;
	ta.readOnly = true;
	ta.style.cssText = 'position:fixed;top:-9999px;left:-9999px;opacity:0';
	document.body.appendChild(ta);
	const previous = document.activeElement;
	try {
		ta.select();
		return document.execCommand('copy');
	} catch {
		return false;
	} finally {
		ta.remove();
		// Same task as the click, so the focus never visibly leaves the button.
		if (previous instanceof HTMLElement) previous.focus();
	}
}
