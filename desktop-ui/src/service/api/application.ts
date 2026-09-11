import { invokeDesktop } from '@/service/tauri/client';

export interface AppInfo {
  version: string;
  platform: string;
}

export type UpdateResult =
  | { kind: 'native' }
  | {
      kind: 'release';
      version: string;
      available: boolean;
      hasApk: boolean;
      releaseUrl: string;
    };

export const getAppInfo = () => invokeDesktop<AppInfo>('get_app_info');
export const checkAppUpdate = () => invokeDesktop<UpdateResult>('check_app_update');
