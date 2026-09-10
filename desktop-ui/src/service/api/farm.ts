import type { FlatResponseData } from '@sa/axios';
import { fetch as nativeHttpFetch } from '@tauri-apps/plugin-http';
import { invokeDesktop } from '@/service/tauri/client';
import { formatInvokeError } from '@/utils/error';

async function invokeFlat<T = any>(
  cmd: string,
  args?: Record<string, unknown>,
  options?: { silent?: boolean }
): Promise<FlatResponseData<any, T>> {
  try {
    const data = (await invokeDesktop<T>(cmd, args)) as T;
    return { data, error: null, response: {} as any };
  } catch (e) {
    const message = formatInvokeError(e);
    const error = new Error(message);
    if (!options?.silent) {
      window.$message?.error(message);
    }
    return { data: null as any, error: error as any, response: {} as any };
  }
}

function aid(id: number | string | null | undefined): string {
  if (id == null || id === '') return '';
  return String(id);
}

function toAccountRecord(raw: any): Api.Farm.Account {
  const idNum = Number(raw?.id);
  return {
    id: Number.isFinite(idNum) ? idNum : 0,
    name: String(raw?.name ?? ''),
    code: String(raw?.code ?? ''),
    platform: (raw?.platform || 'qq') as Api.Farm.Platform,
    uin: String(raw?.uin ?? ''),
    qq: String(raw?.qq ?? ''),
    avatar: String(raw?.avatar ?? ''),
    username: String(raw?.username ?? ''),
    remark: String(raw?.remark ?? raw?.name ?? ''),
    runStatus: (raw?.running || raw?.runStatus === 1 ? 1 : 0) as Api.Farm.RunStatus,
    lastOnlineAt: Number(raw?.lastOnlineAt ?? raw?.updatedAt ?? raw?.updated_at ?? 0),
    status: (String(raw?.status ?? '1') === '2' ? '2' : '1') as Api.Farm.EnableStatus,
    wxAuthorized: Boolean(raw?.wxAuthorized ?? raw?.wx_authorized),
    wxRescanRecommended: Boolean(raw?.wxRescanRecommended ?? raw?.wx_rescan_recommended),
    createdAt: Number(raw?.createdAt ?? raw?.created_at ?? 0),
    updatedAt: Number(raw?.updatedAt ?? raw?.updated_at ?? 0)
  };
}

/** Flattened panel status from Rust `PanelStatus`. */
interface PanelStatusDto {
  accountId?: string | number;
  running?: boolean;
  online?: boolean;
  runStatus?: number;
  nick?: string;
  avatar?: string;
  level?: number;
  exp?: number;
  gold?: number;
  landCount?: number;
  friendCount?: number;
  lastError?: string;
  wsError?: string;
  updatedAt?: number;
  uptime?: number;
  sessionExpGained?: number;
  sessionGoldGained?: number;
  levelProgress?: { current?: number; needed?: number; level?: number };
  operations?: Record<string, number>;
  nextChecks?: Api.Farm.Status['nextChecks'];
}

interface FriendSummaryDto {
  gid?: number;
  name?: string;
  nickname?: string;
  remark?: string;
  avatarUrl?: string;
  avatar?: string;
  avatar_url?: string;
  level?: number;
  gold?: number;
  petState?: string;
  pet?: { id?: number; name?: string; image?: string } | null;
  plant?: {
    stealNum?: number;
    dryNum?: number;
    weedNum?: number;
    insectNum?: number;
  };
}

function toFarmStatus(raw: PanelStatusDto, accountId: number): Api.Farm.Status {
  const running = Boolean(raw.running) || Number(raw.runStatus) === 1 || Boolean(raw.online);
  const levelProgress = raw.levelProgress;
  return {
    accountId: Number(raw.accountId ?? accountId) || accountId,
    runStatus: (running ? 1 : 0) as Api.Farm.RunStatus,
    online: Boolean(raw.online),
    level: Number(raw.level ?? 0) || undefined,
    exp: Number(raw.exp ?? 0) || undefined,
    gold: Number(raw.gold ?? 0) || undefined,
    nick: String(raw.nick ?? ''),
    avatar: String(raw.avatar ?? ''),
    landCount: Number(raw.landCount ?? 0) || undefined,
    friendCount: Number(raw.friendCount ?? 0) || undefined,
    lastError: String(raw.lastError ?? raw.wsError ?? '') || undefined,
    updatedAt: Number(raw.updatedAt ?? 0) || undefined,
    uptime: Number(raw.uptime ?? 0) || undefined,
    sessionExpGained: Number(raw.sessionExpGained ?? 0) || undefined,
    sessionGoldGained: Number(raw.sessionGoldGained ?? 0) || undefined,
    levelProgress: levelProgress
      ? {
          current: Number(levelProgress.current ?? 0),
          needed: Number(levelProgress.needed ?? 0),
          level: Number(levelProgress.level ?? raw.level ?? 0)
        }
      : undefined,
    operations: raw.operations,
    nextChecks: raw.nextChecks
  };
}

