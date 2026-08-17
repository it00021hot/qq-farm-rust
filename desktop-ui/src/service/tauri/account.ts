import { invokeDesktop } from './client';

/** 概览快照 */
export function fetchDesktopSnapshot() {
  return invokeDesktop<Desktop.DesktopSnapshot>('get_snapshot');
}

/** 账号列表 */
export function fetchDesktopAccounts() {
  return invokeDesktop<Desktop.AccountSummary[]>('list_accounts');
}
