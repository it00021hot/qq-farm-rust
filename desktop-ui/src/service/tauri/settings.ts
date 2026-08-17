import { invokeDesktop } from './client';

/** 设置只读摘要 */
export function fetchDesktopSettings(accountId?: string) {
  return invokeDesktop<Desktop.SettingsSummary>('get_settings', {
    accountId: accountId ?? null
  });
}