function toAutomationDetail(raw: any, accountId: number): Api.Farm.AccountAutomationDetail {
  const strategy = raw?.plantingStrategy ?? raw?.strategy;
  return {
    accountId,
    automation: (raw?.automation || {}) as Api.Farm.AutomationConfig,
    intervals: raw?.intervals,
    plantingStrategy: typeof strategy === 'string' ? strategy : String(strategy || 'preferred'),
    preferredSeedId: Number(raw?.preferredSeedId ?? raw?.preferredSeed ?? 0),
    bagSeedPriority: Array.isArray(raw?.bagSeedPriority) ? raw.bagSeedPriority.map(Number) : [],
    bagSeedFallbackStrategy: String(raw?.bagSeedFallbackStrategy || 'level'),
    plantOrderRandom: !!raw?.plantOrderRandom,
    plantDelaySeconds: Number(raw?.plantDelaySeconds ?? 0),
    stealDelaySeconds: Number(raw?.stealDelaySeconds ?? 1),
    friendQuietHours: normalizeQuietHours(raw?.friendQuietHours),
    friendBlacklist: Array.isArray(raw?.friendBlacklist) ? raw.friendBlacklist.map(Number) : [],
    plantBlacklist: Array.isArray(raw?.plantBlacklist) ? raw.plantBlacklist.map(Number) : [],
    friendAutoAccept: Boolean(raw?.automation?.friend_auto_accept ?? raw?.automation?.friendAutoAccept ?? true),
    showManualFertilizer: Boolean(
      raw?.automation?.show_manual_fertilizer ?? raw?.automation?.showManualFertilizer ?? true
    ),
    autoAcceptFriendMinLevel: Number(raw?.autoAcceptFriendMinLevel ?? 0),
    autoAcceptRequireOwnLevel: !!raw?.autoAcceptRequireOwnLevel,
    autoAcceptHarvestStealEnabled: !!raw?.autoAcceptHarvestStealEnabled,
    autoAcceptHarvestStealHarvest: Number(raw?.autoAcceptHarvestStealHarvest ?? 8),
    autoAcceptHarvestStealSteal: Number(raw?.autoAcceptHarvestStealSteal ?? 1),
    fertilizerBuyOrganicCount: Number(raw?.fertilizerBuyOrganicCount ?? 1),
    fertilizerBuyOrganicThresholdHours: Number(raw?.fertilizerBuyOrganicThresholdHours ?? 10),
    fertilizerBuyNormalCount: Number(raw?.fertilizerBuyNormalCount ?? 1),
    fertilizerBuyNormalThresholdHours: Number(raw?.fertilizerBuyNormalThresholdHours ?? 10),
    fertilizerBuyCheckIntervalMinutes: Number(raw?.fertilizerBuyCheckIntervalMinutes ?? 60),
    configJson: raw?.configJson
  };
}

/** 好友静默时段（面板返回 continue_farm，保存用 continueFarm）。 */
function normalizeQuietHours(raw: unknown): Api.Farm.QuietHoursConfig {
  const row = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  return {
    enabled: !!row.enabled,
    start: String(row.start ?? '01:00'),
    end: String(row.end ?? '07:30'),
    continueFarm: Boolean(row.continueFarm ?? row.continue_farm ?? true)
  };
}

function toFriendRecord(raw: FriendSummaryDto, accountId: number): Api.Farm.Friend {
  const gid = Number(raw.gid ?? 0);
  const petState = String(raw.petState ?? '') || undefined;
  const pet = (raw.pet && typeof raw.pet === 'object' ? raw.pet : null) as {
    id?: number;
    name?: string;
    image?: string;
  } | null;
  return {
    accountId,
    gid,
    nickname: String(raw.nickname ?? raw.name ?? raw.remark ?? '').trim(),
    level: Number(raw.level ?? 0) || undefined,
    gold: Number(raw.gold ?? 0) || undefined,
    avatar: String(raw.avatar ?? raw.avatarUrl ?? raw.avatar_url ?? ''),
    petState: petState as Api.Farm.Friend['petState'],
    pet:
      pet && (pet.name || pet.id)
        ? { id: Number(pet.id || 0), name: String(pet.name || ''), image: String(pet.image || '') }
        : null,
    plant: raw.plant
      ? {
          stealNum: Number(raw.plant.stealNum ?? 0),
          dryNum: Number(raw.plant.dryNum ?? 0),
          weedNum: Number(raw.plant.weedNum ?? 0),
          insectNum: Number(raw.plant.insectNum ?? 0)
        }
      : undefined
  };
}

function normalizeSeedRow(raw: any): any {
  if (!raw || typeof raw !== 'object') return raw;
  return {
    ...raw,
    seedId: Number(raw.seedId ?? raw.seed_id ?? 0),
    name: String(raw.name ?? ''),
    requiredLevel: Number(raw.requiredLevel ?? raw.land_level_need ?? raw.required_level ?? 0),
    price: Number(raw.price ?? 0),
    size: Number(raw.size ?? 1),
    locked: !!raw.locked,
    soldOut: !!raw.soldOut
  };
}

export async function fetchGetFarmAccountList(params?: Api.Farm.AccountSearchParams) {
  const res = await invokeFlat<any>('list_accounts_page');
  if (res.error) {
    return res as FlatResponseData<any, Api.Farm.AccountList>;
  }
  let records: Api.Farm.Account[] = (Array.isArray(res.data?.accounts) ? res.data.accounts : []).map(toAccountRecord);
  if (params?.keyword) {
    const kw = String(params.keyword).toLowerCase();
    records = records.filter(
      (a: Api.Farm.Account) =>
        a.name.toLowerCase().includes(kw) || a.qq.toLowerCase().includes(kw) || a.remark.toLowerCase().includes(kw)
    );
  }
  if (params?.platform) {
    records = records.filter((a: Api.Farm.Account) => a.platform === params.platform);
  }
  if (params?.runStatus != null && String(params.runStatus) !== '') {
    records = records.filter((a: Api.Farm.Account) => a.runStatus === Number(params.runStatus));
  }
  if (params?.authStatus === 'authorized') {
    records = records.filter((a: Api.Farm.Account) => Boolean(a.wxAuthorized));
  } else if (params?.authStatus === 'unauthorized') {
    records = records.filter((a: Api.Farm.Account) => !a.wxAuthorized);
  }
  const current = Number(params?.current) || 1;
  const size = Number(params?.size) || 10;
  const total = records.length;
  const start = (current - 1) * size;
  const page = records.slice(start, start + size);
  return {
    data: { records: page, current, size, total },
    error: null,
    response: {} as any
  };
}

async function upsertAndPick(req: Record<string, unknown>) {
  const res = await invokeFlat<any>('upsert_account', { req });
  if (res.error) return res as FlatResponseData<any, Api.Farm.Account>;
  const accounts = Array.isArray(res.data?.accounts) ? res.data.accounts.map(toAccountRecord) : [];
  const id = req.id != null ? Number(req.id) : NaN;
  let picked = Number.isFinite(id) && id > 0 ? accounts.find((a: Api.Farm.Account) => a.id === id) : undefined;
  if (!picked && accounts.length) {
    picked = [...accounts].sort((a, b) => b.updatedAt - a.updatedAt || b.id - a.id)[0];
  }
  return { data: picked || null, error: null, response: {} as any };
}

