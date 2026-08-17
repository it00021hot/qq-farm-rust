import { invokeDesktop, isTauriRuntime } from './client';

/** 探测桌面引擎是否就绪 */
export function fetchDesktopReady() {
  return invokeDesktop<boolean>('desktop_ready');
}

/** LocalOwner 进入：不走面板 HTTP 鉴权 */
export async function fetchDesktopLogin(): Promise<Api.Auth.LoginToken> {
  if (isTauriRuntime()) {
    await fetchDesktopReady();
  }
  return {
    token: 'local-owner',
    refreshToken: 'local-owner'
  };
}

/** 本地所有者用户信息（静态超级角色，便于 static 路由） */
export async function fetchDesktopUserInfo(): Promise<Api.Auth.UserInfo> {
  return {
    userId: 'local',
    userName: 'LocalOwner',
    roles: ['R_SUPER'],
    buttons: []
  };
}
