import { invoke } from '@tauri-apps/api/core';

/** 是否运行在 Tauri WebView 中 */
export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/**
 * 调用桌面端命令；非 Tauri 环境抛错（本前端仅服务桌面）。
 */
export async function invokeDesktop<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    throw new Error(`Tauri IPC unavailable: ${cmd}`);
  }
  return invoke<T>(cmd, args);
}