export function fetchAddFarmAccount(data: Api.Farm.AccountCreateParams) {
  return upsertAndPick({
    name: data.name,
    code: data.code,
    platform: data.platform
  });
}

export function fetchModifyFarmAccount(data: Api.Farm.AccountUpdateParams) {
  return upsertAndPick({
    id: String(data.id),
    name: data.name,
    code: data.code,
    platform: data.platform
  });
}

export function fetchDeleteFarmAccount(id: number) {
  return invokeFlat('delete_account', { accountId: aid(id) });
}

export function fetchStartFarmAccount(id: number) {
  return invokeFlat('start_account', { accountId: aid(id) });
}

export function fetchStopFarmAccount(id: number) {
  return invokeFlat('stop_account', { accountId: aid(id) });
}

export function fetchCreateFarmWxLoginTask() {
  return invokeFlat('wx_login_create');
}

export function fetchFarmWxLoginStatus(taskId: string) {
  return invokeFlat('wx_login_poll', { taskId });
}

export function fetchConfirmFarmWxLogin(taskId: string) {
  return invokeFlat('wx_login_confirm', { taskId });
}

export function fetchFarmWxLoginCode(taskId: string) {
  return invokeFlat<{ code: string }>('wx_login_code', { taskId });
}

// ===== QQ 扫码登录（NapCat 对接，对齐 bot qq-login）=====

export function fetchGetQqLoginSettings() {
  return invokeFlat<{
    qqQrLogin: boolean;
    napCatEndpoint: string;
    napCatSignature: string;
  }>('get_qq_login_settings');
}

export function fetchSaveQqLoginSettings(settings: {
  qqQrLogin: boolean;
  napCatEndpoint: string;
  napCatSignature: string;
}) {
  return invokeFlat('save_qq_login_settings', { settings });
}

export function fetchQqLoginCreateTask() {
  return invokeFlat<{ taskId: string; status: string; qrImage: string; expiresAt?: number }>('qq_login_create_task');
}

export function fetchQqLoginTaskStatus(taskId: string) {
  return invokeFlat<{ taskId: string; status: string; qrImage: string; expiresAt?: number }>('qq_login_task_status', {
    taskId
  });
}

export function fetchQqLoginMiniappCode(taskId: string) {
  return invokeFlat<{ code: string }>('qq_login_miniapp_code', { taskId });
}

export function fetchQqLoginCancelTask(taskId: string) {
  return invokeFlat<{ ok: boolean }>('qq_login_cancel_task', { taskId });
}

export function fetchCreateFarmWxQuickLoginSession() {
  return invokeFlat<{
    sessionId: string;
    appId: string;
    scope: string;
    redirectUri: string;
    state: string;
    ports: number[];
    expiresAt: number;
  }>('wx_quick_login_create', undefined, { silent: true });
}

export function fetchDetectFarmWxQuickLogin(sessionId: string) {
  return invokeFlat<{
    port: number;
    authorizeUuid: string;
    nickname?: string;
    headimgurl?: string;
  }>('wx_quick_login_detect', { sessionId }, { silent: true });
}

export function fetchAuthorizeFarmWxQuickLogin(
  sessionId: string,
  payload: { port: number; authorizeUuid: string; x: number; y: number }
) {
  return invokeFlat<{ redirectUrl: string }>(
    'wx_quick_login_authorize',
    {
      sessionId,
      port: payload.port,
      authorizeUuid: payload.authorizeUuid,
      x: payload.x,
      y: payload.y
    },
    { silent: true }
  );
}

export function fetchConfirmFarmWxQuickLogin(sessionId: string, redirectUrl: string) {
  return invokeFlat<{ code: string; openid?: string }>(
    'wx_quick_login_confirm',
    {
      sessionId,
      redirectUrl
    },
    { silent: true }
  );
}

// ===== 本机微信 API（前端统一调用 HTTP 插件，由 Rust 发请求，不受 WebView CORS 限制）=====

interface WxLocalPayload {
  errcode: number;
  errmsg?: string;
  jsdata?: Record<string, unknown> | string | null;
}

function parseWxLocalResponse(text: string): WxLocalPayload {
  let value: unknown = JSON.parse(text);
  if (typeof value === 'string') value = JSON.parse(value as string);
  return (value ?? {}) as WxLocalPayload;
}

export interface WxLocalOauth {
  appId: string;
  scope: string;
  redirectUri: string;
  state: string;
}

async function wxLocalFetch(
  port: number,
  path: '/api/check-login' | '/api/authorize',
  jsdata: Record<string, unknown>,
  timeoutMs: number
): Promise<WxLocalPayload> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await nativeHttpFetch(`https://localhost.weixin.qq.com:${port}${path}`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        origin: 'https://open.weixin.qq.com',
        referer: 'https://open.weixin.qq.com/'
      },
      connectTimeout: timeoutMs,
      maxRedirections: 0,
      // 仅上述 capability 白名单内的本机微信接口接受自签证书。
      danger: { acceptInvalidCerts: true, acceptInvalidHostnames: false },
      body: JSON.stringify({
        apiname: path === '/api/authorize' ? 'qrconnectfastauthorize' : 'qrconnectchecklogin',
        jsdata
      }),
      signal: controller.signal
    });
    const text = await res.text();
    if (!res.ok) throw new Error(`本机微信 HTTP ${res.status}（端口 ${port}）`);
    return parseWxLocalResponse(text);
  } catch (error) {
    // 插件 IPC 可能抛出字符串，统一为 Error 以保留抽屉中的诊断信息。
    throw new Error(controller.signal.aborted ? '本机微信请求超时' : formatInvokeError(error), { cause: error });
  } finally {
    // 避免请求成功后定时器再次触发，取消插件已经释放的资源。
    clearTimeout(timer);
  }
}

