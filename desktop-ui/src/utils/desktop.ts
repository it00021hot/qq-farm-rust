/** Desktop (Tauri) shell helpers — align chrome with qq-farm-desktop / qq-farm-web. */

import { computed, ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { isTauriRuntime } from '@/service/tauri/client';

export const isDesktop = true;

const desktopShellActive = ref(false);

export const isDesktopShell = computed(() => desktopShellActive.value);

/** Detect shell + apply window chrome classes. */
export async function bootstrapDesktopShell(): Promise<boolean> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    desktopShellActive.value = false;
    return false;
  }
  desktopShellActive.value = true;
  installDesktopContextMenuGuard();
  document.documentElement.classList.toggle('desktop-windows', isDesktopWindows());
  document.documentElement.classList.toggle('desktop-mac', isDesktopMac());
  return true;
}

export function isDesktopWindows(): boolean {
  if (!desktopShellActive.value || typeof navigator === 'undefined') return false;
  return /windows/i.test(navigator.userAgent);
}

export function isDesktopMac(): boolean {
  if (!desktopShellActive.value || typeof navigator === 'undefined') return false;
  return /mac/i.test(navigator.userAgent);
}

export async function desktopMinimise(): Promise<void> {
  if (!isTauriRuntime()) return;
  await getCurrentWindow().minimize();
}

export async function desktopToggleMaximise(): Promise<void> {
  if (!isTauriRuntime()) return;
  const win = getCurrentWindow();
  if (await win.isMaximized()) {
    await win.unmaximize();
  } else {
    await win.maximize();
  }
}

export async function desktopClose(): Promise<void> {
  if (!isTauriRuntime()) return;
  await getCurrentWindow().close();
}

export async function desktopIsFullscreen(): Promise<boolean> {
  if (!isTauriRuntime()) return false;
  return getCurrentWindow().isFullscreen();
}

/** WKWebView `requestFullscreen` is a no-op; toggle the native Tauri window. */
export async function desktopToggleFullscreen(): Promise<boolean> {
  if (!isTauriRuntime()) return false;
  const win = getCurrentWindow();
  const next = !(await win.isFullscreen());
  await win.setFullscreen(next);
  return next;
}

let contextMenuGuardInstalled = false;

/** Block WebView “View Source / Inspect”; keep copy-paste on inputs and Vue custom menus. */
export function installDesktopContextMenuGuard(): void {
  if (typeof document === 'undefined' || contextMenuGuardInstalled) return;
  contextMenuGuardInstalled = true;
  document.addEventListener(
    'contextmenu',
    event => {
      if (!desktopShellActive.value) return;
      const target = event.target;
      if (target instanceof Element && target.closest('input, textarea, [contenteditable="true"]')) {
        return;
      }
      event.preventDefault();
    },
    true
  );
}
