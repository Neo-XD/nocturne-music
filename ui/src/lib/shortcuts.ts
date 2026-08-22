// App-wide keyboard shortcuts. One window listener, gated on Ctrl/Cmd before anything else, so a
// key typed into a field costs a single boolean check and falls straight through. Zoom keeps its
// own listener (zoom.ts) because it also owns the ctrl+wheel gesture.
import { np, nudgeVolume, playback, ui } from './player.svelte';

/** Percent per press, matching a step of the volume slider's arrow keys. */
const VOLUME_STEP = 5;

export function initShortcuts() {
	const onKey = (e: KeyboardEvent) => {
		if (!e.ctrlKey && !e.metaKey) return;
		switch (e.key) {
			// Toggles, so the key that opened the palette also dismisses it.
			case 'k':
			case 'K':
				ui.paletteOpen = !ui.paletteOpen;
				break;
			case 'e':
			case 'E':
				// With nothing playing there is no view to open (the layout renders it behind
				// `playback.now`), and flipping the flag anyway would ambush the next play.
				if (!playback.now) return;
				np.open = !np.open;
				break;
			// Shift+. and Shift+, on a US layout. The unshifted keys are accepted too, so the
			// shortcut still works on layouts that put > and < somewhere else.
			case '>':
			case '.':
				nudgeVolume(VOLUME_STEP);
				break;
			case '<':
			case ',':
				nudgeVolume(-VOLUME_STEP);
				break;
			default:
				return;
		}
		e.preventDefault();
	};
	window.addEventListener('keydown', onKey);
	return () => window.removeEventListener('keydown', onKey);
}