/** 本机微信探测（POST /api/check-login） */
export function fetchWxLocalCheckLogin(port: number, oauth: WxLocalOauth, timeoutMs = 8000): Promise<WxLocalPayload> {
  return wxLocalFetch(
    port,
    '/api/check-login',
    {
      appid: oauth.appId,
      scope: oauth.scope,
      redirect_uri: oauth.redirectUri,
      state: oauth.state
    },
    timeoutMs
  );
}

/** 本机微信快捷授权（POST /api/authorize），成功返回 redirect_url */
export function fetchWxLocalAuthorize(
  port: number,
  oauth: WxLocalOauth,
  authorizeUuid: string,
  position: { x: number; y: number },
  timeoutMs = 40_000
): Promise<WxLocalPayload> {
  return wxLocalFetch(
    port,
    '/api/authorize',
    {
      data: JSON.stringify(position),
      appid: oauth.appId,
      scope: oauth.scope,
      redirect_uri: oauth.redirectUri,
      state: oauth.state,
      authorize_uuid: authorizeUuid
    },
    timeoutMs
  );
}

export async function fetchGetFarmStatusDetail(accountId: number) {
  const res = await invokeFlat<PanelStatusDto>('farm_status_detail', { accountId: aid(accountId) });
  if (res.error) return res as FlatResponseData<Api.Farm.Status, Api.Farm.Status>;
  return {
    data: toFarmStatus(res.data ?? {}, accountId),
    error: null,
    response: {} as FlatResponseData<Api.Farm.Status, Api.Farm.Status>['response']
  };
}

export function fetchGetFarmLogs(params?: Api.Farm.LogsSearchParams) {
  return invokeFlat('farm_get_logs', {
    accountId: params?.accountId != null ? aid(params.accountId) : null,
    limit: (params as any)?.size ?? (params as any)?.limit ?? 100
  });
}

export function fetchClearFarmLogs(accountId?: number) {
  return invokeFlat('farm_clear_logs', {
    accountId: accountId != null ? aid(accountId) : null
  });
}

export async function fetchGetFarmAutomationDetail(accountId: number) {
  const res = await invokeFlat<any>('get_settings_panel', { accountId: aid(accountId) });
  if (res.error) return res as FlatResponseData<any, Api.Farm.AccountAutomationDetail>;
  return {
    data: toAutomationDetail(res.data, accountId),
    error: null,
    response: {} as any
  };
}

export function fetchModifyFarmAutomation(data: Api.Farm.AccountAutomationModifyParams) {
  const { accountId, ...snapshot } = data;
  return invokeFlat('save_settings', {
    accountId: aid(accountId),
    snapshot
  });
}

export function fetchFertilizerCheckAndBuy(accountId: number | string) {
  return invokeFlat('farm_fertilizer_check_and_buy', { accountId: aid(accountId) });
}

interface DeviceInfoPayload {
  os: string;
  clientVersion: string;
  sysSoftware: string;
  network: string;
  memory: string;
  deviceId: string;
  userAgent: string;
}

export interface SystemConfigPayload {
  serverUrl: string;
  clientVersion: string;
  platform: string;
  os: string;
  timeZone?: string;
  deviceInfo: DeviceInfoPayload;
}

export interface SystemConfigResponse {
  saved: SystemConfigPayload | null;
  default: SystemConfigPayload;
  current?: unknown;
}

function normalizeSystemConfig(source: unknown, fallback: SystemConfigPayload): SystemConfigPayload {
  const row = (source && typeof source === 'object' ? source : {}) as Record<string, unknown>;
  const deviceRow =
    row.deviceInfo && typeof row.deviceInfo === 'object' ? (row.deviceInfo as Record<string, unknown>) : {};
  return {
    serverUrl: String(row.serverUrl ?? row.server_url ?? fallback.serverUrl ?? ''),
    clientVersion: String(row.clientVersion ?? row.client_version ?? fallback.clientVersion ?? ''),
    platform: String(row.platform ?? fallback.platform ?? 'qq'),
    os: String(row.os ?? fallback.os ?? 'Windows'),
    timeZone: String(row.timeZone ?? row.time_zone ?? fallback.timeZone ?? 'Asia/Shanghai'),
    deviceInfo: {
      os: String(deviceRow.os ?? fallback.deviceInfo.os ?? 'Windows'),
      clientVersion: String(
        deviceRow.clientVersion ?? deviceRow.client_version ?? fallback.deviceInfo.clientVersion ?? ''
      ),
      sysSoftware: String(deviceRow.sysSoftware ?? deviceRow.sys_software ?? fallback.deviceInfo.sysSoftware ?? ''),
      network: String(deviceRow.network ?? fallback.deviceInfo.network ?? 'wifi'),
      memory: String(deviceRow.memory ?? fallback.deviceInfo.memory ?? ''),
      deviceId: String(deviceRow.deviceId ?? deviceRow.device_id ?? fallback.deviceInfo.deviceId ?? ''),
      userAgent: String(deviceRow.userAgent ?? deviceRow.user_agent ?? fallback.deviceInfo.userAgent ?? '')
    }
  };
}

const DEFAULT_DEVICE_INFO: DeviceInfoPayload = {
  os: 'Windows',
  clientVersion: '',
  sysSoftware: 'Windows',
  network: 'wifi',
  memory: '16384',
  deviceId: 'DESKTOP-PC<WPC>',
  userAgent: ''
};

const DEFAULT_SYSTEM_CONFIG: SystemConfigPayload = {
  serverUrl: '',
  clientVersion: '',
  platform: 'qq',
  os: 'Windows',
  timeZone: 'Asia/Shanghai',
  deviceInfo: { ...DEFAULT_DEVICE_INFO }
};

export async function fetchGetDevicePresets() {
  const res = await invokeFlat<any[]>('get_device_presets');
  if (res.error) return res;
  return { ...res, data: Array.isArray(res.data) ? res.data : [] };
}

