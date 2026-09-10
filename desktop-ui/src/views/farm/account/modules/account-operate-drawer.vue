<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { farmPlatformOptions, translateStringOptions } from '@/constants/business';
import {
  fetchAddFarmAccount,
  fetchConfirmFarmWxLogin,
  fetchConfirmFarmWxQuickLogin,
  fetchCreateFarmWxLoginTask,
  fetchCreateFarmWxQuickLoginSession,
  fetchFarmWxLoginCode,
  fetchFarmWxLoginStatus,
  fetchGetQqLoginSettings,
  fetchModifyFarmAccount,
  fetchQqLoginCancelTask,
  fetchQqLoginCreateTask,
  fetchQqLoginMiniappCode,
  fetchQqLoginTaskStatus,
  fetchStartFarmAccount,
  fetchWxLocalAuthorize,
  fetchWxLocalCheckLogin
} from '@/service/api';
import { useFormRules, useNaiveForm } from '@/hooks/common/form';
import { $t } from '@/locales';

defineOptions({
  name: 'FarmAccountOperateDrawer'
});

interface Props {
  operateType: NaiveUI.TableOperateType;
  rowData?: Api.Farm.Account | null;
}

const props = defineProps<Props>();

interface Emits {
  (e: 'submitted'): void;
}

const emit = defineEmits<Emits>();

const visible = defineModel<boolean>('visible', { default: false });

const { formRef, validate, restoreValidation } = useNaiveForm();
const { defaultRequiredRule } = useFormRules();

const title = computed(() => {
  const titles: Record<NaiveUI.TableOperateType, string> = {
    add: $t('page.farm.account.addAccount'),
    edit: $t('page.farm.account.editAccount')
  };
  return titles[props.operateType];
});

type Model = Api.Farm.AccountCreateParams & Partial<Pick<Api.Farm.AccountUpdateParams, 'id' | 'status'>>;
type LoginTab = 'code' | 'wx' | 'qq';
type WxMode = 'local' | 'qr';

const model = ref<Model>(createDefaultModel());
const urlHint = ref('');
const activeLoginTab = ref<LoginTab>('code');
const wxMode = ref<WxMode>('local');
const wxTaskId = ref('');
const wxSessionId = ref('');
const wxQuickPort = ref<number | null>(null);
const wxQuickProfile = ref<{ authorizeUuid?: string; nickname?: string; headimgurl?: string } | null>(null);
const wxStatus = ref('');
const wxError = ref('');
const wxHint = ref('');
const wxLoading = ref(false);
const wxQrUrl = ref('');
const wxSubmitting = ref(false);
let wxPollTimer: ReturnType<typeof setTimeout> | undefined;
let wxQrStartPromise: Promise<void> | undefined;

// ===== QQ 扫码登录（NapCat 对接）=====
const qqQrAvailable = ref(false);
const qqTaskId = ref('');
const qqQrUrl = ref('');
const qqStatus = ref('');
const qqError = ref('');
const qqLoading = ref(false);
const qqSubmitting = ref(false);
let qqPollTimer: ReturnType<typeof setTimeout> | undefined;

// 前端调用本机微信 HTTP 插件所需的 OAuth 参数（create session 返回）
const wxQuickOauth = ref<{ appId: string; scope: string; redirectUri: string; state: string } | null>(null);
function createDefaultModel(): Model {
  return {
    code: '',
    name: '',
    platform: 'qq',
    remark: '',
    status: '1'
  };
}

const rules = computed<Record<string, App.Global.FormRule | App.Global.FormRule[]>>(() => {
  const base: Record<string, App.Global.FormRule | App.Global.FormRule[]> = {
    platform: defaultRequiredRule
  };
  if (activeLoginTab.value !== 'wx' && activeLoginTab.value !== 'qq') {
    base.code = defaultRequiredRule;
  }
  return base;
});

const platformOptions = computed(() => translateStringOptions(farmPlatformOptions));
const isAddMode = computed(() => props.operateType === 'add');
const isWxTab = computed(() => activeLoginTab.value === 'wx');
const isWxLocalMode = computed(() => wxMode.value === 'local');
const isQqTab = computed(() => activeLoginTab.value === 'qq');

