// Shared window state, one listener, initialized once by the root layout.
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';

export const win = $state({ maximized: false, fullscreen: false });

let started = false;
let applying = false;
let wasMaximized = false;
let transitions = 0;
let pending: Promise<void> = Promise.resolve();
let exitHandler: (() => void) | null = null;

export function onFullscreenExit(cb: () => void): void {
	exitHandler = cb;
}

export async function setWindowFullscreen(on: boolean): Promise<void> {
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
			// Going maximized -> fullscreen: unmaximize first so the webview gets a proper resize,
			// avoiding taskbar and half-maximized glitch states on Windows borderless windows.
			if (wasMaximized) {
				await w.unmaximize().catch(() => {});
				for (let i = 0; i < 20; i++) {
					const isMax = await w.isMaximized().catch(() => false);
					if (!isMax) break;
					await new Promise((r) => setTimeout(r, 20));
				}
				await new Promise((r) => setTimeout(r, 50));
			}
			await w.setFullscreen(true);
			win.fullscreen = true;
			win.maximized = false;
		} else {
			win.fullscreen = false;
			await w.setFullscreen(false);
			if (wasMaximized) {
				wasMaximized = false;
				for (let i = 0; i < 20; i++) {
					const isFull = await w.isFullscreen().catch(() => false);
					if (!isFull) break;
					await new Promise((r) => setTimeout(r, 20));
				}
				await new Promise((r) => setTimeout(r, 50));
				await w.maximize().catch(() => {});
				win.maximized = true;
			} else {
				win.maximized = await w.isMaximized().catch(() => false);
			}
		}
	} catch (e) {
		console.error('setFullscreen failed', e);
	} finally {
		setTimeout(async () => {
			applying = false;
			transitions++;
			try {
				const f = await w.isFullscreen();
				win.fullscreen = f;
				win.maximized = f ? false : await w.isMaximized();
			} catch {}
		}, 350);
	}
}

export function initWin(): () => void {
	if (started) return () => {};
	started = true;
	try {
		const w = getCurrentWindow();
		invoke<boolean>('show_main').catch(() => w.show().catch(() => {}));
		const sync = () => {
			if (applying) return;
			Promise.all([w.isMaximized().catch(() => false), w.isFullscreen().catch(() => false)])
				.then(([m, f]) => {
					if (applying) return;
					const wasFull = win.fullscreen;
					win.fullscreen = f;
					win.maximized = f ? false : m;
					if (wasFull && !f && exitHandler) {
						exitHandler();
					}
				})
				.catch(() => {});
		};
		sync();
		const un = w.onResized(sync);
		return () => {
			un.then((u) => u()).catch(() => {});
		};
	} catch {
		return () => {};
	}
}