export async function fetchGetSystemConfig() {
  const res = await invokeFlat<any>('get_system_config');
  if (res.error) return res as FlatResponseData<any, SystemConfigResponse>;
  const raw = res.data || {};
  const fallback = normalizeSystemConfig(raw.default, DEFAULT_SYSTEM_CONFIG);
  return {
    data: {
      saved: raw.saved ? normalizeSystemConfig(raw.saved, fallback) : null,
      default: fallback,
      current: raw.current
    },
    error: null,
    response: {} as any
  };
}

export function fetchSetSystemConfig(cfg: SystemConfigPayload) {
  return invokeFlat('set_system_config', { cfg });
}

export async function fetchResetSystemConfig() {
  const res = await invokeFlat<any>('reset_system_config');
  if (res.error) return res;
  const raw = res.data || {};
  const fallback = normalizeSystemConfig(raw.default, DEFAULT_SYSTEM_CONFIG);
  return {
    data: {
      saved: raw.saved ? normalizeSystemConfig(raw.saved, fallback) : null,
      default: fallback,
      current: raw.current
    },
    error: null,
    response: {} as any
  };
}

function toQqBotBinding(raw: any): Api.Farm.QqBotBinding {
  return {
    userOpenid: String(raw?.userOpenid ?? raw?.user_openid ?? ''),
    boundAt: Number(raw?.boundAt ?? raw?.bound_at ?? 0) || undefined,
    nickname: String(raw?.nickname ?? '')
  };
}

function toOfflineReminder(raw: any): Api.Farm.OfflineReminder {
  const provider = String(raw?.provider ?? 'none');
  return {
    provider: (['qq_bot', 'wechat_bot', 'ding_talk'].includes(provider)
      ? provider
      : 'none') as Api.Farm.OfflineReminder['provider'],
    qqBot: {
      appId: String(raw?.qqBot?.appId ?? ''),
      clientSecret: String(raw?.qqBot?.clientSecret ?? '')
    },
    qqBotBinding: toQqBotBinding(raw?.qqBotBinding ?? raw?.qq_bot_binding ?? {}),
    wechatBot: {},
    title: String(raw?.title ?? ''),
    msg: String(raw?.msg ?? ''),
    offlineDeleteSec: Number(raw?.offlineDeleteSec ?? raw?.offline_delete_sec ?? 0),
    endpoint: String(raw?.endpoint ?? ''),
    token: String(raw?.token ?? ''),
    secret: String(raw?.secret ?? '')
  };
}

export async function fetchGetOfflineReminder() {
  const res = await invokeFlat<any>('get_offline_reminder');
  if (res.error) return res as FlatResponseData<any, Api.Farm.OfflineReminder>;
  return { data: toOfflineReminder(res.data), error: null, response: {} as any };
}

export async function fetchSaveOfflineReminder(cfg: Api.Farm.OfflineReminder) {
  const res = await invokeFlat<any>('set_offline_reminder', { cfg });
  if (res.error) return res as FlatResponseData<any, Api.Farm.OfflineReminder>;
  return { data: toOfflineReminder(res.data), error: null, response: {} as any };
}

export async function fetchTestOfflineReminder(cfg: Api.Farm.OfflineReminder) {
  return invokeFlat<{ ok: boolean; code?: string; msg?: string }>('test_offline_reminder', { cfg });
}

export async function fetchGetQqBotBindStatus() {
  const res = await invokeFlat<any>('get_qq_bot_bind_status');
  if (res.error) return res as FlatResponseData<any, Api.Farm.QqBotBindStatus>;
  const raw = res.data || {};
  return {
    data: {
      credentialsConfigured: Boolean(raw?.credentialsConfigured),
      bound: Boolean(raw?.bound),
      binding: toQqBotBinding(raw?.binding ?? {}),
      botInviteUrl: String(raw?.botInviteUrl ?? '')
    } satisfies Api.Farm.QqBotBindStatus,
    error: null,
    response: {} as any
  };
}

export async function fetchStartQqBotBind() {
  const res = await invokeFlat<any>('start_qq_bot_bind');
  if (res.error) return res as FlatResponseData<any, Api.Farm.QqBotBindStart>;
  const raw = res.data || {};
  return {
    data: {
      sessionId: String(raw?.sessionId ?? ''),
      botInviteUrl: String(raw?.botInviteUrl ?? ''),
      qrDataUrl: String(raw?.qrDataUrl ?? ''),
      expiresAt: Number(raw?.expiresAt ?? 0)
    } satisfies Api.Farm.QqBotBindStart,
    error: null,
    response: {} as any
  };
}

export async function fetchPollQqBotBind(sessionId: string) {
  const res = await invokeFlat<any>('poll_qq_bot_bind', { sessionId });
  if (res.error) return res as FlatResponseData<any, Api.Farm.QqBotBindPoll>;
  const raw = res.data || {};
  return {
    data: {
      status: ['pending', 'bound', 'expired'].includes(String(raw?.status)) ? raw.status : 'expired',
      binding: raw?.binding ? toQqBotBinding(raw.binding) : null
    } satisfies Api.Farm.QqBotBindPoll,
    error: null,
    response: {} as any
  };
}

export async function fetchUnbindQqBot() {
  const res = await invokeFlat<any>('unbind_qq_bot');
  if (res.error) return res as FlatResponseData<any, Api.Farm.OfflineReminder>;
  return { data: toOfflineReminder(res.data), error: null, response: {} as any };
}

export function fetchGetFarmLands(accountId: number) {
  return invokeFlat('farm_lands', { accountId: aid(accountId) });
}

export function fetchFarmOperate(data: Api.Farm.OperateParams) {
  return invokeFlat('farm_operate', {
    accountId: aid(data.accountId),
    op: String((data as any).op || '')
  });
}

export function fetchGetFarmBag(accountId: number) {
  return invokeFlat('farm_bag', { accountId: aid(accountId) });
}

/** Static seed catalog (no account worker required). */
export async function fetchGetFarmSeeds(_accountId?: number) {
  const res = await invokeFlat<any>('farm_seeds', { accountId: null });
  if (res.error) return res;
  const list = Array.isArray(res.data) ? res.data.map(normalizeSeedRow) : res.data;
  return { data: list, error: null, response: {} as any };
}

