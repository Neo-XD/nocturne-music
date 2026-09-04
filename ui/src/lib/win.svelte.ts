// Shared window state, one listener, initialized once by the root layout.
import { getCurrentWindow } from '@tauri-apps/api/window';

export const win = $state({ maximized: false, fullscreen: false });

let started = false;
// Set while our own setFullscreen call is in flight, so the resize it causes is not mistaken for the user leaving fullscreen.
let applying = false;
let wasMaximized = false;
// Bumped when a transition settles, so an isFullscreen read that straddled one is dropped instead of overwriting win.fullscreen.
let transitions = 0;
let pending: Promise<void> = Promise.resolve();
let exitHandler: (() => void) | null = null;

// The overlay is closed from here rather than from a component, because the OS can leave fullscreen without anything in the UI asking.
export function onFullscreenExit(cb: () => void): void {
	exitHandler = cb;
}

export async function setWindowFullscreen(on: boolean): Promise<void> {
	// Serialized: a track change can re-enter this mid-transition, and two overlapping runs lose the state to restore.
	pending = pending.then(() => applyFullscreen(on)).catch(() => {});
	return pending;
}

async function applyFullscreen(on: boolean): Promise<void> {
	if (on === win.fullscreen) return;
	let w: ReturnType<typeof getCurrentWindow>;
	try {
		w = getCurrentWindow();
	} catch {
		return;
	}
	applying = true;
	try {
		if (on) {
			wasMaximized = await w.isMaximized().catch(() => false);
			// Going maximized -> fullscreen leaves WRY_WEBVIEW at the old working-area height, so the taskbar shows through the transparent window; restoring first gives the webview a resize it follows.
			if (wasMaximized) await w.unmaximize().catch(() => {});
		}
		await w.setFullscreen(on);
		win.fullscreen = on;
		if (!on && wasMaximized) {
			// Consumed here, so a later exit that never entered fullscreen cannot re-maximize the window.
			wasMaximized = false;
			await w.maximize().catch(() => {});
		}
	} catch (e) {
		console.error('setFullscreen failed', e);
		win.fullscreen = await w.isFullscreen().catch(() => false);
	} finally {
		applying = false;
		transitions++;
	}
}

export function initWin(): () => void {
	if (started) return () => {};
	started = true;
	try {
		const w = getCurrentWindow();
		w.show().catch(() => {});
		const sync = () => {
			w.isMaximized()
				.then((m) => (win.maximized = m))
				.catch(() => {});
			const seen = transitions;
			w.isFullscreen()
				.then((f) => {
					if (applying || seen !== transitions) return;
					const left = win.fullscreen && !f;
					win.fullscreen = f;
					if (left) {
						// Restored here, not via setWindowFullscreen: this path has already cleared win.fullscreen, so applyFullscreen would match the target and return.
						if (wasMaximized) {
							wasMaximized = false;
							w.maximize().catch(() => {});
						}
						exitHandler?.();
					}
				})
				.catch(() => {});
		};
		sync();
		const un = w.onResized(sync);
		return () => { un.then((u) => u()).catch(() => {}); };
	} catch {
		return () => {};
	}
}