function decodeParam(value: string | null | undefined): string {
  const raw = String(value || '').trim();
  if (!raw) return '';
  try {
    return decodeURIComponent(raw);
  } catch {
    return raw;
  }
}

function looksLikeLoginUrl(raw: string): boolean {
  return /^https?:\/\//i.test(raw) || /^wss?:\/\//i.test(raw) || /[?&](?:code|platform|os|ver)=/i.test(raw);
}

function parseLoginInput(rawInput: string): {
  code: string;
  platform: '' | Api.Farm.Platform;
  os: string;
  ver: string;
} {
  const raw = String(rawInput || '').trim();
  const result: { code: string; platform: '' | Api.Farm.Platform; os: string; ver: string } = {
    code: raw,
    platform: '',
    os: '',
    ver: ''
  };
  if (!raw || !looksLikeLoginUrl(raw)) {
    return result;
  }

  try {
    let href = raw;
    if (!/^[a-z][a-z0-9+.-]*:/i.test(href)) {
      href = href.startsWith('/') ? `http://127.0.0.1${href}` : `http://127.0.0.1/prod/ws?${href.replace(/^\?/, '')}`;
    }
    const url = new URL(href);
    const code = decodeParam(url.searchParams.get('code'));
    const platform = decodeParam(url.searchParams.get('platform')).toLowerCase();
    result.os = decodeParam(url.searchParams.get('os'));
    result.ver = decodeParam(url.searchParams.get('ver'));
    if (code) result.code = code;
    if (platform === 'qq' || platform === 'wx') result.platform = platform;
    return result;
  } catch {
    const codeMatch = raw.match(/[?&]code=([^&\s#]+)/i);
    if (codeMatch?.[1]) result.code = decodeParam(codeMatch[1]);
    const platformMatch = raw.match(/[?&]platform=([^&\s#]+)/i);
    if (platformMatch?.[1]) {
      const platform = decodeParam(platformMatch[1]).toLowerCase();
      if (platform === 'qq' || platform === 'wx') result.platform = platform;
    }
    const osMatch = raw.match(/[?&]os=([^&\s#]+)/i);
    if (osMatch?.[1]) result.os = decodeParam(osMatch[1]);
    const verMatch = raw.match(/[?&]ver=([^&\s#]+)/i);
    if (verMatch?.[1]) result.ver = decodeParam(verMatch[1]);
    return result;
  }
}

function onCodeInput(value: string | null) {
  const raw = String(value ?? '');
  model.value.code = raw;
  if (!looksLikeLoginUrl(raw)) {
    urlHint.value = '';
    return;
  }
  const parsed = parseLoginInput(raw);
  if (parsed.platform) {
    model.value.platform = parsed.platform;
  }
  const parts: string[] = [];
  if (parsed.platform) {
    parts.push(`${$t('page.farm.account.platform')} ${parsed.platform === 'wx' ? '微信' : 'QQ'}`);
  }
  if (parsed.os) parts.push(`os ${parsed.os}`);
  if (parsed.ver) parts.push(`ver ${parsed.ver}`);
  urlHint.value = parts.length ? $t('page.farm.account.urlHint', { detail: parts.join(' / ') }) : '';
}

function stopWxPolling() {
  if (wxPollTimer) {
    clearTimeout(wxPollTimer);
    wxPollTimer = undefined;
  }
}

function resetWxLogin() {
  stopWxPolling();
  wxTaskId.value = '';
  wxSessionId.value = '';
  wxQuickPort.value = null;
  wxQuickProfile.value = null;
  wxQuickOauth.value = null;
  wxStatus.value = '';
  wxError.value = '';
  wxHint.value = '';
  wxQrUrl.value = '';
  wxLoading.value = false;
  wxSubmitting.value = false;
}

/** 对齐官方快捷登录交互：先探本机微信，全部失败自动回退扫码。 */
async function startWxAuthFlow() {
  wxMode.value = 'local';
  resetWxLogin();
  await detectLocalWechat();
}

function switchToQrLogin() {
  wxMode.value = 'qr';
  wxSessionId.value = '';
  wxQuickPort.value = null;
  wxQuickProfile.value = null;
  wxError.value = '';
  wxHint.value = '';
  wxStatus.value = '';
  void startWxLogin();
}

function redetectLocalWechat() {
  void startWxAuthFlow();
}

async function fallbackToQrLogin(hint = '') {
  const qrAlreadyReady = wxMode.value === 'qr' && Boolean(wxTaskId.value && wxQrUrl.value);
  wxMode.value = 'qr';
  wxSessionId.value = '';
  wxQuickPort.value = null;
  wxQuickProfile.value = null;
  wxError.value = '';
  wxHint.value = '';
  if (!qrAlreadyReady) {
    wxStatus.value = '';
    await startWxLogin();
  }
  // startWxLogin 内部会 reset，回退原因等二维码就绪后再展示
  if (hint) {
    wxHint.value = hint;
  }
}

function authorizePosition() {
  const width = 360;
  const height = 263;
  const left = window.screenX || window.screenLeft || 0;
  const top = window.screenY || window.screenTop || 0;
  return {
    x: Math.round(left + (window.outerWidth || window.innerWidth) / 2 - width / 2),
    y: Math.round(top + (window.outerHeight || window.innerHeight) / 2 - height / 2)
  };
}

async function saveWxCode(codeInput: string) {
  wxSubmitting.value = true;
  wxStatus.value = '正在保存账号...';
  try {
    const code = String(codeInput).trim();
    const name = String(model.value.name || '').trim();
    const remark = model.value.remark;
    if (props.operateType === 'edit') {
      if (!model.value.id) {
        throw new Error('账号信息不完整');
      }
      const { error: modifyError } = await fetchModifyFarmAccount({
        id: model.value.id,
        code,
        name,
        platform: 'wx',
        remark,
        status: (Number(model.value.status || 1) === 2 ? 2 : 1) as unknown as Api.Farm.EnableStatus
      });
      if (modifyError) {
        throw new Error((modifyError as any)?.message || '更新账号失败');
      }
      // Backend also auto-starts on code refresh; call start as a safety net for stopped accounts.
      const { error: startError } = await fetchStartFarmAccount(model.value.id);
      if (startError) {
        window.$message?.warning($t('common.updateSuccess') + '，自动启动失败，请手动重新登录');
      } else {
        window.$message?.success($t('common.updateSuccess') + '，已自动启动');
      }
    } else {
      const { data: added, error: addError } = await fetchAddFarmAccount({
        name,
        code,
        platform: 'wx',
        remark
      });
      if (addError) {
        throw new Error((addError as any)?.message || '保存账号失败');
      }
      // QR add: start running immediately (code-paste add still requires manual start).
      if (added?.id) {
        const { error: startError } = await fetchStartFarmAccount(added.id);
        if (startError) {
          window.$message?.warning($t('common.addSuccess') + '，自动启动失败，请手动点击启动');
        } else {
          window.$message?.success($t('common.addSuccess') + '，已自动启动');
        }
      } else {
        window.$message?.success($t('common.addSuccess'));
      }
    }
    closeDrawer();
    emit('submitted');
  } finally {
    wxSubmitting.value = false;
  }
}

async function getWxCodeAndSave() {
  const { data, error } = await fetchFarmWxLoginCode(wxTaskId.value);
  if (error || !data?.code) {
    throw new Error((error as any)?.message || '未获取到登录 Code');
  }
  await saveWxCode(String(data.code));
}

// ===== QQ 扫码登录（NapCat 对接，对齐 bot AccountModal 扫码页签）=====

function stopQqPolling() {
  if (qqPollTimer) {
    clearTimeout(qqPollTimer);
    qqPollTimer = undefined;
  }
}

function resetQqLogin() {
  stopQqPolling();
  qqTaskId.value = '';
  qqQrUrl.value = '';
  qqStatus.value = '';
  qqError.value = '';
  qqLoading.value = false;
  qqSubmitting.value = false;
}

async function loadQqLoginAvailability() {
  try {
    const { data } = await fetchGetQqLoginSettings();
    qqQrAvailable.value = Boolean(data?.qqQrLogin);
  } catch {
    qqQrAvailable.value = false;
  }
}

/** 用登录 code 保存账号（add：保存后自动启动；edit：更新后自动启动）。 */
async function saveAccountWithLoginCode(code: string, platform: 'qq' | 'wx') {
  const name = String(model.value.name || '').trim();
  const remark = model.value.remark || '';
  if (props.operateType !== 'add' && model.value.id) {
    const { error: modifyError } = await fetchModifyFarmAccount({
      id: model.value.id,
      code,
      name,
      platform,
      remark,
      status: (Number(model.value.status || 1) === 2 ? 2 : 1) as unknown as Api.Farm.EnableStatus
    });
    if (modifyError) {
      throw new Error((modifyError as any)?.message || '更新账号失败');
    }
    const { error: startError } = await fetchStartFarmAccount(model.value.id);
    if (startError) {
      window.$message?.warning($t('common.updateSuccess') + '，自动启动失败，请手动重新登录');
    } else {
      window.$message?.success($t('common.updateSuccess') + '，已自动启动');
    }
  } else {
    const { data: added, error: addError } = await fetchAddFarmAccount({ code, name, platform, remark });
    if (addError) {
      throw new Error((addError as any)?.message || '保存账号失败');
    }
    if (added?.id) {
      const { error: startError } = await fetchStartFarmAccount(added.id);
      if (startError) {
        window.$message?.warning($t('common.addSuccess') + '，自动启动失败，请手动点击启动');
      } else {
        window.$message?.success($t('common.addSuccess') + '，已自动启动');
      }
    } else {
      window.$message?.success($t('common.addSuccess'));
    }
  }
  resetQqLogin();
  closeDrawer();
  emit('submitted');
}

async function pollQqLogin() {
  if (!qqTaskId.value) return;
  try {
    const { data, error } = await fetchQqLoginTaskStatus(qqTaskId.value);
    if (error) {
      qqError.value = (error as any)?.message || '登录状态检查失败';
      return;
    }
    const status = String(data?.status || '');
    if (status === 'waiting_scan') qqStatus.value = '等待 QQ 扫码';
    else if (status === 'scanned') qqStatus.value = '已扫码，请在手机上确认';
    else if (status === 'confirmed') {
      stopQqPolling();
      await getQqCodeAndSave();
      return;
    } else if (['cancelled', 'expired', 'failed'].includes(status)) {
      qqError.value = '二维码已失效，请重新获取';
      return;
    }
    qqPollTimer = setTimeout(pollQqLogin, 1200);
  } catch (err: any) {
    qqError.value = err?.message || '登录状态检查失败';
  }
}

async function startQqLogin() {
  stopQqPolling();
  resetQqLogin();
  qqLoading.value = true;
  try {
    const { data, error } = await fetchQqLoginCreateTask();
    if (error || !data?.taskId || !data?.qrImage) {
      throw new Error((error as any)?.message || '未获取到 QQ 登录二维码');
    }
    qqTaskId.value = String(data.taskId);
    qqQrUrl.value = String(data.qrImage);
    qqStatus.value = '等待 QQ 扫码';
    void pollQqLogin();
  } catch (err: any) {
    qqError.value = err?.message || '二维码获取失败';
  } finally {
    qqLoading.value = false;
  }
}

async function cancelQqLogin() {
  if (!qqTaskId.value) return;
  try {
    await fetchQqLoginCancelTask(qqTaskId.value);
  } catch {
    // 尽力而为
  }
  resetQqLogin();
}

async function getQqCodeAndSave() {
  qqSubmitting.value = true;
  try {
    const { data, error } = await fetchQqLoginMiniappCode(qqTaskId.value);
    if (error || !data?.code) {
      throw new Error((error as any)?.message || '未获取到 QQ 登录 Code');
    }
    await saveAccountWithLoginCode(String(data.code), 'qq');
  } catch (err: any) {
    qqError.value = err?.message || 'QQ 登录失败';
  } finally {
    qqSubmitting.value = false;
  }
}

async function detectLocalWechat() {
  wxLoading.value = true;
  wxError.value = '';
  wxQuickPort.value = null;
  wxQuickProfile.value = null;
  wxStatus.value = '正在检测本机微信...';
  try {
    const { data, error } = await fetchCreateFarmWxQuickLoginSession();
    if (error || !data?.sessionId) {
      throw new Error((error as any)?.message || '创建快速授权会话失败');
    }
    wxSessionId.value = String(data.sessionId);
    wxQuickOauth.value = {
      appId: String(data.appId),
      scope: String(data.scope),
      redirectUri: String(data.redirectUri),
      state: String(data.state)
    };
    // 各平台统一通过 HTTP 插件请求，避免 WebView 的跨域限制。
    const oauth = wxQuickOauth.value;
    const ports = (data.ports || []).map(Number);
    if (!ports.length) {
      throw new Error('没有可探测的微信本地端口');
    }
    const probes = await Promise.allSettled(ports.map(port => fetchWxLocalCheckLogin(port, oauth)));
    const errors: string[] = [];
    let hit: { port: number; authorizeUuid: string; nickname: string; headimgurl: string } | null = null;
    for (const [index, probe] of probes.entries()) {
      const port = ports[index]!;
      if (probe.status !== 'fulfilled') {
        errors.push(`端口 ${port}: ${probe.reason?.message || '连接失败'}`);
        continue;
      }
      const payload = probe.value;
      const jsdata = payload.jsdata && typeof payload.jsdata === 'object' ? payload.jsdata : null;
      const uuid = jsdata ? String(jsdata.authorize_uuid || '').trim() : '';
      if (payload.errcode === 0 && uuid) {
        hit = hit ?? {
          port,
          authorizeUuid: uuid,
          nickname: String(jsdata?.nickname || ''),
          headimgurl: String(jsdata?.headimgurl || '')
        };
      } else if (payload.errcode) {
        errors.push(`端口 ${port}: errcode=${payload.errcode}`);
      } else {
        errors.push(`端口 ${port}: 无授权信息`);
      }
    }
    if (!hit) {
      throw new Error(errors[0] || '未检测到可用的桌面微信');
    }
    wxQuickPort.value = hit.port;
    wxQuickProfile.value = {
      authorizeUuid: hit.authorizeUuid,
      nickname: hit.nickname,
      headimgurl: hit.headimgurl
    };
    wxStatus.value = hit.nickname ? `${hit.nickname} · 请在电脑微信中确认` : '本机微信已就绪，请点击授权';
  } catch (err: any) {
    // 保留探测失败的具体原因（端口/errcode/连接错误），便于诊断
    const reason = String(err?.message || '').trim() || '未检测到本机微信';
    await fallbackToQrLogin(`未检测到本机微信：${reason}。请扫码登录`);
  } finally {
    wxLoading.value = false;
  }
}

async function authorizeLocalWechat() {
  const port = wxQuickPort.value;
  const profile = wxQuickProfile.value;
  if (!port || !profile?.authorizeUuid || !wxSessionId.value) {
    await fallbackToQrLogin();
    return;
  }
  wxSubmitting.value = true;
  wxError.value = '';
  wxStatus.value = '等待电脑微信确认...';
  let quickCode = '';
  try {
    const pos = authorizePosition();
    // 授权与检测使用相同的 HTTP 插件通道。
    const oauth = wxQuickOauth.value;
    if (!oauth) {
      throw new Error('授权会话缺少 OAuth 参数，请重新检测');
    }
    const authorized = await fetchWxLocalAuthorize(port, oauth, profile.authorizeUuid, { x: pos.x, y: pos.y });
    if (authorized.errcode !== 0) {
      const errMap: Record<number, string> = {
        10050: '已在微信中拒绝授权',
        10046: '授权已超时，请重新检测',
        10057: '当前应用仅支持扫码授权'
      };
      throw new Error(errMap[authorized.errcode] || `桌面微信未返回有效授权结果（errcode=${authorized.errcode}）`);
    }
    const redirectUrl = String(
      authorized.jsdata && typeof authorized.jsdata === 'object'
        ? (authorized.jsdata as Record<string, unknown>).redirect_url
        : ''
    ).trim();
    if (!redirectUrl) {
      throw new Error('桌面微信未返回有效授权结果');
    }
    const { data, error } = await fetchConfirmFarmWxQuickLogin(wxSessionId.value, redirectUrl);
    if (error || !data?.code) {
      throw new Error((error as any)?.message || '快速授权确认失败');
    }
    quickCode = String(data.code);
  } catch (err: any) {
    await fallbackToQrLogin(err?.message || '本机微信授权失败，请扫码登录');
    return;
  } finally {
    wxSubmitting.value = false;
  }
  try {
    await saveWxCode(quickCode);
  } catch (err: any) {
    wxError.value = err?.message || '保存账号失败';
    wxStatus.value = '保存账号失败';
  }
}

async function confirmWxLogin() {
  wxStatus.value = '正在建立登录会话...';
  const { error } = await fetchConfirmFarmWxLogin(wxTaskId.value);
  if (error) {
    throw new Error((error as any)?.message || '确认登录失败');
  }
  await getWxCodeAndSave();
}

async function pollWxLogin() {
  if (!wxTaskId.value) return;
  try {
    const { data, error } = await fetchFarmWxLoginStatus(wxTaskId.value);
    if (error) {
      wxError.value = (error as any)?.message || '登录状态检查失败';
      return;
    }
    const status = data?.status;
    if (status === 'waiting') wxStatus.value = '等待微信扫码';
    else if (status === 'scanned') wxStatus.value = '已扫码，请在手机上确认';
    else if (status === 'authorized') {
      stopWxPolling();
      await confirmWxLogin();
      return;
    } else if (['cancelled', 'expired', 'failed'].includes(String(status))) {
      wxError.value = '二维码已失效，请重新获取';
      return;
    }
    wxPollTimer = setTimeout(pollWxLogin, 1200);
  } catch (err: any) {
    wxError.value = err?.message || '登录状态检查失败';
  }
}

async function runWxLoginStart() {
  resetWxLogin();
  wxLoading.value = true;
  model.value.platform = 'wx';
  try {
    const { data, error } = await fetchCreateFarmWxLoginTask();
    const payload = data as any;
    if (error || !payload?.taskId) {
      throw new Error((error as any)?.message || '未创建登录任务');
    }
    wxTaskId.value = String(payload.taskId);
    const b64 = String(payload.qrJpegBase64 || '');
    if (!b64) {
      throw new Error('未返回二维码');
    }
    wxQrUrl.value = b64.startsWith('data:') ? b64 : `data:image/jpeg;base64,${b64}`;
    wxStatus.value = '等待微信扫码';
    void pollWxLogin();
  } catch (err: any) {
    wxError.value = err?.message || '二维码获取失败';
  } finally {
    wxLoading.value = false;
  }
}

async function startWxLogin() {
  if (!wxQrStartPromise) {
    wxQrStartPromise = runWxLoginStart();
  }
  const pending = wxQrStartPromise;
  try {
    await pending;
  } finally {
    if (wxQrStartPromise === pending) {
      wxQrStartPromise = undefined;
    }
  }
}

function onLoginTabChange(tab: string | number) {
  const next = (['wx', 'qq'].includes(String(tab)) ? tab : 'code') as LoginTab;
  activeLoginTab.value = next;
  if (next === 'wx') {
    model.value.platform = 'wx';
    resetQqLogin();
    void startWxAuthFlow();
  } else if (next === 'qq') {
    model.value.platform = 'qq';
    resetWxLogin();
    void startQqLogin();
  } else {
    resetWxLogin();
    resetQqLogin();
  }
}

function handleInitModel() {
  model.value = createDefaultModel();
  urlHint.value = '';
  activeLoginTab.value = 'code';
  resetWxLogin();
  resetQqLogin();
  void loadQqLoginAvailability();

  if (props.operateType === 'edit' && props.rowData) {
    const { id, name, code, platform, remark, status } = props.rowData;
    Object.assign(model.value, {
      id,
      name: name || '',
      code: code || '',
      platform: platform || 'qq',
      remark: remark || '',
      status: status !== undefined && status !== null ? (String(status) as Api.Farm.EnableStatus) : '1'
    });
  }
}

function closeDrawer() {
  resetWxLogin();
  visible.value = false;
}

async function handleSubmit() {
  if (activeLoginTab.value === 'wx') {
    window.$message?.info(isAddMode.value ? '请使用微信授权完成添加' : '请使用微信授权完成更新');
    return;
  }
  if (activeLoginTab.value === 'qq') {
    window.$message?.info(isAddMode.value ? '请使用 QQ 扫码完成添加' : '请使用 QQ 扫码完成更新');
    return;
  }

  try {
    await validate();
  } catch {
    return;
  }

  const rawInput = String(model.value.code || '').trim();
  if (!rawInput) {
    window.$message?.warning($t('page.farm.account.codeRequired'));
    return;
  }

  const parsed = parseLoginInput(rawInput);
  const codeForApi = looksLikeLoginUrl(rawInput) ? rawInput : parsed.code || rawInput;
  const platform = parsed.platform || model.value.platform || 'qq';

  if (props.operateType === 'add') {
    const { error } = await fetchAddFarmAccount({
      code: codeForApi,
      name: String(model.value.name || '').trim(),
      platform,
      remark: model.value.remark
    });

    if (!error) {
      window.$message?.success($t('common.addSuccess'));
      closeDrawer();
      emit('submitted');
    }
  } else {
    const { error } = await fetchModifyFarmAccount({
      id: model.value.id!,
      code: codeForApi,
      name: String(model.value.name || '').trim(),
      platform,
      remark: model.value.remark,
      status: (Number(model.value.status || 1) === 2 ? 2 : 1) as unknown as Api.Farm.EnableStatus
    });

    if (!error) {
      window.$message?.success($t('common.updateSuccess'));
      closeDrawer();
      emit('submitted');
    }
  }
}

watch(visible, () => {
  if (visible.value) {
    handleInitModel();
    restoreValidation();
  } else {
    resetWxLogin();
    resetQqLogin();
  }
});

onBeforeUnmount(() => {
  resetWxLogin();
  resetQqLogin();
});
</script>

<template>
  <NDrawer v-model:show="visible" display-directive="if" to="body" :width="420">
    <NDrawerContent :title="title" :native-scrollbar="false" closable>
      <NForm ref="formRef" :model="model" :rules="rules" label-placement="top">
        <NFormItem :label="$t('page.farm.account.name')" path="name">
          <NInput v-model:value="model.name" :placeholder="$t('page.farm.account.namePlaceholder')" />
        </NFormItem>

        <NTabs :value="activeLoginTab" type="segment" class="mb-12px" @update:value="onLoginTabChange">
          <NTab name="code" tab="输入 code" />
          <NTab name="wx" tab="微信授权" />
          <NTab v-if="qqQrAvailable" name="qq" tab="QQ 扫码" />
        </NTabs>

        <template v-if="isQqTab">
          <!-- QQ 扫码登录：NapCat 出码，1.2s 轮询，confirmed 后换 code 登录 -->
          <div class="mb-12px flex flex-col items-center gap-12px">
            <NSpin :show="qqLoading || qqSubmitting">
              <div class="h-220px w-220px flex items-center justify-center overflow-hidden rounded-8px bg-#f5f5f5">
                <img v-if="qqQrUrl" :src="qqQrUrl" alt="QQ 登录二维码" class="h-full w-full object-contain" />
                <span v-else class="text-13px text-#999">二维码加载中</span>
              </div>
            </NSpin>
            <p class="text-13px text-primary">{{ qqStatus || '准备扫码登录' }}</p>
            <p v-if="qqError" class="text-13px text-error">{{ qqError }}</p>
            <NSpace>
              <NButton size="small" :loading="qqLoading" @click="startQqLogin">刷新二维码</NButton>
              <NButton v-if="qqTaskId" size="small" quaternary @click="cancelQqLogin">取消登录</NButton>
            </NSpace>
          </div>
        </template>

        <template v-else-if="!isWxTab">
          <NFormItem :label="$t('page.farm.account.code')" path="code">
            <NInput
              :value="model.code"
              type="textarea"
              :rows="4"
              :placeholder="$t('page.farm.account.codePlaceholder')"
              @update:value="onCodeInput"
            />
          </NFormItem>
          <p v-if="urlHint" class="mb-12px text-12px text-primary">{{ urlHint }}</p>
          <NFormItem :label="$t('page.farm.account.platform')" path="platform">
            <NRadioGroup v-model:value="model.platform">
              <NSpace>
                <NRadio v-for="item in platformOptions" :key="item.value" :value="item.value" :label="item.label" />
              </NSpace>
            </NRadioGroup>
          </NFormItem>
        </template>

        <template v-else>
          <!-- 对齐官方微信快捷登录：自动探测本机微信，命中显示快捷登录，全部失败回退扫码 -->
          <div v-if="isWxLocalMode" class="mb-12px flex flex-col items-center gap-12px">
            <NSpin :show="wxSubmitting">
              <div
                class="min-h-180px w-full flex flex-col items-center justify-center gap-8px rounded-8px bg-#f5f5f5 p-16px"
              >
                <template v-if="wxQuickProfile">
                  <img
                    v-if="wxQuickProfile.headimgurl"
                    :src="wxQuickProfile.headimgurl"
                    alt="微信头像"
                    class="h-72px w-72px rounded-full object-cover"
                  />
                  <span v-else class="text-40px">微</span>
                  <p class="text-14px font-600">{{ wxQuickProfile.nickname || '本机微信' }}</p>
                  <p class="text-center text-13px text-#666">{{ wxStatus }}</p>
                </template>
                <template v-else>
                  <span class="text-40px">微</span>
                  <p class="text-center text-13px text-#666">{{ wxStatus || '正在检测本机微信...' }}</p>
                </template>
              </div>
            </NSpin>
            <p v-if="wxError" class="text-13px text-error">{{ wxError }}</p>
            <NSpace justify="center" :wrap="true">
              <NButton
                v-if="wxQuickProfile"
                type="success"
                size="small"
                :loading="wxSubmitting"
                :disabled="!wxQuickPort"
                @click="authorizeLocalWechat"
              >
                微信快捷登录
              </NButton>
              <NButton size="small" secondary @click="switchToQrLogin">切换扫码登录</NButton>
            </NSpace>
          </div>

          <div v-else class="mb-12px flex flex-col items-center gap-12px">
            <NSpin :show="wxLoading || wxSubmitting">
              <div class="h-220px w-220px flex items-center justify-center overflow-hidden rounded-8px bg-#f5f5f5">
                <img v-if="wxQrUrl" :src="wxQrUrl" alt="微信登录二维码" class="h-full w-full object-contain" />
                <span v-else class="text-13px text-#999">二维码加载中</span>
              </div>
            </NSpin>
            <p class="text-13px text-primary">{{ wxStatus || '准备扫码登录' }}</p>
            <p v-if="wxHint" class="text-center text-12px text-#999">{{ wxHint }}</p>
            <p v-if="wxError" class="text-13px text-error">{{ wxError }}</p>
            <NSpace>
              <NButton size="small" :loading="wxLoading" @click="startWxLogin">刷新二维码</NButton>
              <NButton size="small" quaternary @click="redetectLocalWechat">重新检测本机微信</NButton>
            </NSpace>
          </div>
        </template>

        <NFormItem :label="$t('page.farm.account.remark')" path="remark">
          <NInput v-model:value="model.remark" type="textarea" :placeholder="$t('page.farm.account.remark')" />
        </NFormItem>
      </NForm>
      <template #footer>
        <NSpace :size="16">
          <NButton @click="closeDrawer">{{ $t('common.cancel') }}</NButton>
          <NButton v-if="!isWxTab && !isQqTab" type="primary" @click="handleSubmit">
            {{ $t('common.confirm') }}
          </NButton>
        </NSpace>
      </template>
    </NDrawerContent>
  </NDrawer>
</template>

<style scoped></style>