export async function fetchGetFarmBagSeeds(accountId: number) {
  const res = await invokeFlat<any>('farm_seeds', { accountId: aid(accountId) });
  if (res.error) return res;
  const list = Array.isArray(res.data) ? res.data.map(normalizeSeedRow) : res.data;
  return { data: list, error: null, response: {} as any };
}

export function fetchSellFarmBag(data: Api.Farm.BagSellParams) {
  const items = (data.items || []).map((it: any) => ({
    itemId: Number(it.itemId ?? it.id ?? 0),
    count: Number(it.count ?? 0),
    uid: Number(it.uid ?? 0)
  }));
  return invokeFlat('farm_bag_sell', { accountId: aid(data.accountId), items });
}

export function fetchUseFarmBag(data: Api.Farm.BagUseParams) {
  return invokeFlat('farm_bag_use', {
    accountId: aid(data.accountId),
    itemId: Number((data as any).itemId ?? (data as any).id ?? 0),
    count: Number((data as any).count ?? 1),
    uid: Number((data as any).uid ?? 0)
  });
}

export function fetchGetFarmDailyGifts(accountId: number) {
  return invokeFlat<Api.Farm.DailyGiftsResponse>('farm_daily_gifts', {
    accountId: aid(accountId)
  });
}

export async function fetchGetFarmFriendList(params?: Api.Farm.FriendSearchParams & { force?: boolean }) {
  const accountId = params?.accountId;
  if (accountId == null) {
    return { data: { records: [], current: 1, size: 50, total: 0 }, error: null, response: {} as any };
  }
  const res = await invokeFlat<FriendSummaryDto[]>('friend_list', {
    accountId: aid(accountId),
    force: Boolean(params?.force)
  });
  if (res.error) return res;
  const arr = Array.isArray(res.data) ? res.data : [];
  let records = arr.map(f => toFriendRecord(f, Number(accountId)));
  if (params?.keyword) {
    const kw = String(params.keyword).toLowerCase();
    records = records.filter(
      (f: Api.Farm.Friend) => String(f.nickname).toLowerCase().includes(kw) || String(f.gid).includes(kw)
    );
  }
  return {
    data: { records, current: 1, size: records.length || 50, total: records.length },
    error: null,
    response: {} as any
  };
}

export function fetchGetFarmFriendLands(params: Api.Farm.FriendLandsParams) {
  return invokeFlat('friend_lands', {
    accountId: aid(params.accountId),
    gid: Number(params.gid)
  });
}

export function fetchFarmFriendOp(data: Api.Farm.FriendOpParams) {
  return invokeFlat('friend_op', {
    accountId: aid(data.accountId),
    gid: Number(data.gid),
    op: String(data.op)
  });
}

export function fetchGetFarmFriendInteractRecords(accountId: number) {
  return invokeFlat('friend_interact_records', { accountId: aid(accountId) });
}

export function fetchGetFarmActivitySnapshot(accountId: number) {
  return invokeFlat<Api.Farm.ActivitySnapshot>('activity_snapshot', {
    accountId: aid(accountId)
  });
}

export function fetchClaimFarmActivityPass(data: Api.Farm.ActivityClaimParams) {
  return invokeFlat('activity_claim_battle_pass', { accountId: aid(data.accountId) });
}

export function fetchLightFarmActivityConstellation(data: Api.Farm.ActivityClaimParams) {
  return invokeFlat('activity_light_constellation', { accountId: aid(data.accountId) });
}

export function fetchExchangeFarmActivityShop(data: Api.Farm.ActivityClaimParams) {
  const body = data as any;
  return invokeFlat('activity_exchange_star_sand', {
    accountId: aid(data.accountId),
    goodsId: body.itemId ?? body.goodsId ?? body.id,
    count: body.count ?? 1
  });
}

export function fetchClaimFarmActivitySolarTerm(data: Api.Farm.ActivityClaimParams) {
  const body = data as any;
  return invokeFlat('activity_claim_solar_term', {
    accountId: aid(data.accountId),
    termId: String(body.termId ?? body.id ?? '')
  });
}

export function fetchClaimFarmActivityGreenPlum(data: Api.Farm.ActivityClaimParams) {
  return invokeFlat('activity_claim_qingmei_seed', { accountId: aid(data.accountId) });
}

export function fetchStartFarmActivityGreenPlumBrew(data: Api.Farm.ActivityClaimParams) {
  const body = data as any;
  return invokeFlat('activity_qingmei_brew_start', {
    accountId: aid(data.accountId),
    input: { ingredients: body.ingredients || [] }
  });
}

export function fetchContinueFarmActivityGreenPlumBrew(data: Api.Farm.ActivityClaimParams) {
  return invokeFlat('activity_qingmei_brew_continue', { accountId: aid(data.accountId) });
}

export function fetchSettleFarmActivityGreenPlumBrew(data: Api.Farm.ActivityClaimParams) {
  return invokeFlat('activity_qingmei_brew_settle', { accountId: aid(data.accountId) });
}

export function fetchClaimFarmActivityQixiBridge(data: Api.Farm.ActivityClaimParams) {
  return invokeFlat('activity_claim_qixi_bridge', { accountId: aid(data.accountId) });
}

export function fetchGiftFarmActivityQixiSachet(data: Api.Farm.ActivityClaimParams) {
  const body = data as any;
  const count = Math.trunc(Number(body.sachetCount ?? body.count ?? 0));
  return invokeFlat('activity_gift_qixi_sachet', {
    accountId: aid(data.accountId),
    friendGid: body.friendGid,
    sachetCount: count,
    count
  });
}

export function fetchGetFarmActivityCharity(data: Api.Farm.ActivityClaimParams) {
  return invokeFlat('activity_get_charity', { accountId: aid(data.accountId) });
}

export function fetchClaimFarmActivityCharitySeeds(data: Api.Farm.ActivityClaimParams) {
  return invokeFlat('activity_claim_charity_seeds', { accountId: aid(data.accountId) });
}

export function fetchDonateFarmActivityCharityLove(data: Api.Farm.ActivityClaimParams) {
  return invokeFlat('activity_donate_charity_love', { accountId: aid(data.accountId) });
}

export function fetchClaimFarmActivityCharityDailyGift(data: Api.Farm.ActivityClaimParams) {
  return invokeFlat('activity_claim_charity_daily_gift', { accountId: aid(data.accountId) });
}

export function fetchGetFarmActivityPetDiary(accountId: number) {
  return invokeFlat('activity_get_pet_diary', { accountId: aid(accountId) });
}

export function fetchOperateFarmActivityPetDiary(
  accountId: number,
  action: string,
  params: Record<string, unknown>
) {
  return invokeFlat('activity_operate_pet_diary', {
    accountId: aid(accountId),
    action,
    params
  });
}

export function fetchGetFarmActivityPetDiaryRecords(accountId: number, kind: 'interact' | 'plunder') {
  return invokeFlat('activity_get_pet_diary_records', { accountId: aid(accountId), kind });
}

export function fetchGetFarmActivityPetDiaryFriend(accountId: number, gid: string) {
  return invokeFlat('activity_get_pet_diary_friend', { accountId: aid(accountId), gid });
}

export function fetchClaimFarmActivityCharityProgressReward(data: Api.Farm.ActivityClaimParams) {
  const body = data as any;
  return invokeFlat('activity_claim_charity_progress_reward', {
    accountId: aid(data.accountId),
    target: String(body.target ?? '')
  });
}

export async function fetchGetFarmAnalyticsDetail(params?: any) {
  const res = await invokeFlat<any>('farm_analytics', {
    sortBy: params?.sortBy ?? params?.sort ?? null
  });
  if (res.error) return res;
  const rankings = Array.isArray(res.data) ? res.data : Array.isArray(res.data?.rankings) ? res.data.rankings : [];
  return { data: { rankings }, error: null, response: {} as any };
}

export function fetchGetFarmGameConfigSeeds() {
  return invokeFlat<Api.Farm.GameConfigSeed[]>('config_list_seeds');
}

export function fetchGetFarmGameConfigFruits() {
  return invokeFlat<Api.Farm.GameConfigFruit[]>('config_list_fruits');
}

export async function fetchGetFarmGameConfigItems(params?: { type?: number }) {
  const res = await invokeFlat<any[]>('config_list_items');
  if (res.error) return res as FlatResponseData<any, Api.Farm.GameConfigItem[]>;
  let list = Array.isArray(res.data) ? res.data : [];
  if (params?.type != null) {
    list = list.filter((it: any) => Number(it.type ?? it.itemType ?? it.item_type) === Number(params.type));
  }
  return { data: list, error: null, response: {} as any };
}

export function fetchGetFarmGameConfigPlants() {
  return invokeFlat('config_list_plants');
}

export async function fetchGetFarmGameConfigItemTypes() {
  const res = await invokeFlat<any>('config_list_item_types');
  if (res.error) return res;
  const raw = res.data;
  let list: Array<{ label: string; value: number }> = [];
  if (Array.isArray(raw)) {
    list = raw.map((t: any) => ({
      label: String(t.label ?? t.name ?? t.value ?? ''),
      value: Number(t.value ?? t.id ?? 0)
    }));
  } else if (raw && typeof raw === 'object') {
    list = Object.keys(raw).map(k => ({ label: `类型 ${k}`, value: Number(k) }));
  }
  return { data: list, error: null, response: {} as any };
}

export function fetchAddFarmGameConfigSeed(data: Api.Farm.GameConfigSeedWriteParams) {
  return invokeFlat('config_add', { kind: 'seed', payload: data });
}

export function fetchModifyFarmGameConfigSeed(data: Api.Farm.GameConfigSeedWriteParams) {
  return invokeFlat('config_modify', {
    kind: 'seed',
    id: String((data as any).seedId ?? (data as any).id ?? ''),
    payload: data
  });
}

export function fetchDeleteFarmGameConfigSeed(seedId: number) {
  return invokeFlat('config_delete', { kind: 'seed', id: String(seedId) });
}

export function fetchAddFarmGameConfigFruit(data: Api.Farm.GameConfigFruitWriteParams) {
  return invokeFlat('config_add', { kind: 'fruit', payload: data });
}

export function fetchModifyFarmGameConfigFruit(data: Api.Farm.GameConfigFruitWriteParams) {
  return invokeFlat('config_modify', {
    kind: 'fruit',
    id: String((data as any).id ?? ''),
    payload: data
  });
}

export function fetchDeleteFarmGameConfigFruit(id: number) {
  return invokeFlat('config_delete', { kind: 'fruit', id: String(id) });
}

export function fetchAddFarmGameConfigItem(data: Api.Farm.GameConfigItemWriteParams) {
  return invokeFlat('config_add', { kind: 'item', payload: data });
}

export function fetchModifyFarmGameConfigItem(data: Api.Farm.GameConfigItemWriteParams) {
  return invokeFlat('config_modify', {
    kind: 'item',
    id: String((data as any).id ?? ''),
    payload: data
  });
}

export function fetchDeleteFarmGameConfigItem(id: number) {
  return invokeFlat('config_delete', { kind: 'item', id: String(id) });
}

export function fetchGetFarmGameMall(
  accountIdOrParams: number | { accountId: number; slotType?: number; subSlotType?: number }
) {
  const params = typeof accountIdOrParams === 'number' ? { accountId: accountIdOrParams } : accountIdOrParams;
  return invokeFlat('commerce_mall_catalog', {
    accountId: aid(params.accountId),
    slotType: params.slotType ?? null,
    subSlotType: params.subSlotType ?? null
  });
}

export function fetchPurchaseFarmGameMall(data: any) {
  return invokeFlat('commerce_mall_purchase', {
    accountId: aid(data.accountId),
    goodsId: Number(data.goodsId ?? data.id ?? 0),
    count: Number(data.count ?? 1)
  });
}

export async function fetchGetFarmDiamond(accountId: number) {
  const res = await invokeFlat<any>('farm_diamond', { accountId: aid(accountId) });
  if (res.error) return res;
  const diamond = typeof res.data === 'number' ? res.data : Number(res.data?.diamond ?? 0);
  return { data: { diamond }, error: null, response: {} as any };
}

export function fetchGetFarmMysteryShop(accountId: number) {
  return invokeFlat('commerce_mystery_shop', { accountId: aid(accountId) });
}

export function fetchPurchaseFarmMysteryShop(data: any) {
  return invokeFlat('commerce_mystery_purchase', {
    accountId: aid(data.accountId),
    offerId: String(data.offerId ?? data.npcId ?? data.id ?? '')
  });
}

export function fetchGetFarmSettings(accountId?: number | string) {
  return invokeFlat('get_settings_panel', { accountId: accountId != null ? aid(accountId) : '' });
}

export function fetchSaveFarmSettings(accountId: number | string, snapshot: unknown) {
  return invokeFlat('save_settings', { accountId: aid(accountId), snapshot });
}

// ============ 宠物 / 同气连枝礼包 ============

export function fetchGetPetInfo(accountId: number) {
  return invokeFlat('pet_info', { accountId: aid(accountId) });
}

export function fetchPetDeploy(accountId: number, dogId: number) {
  return invokeFlat('pet_deploy', { accountId: aid(accountId), dogId: Number(dogId) });
}

export function fetchPetWithdraw(accountId: number) {
  return invokeFlat('pet_withdraw', { accountId: aid(accountId) });
}

export function fetchPetFoodUse(accountId: number, itemId: number, count = 1, uid = 0) {
  return invokeFlat('pet_food_use', {
    accountId: aid(accountId),
    itemId: Number(itemId),
    count: Number(count),
    uid: Number(uid)
  });
}

export function fetchGetPetProtectLogs(accountId: number) {
  return invokeFlat('pet_protect_logs', { accountId: aid(accountId) });
}

export function fetchGetDogSkillGifts(accountId: number) {
  return invokeFlat('dog_skill_gifts_status', { accountId: aid(accountId) });
}

export function fetchClaimDogSkillGifts(accountId: number) {
  return invokeFlat('dog_skill_gifts_claim', { accountId: aid(accountId) });
}

// ============ 好友互动道具 / 图鉴 ============

export function fetchGetFriendInteractionItems(accountId: number) {
  return invokeFlat('friend_interaction_items', { accountId: aid(accountId) });
}

export function fetchUseFriendInteractionItems(
  accountId: number,
  friendGid: number,
  itemId: number,
  landIds: number[]
) {
  return invokeFlat('friend_interaction_items_use', {
    accountId: aid(accountId),
    friendGid: Number(friendGid),
    itemId: Number(itemId),
    landIds: landIds.map(Number)
  });
}

export function fetchGetFarmInteractionItems(accountId: number) {
  return invokeFlat('farm_interaction_items', { accountId: aid(accountId) });
}

export function fetchUseFarmInteractionItems(accountId: number, itemId: number, landIds: number[]) {
  return invokeFlat('farm_interaction_items_use', {
    accountId: aid(accountId),
    itemId: Number(itemId),
    landIds: landIds.map(Number)
  });
}

export function fetchGetIllustratedSnapshot(accountId: number) {
  return invokeFlat('illustrated_snapshot', { accountId: aid(accountId) });
}

// ============ 天气活动「雨落成诗」 ============

export function fetchGetWeatherSnapshot(accountId: number) {
  return invokeFlat('weather_snapshot', { accountId: aid(accountId) });
}

export function fetchGetWeatherFriends(accountId: number) {
  return invokeFlat('weather_friends', { accountId: aid(accountId) });
}

export function fetchScanWeatherFriends(accountId: number, friendGids: string[]) {
  return invokeFlat(
    'weather_friends_scan',
    { accountId: aid(accountId), friendGids: friendGids.map(g => String(g)) },
    { silent: true }
  );
}

export function fetchExchangeWeatherCollector(accountId: number) {
  return invokeFlat('weather_exchange_collector', { accountId: aid(accountId) }, { silent: true });
}

export function fetchCollectWeather(accountId: number, friendGid: string) {
  return invokeFlat('weather_collect', { accountId: aid(accountId), friendGid: String(friendGid) }, { silent: true });
}

export function fetchSummonWeather(accountId: number) {
  return invokeFlat('weather_summon', { accountId: aid(accountId) }, { silent: true });
}

export function fetchWeatherMischiefFrog(accountId: number, friendGid: string) {
  return invokeFlat(
    'weather_mischief_frog',
    { accountId: aid(accountId), friendGid: String(friendGid) },
    { silent: true }
  );
}

export function fetchWeatherMischiefCloud(accountId: number, friendGid: string, landId?: string) {
  return invokeFlat(
    'weather_mischief_cloud',
    {
      accountId: aid(accountId),
      friendGid: String(friendGid),
      landId: landId != null && String(landId) !== '' ? String(landId) : null
    },
    { silent: true }
  );
}

export function fetchAdvanceWeatherResearch(accountId: number, nodeId: string) {
  return invokeFlat(
    'weather_advance_research',
    { accountId: aid(accountId), nodeId: String(nodeId) },
    { silent: true }
  );
}

/** 游戏内删除好友（后端会同时加入黑名单）。 */
export function fetchDeleteFarmFriend(accountId: number, gid: number | string) {
  return invokeFlat('friend_delete', { accountId: aid(accountId), gid: String(gid) });
}

/** Route stubs (static mode — unused but imported by route store) */
export async function fetchGetConstantRoutes() {
  return { data: [], error: null, response: {} as any };
}

export async function fetchGetUserRoutes() {
  return {
    data: { routes: [] as any[], home: 'home' as const },
    error: null,
    response: {} as any
  } as any;
}

export async function fetchIsRouteExist(_routeName?: string) {
  return { data: true, error: null, response: {} as any };
}
