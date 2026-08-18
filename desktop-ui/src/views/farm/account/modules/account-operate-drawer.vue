<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { farmPlatformOptions, translateStringOptions } from '@/constants/business';
import {
  fetchAddFarmAccount,
  fetchAuthorizeFarmWxQuickLogin,
  fetchConfirmFarmWxLogin,
  fetchConfirmFarmWxQuickLogin,
  fetchCreateFarmWxLoginTask,
  fetchCreateFarmWxQuickLoginSession,
  fetchDetectFarmWxQuickLogin,
  fetchFarmWxLoginCode,
  fetchFarmWxLoginStatus,
  fetchModifyFarmAccount,
  fetchStartFarmAccount
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
type LoginTab = 'code' | 'wx';
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
const wxLoading = ref(false);
const wxQrUrl = ref('');
const wxSubmitting = ref(false);
let wxPollTimer: ReturnType<typeof setTimeout> | undefined;
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
  if (activeLoginTab.value !== 'wx') {
    base.code = defaultRequiredRule;
  }
  return base;
});

const platformOptions = computed(() => translateStringOptions(farmPlatformOptions));
const isAddMode = computed(() => props.operateType === 'add');
const isWxTab = computed(() => activeLoginTab.value === 'wx');
const isWxLocalMode = computed(() => wxMode.value === 'local');

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
  wxStatus.value = '';
  wxError.value = '';
  wxQrUrl.value = '';
  wxLoading.value = false;
  wxSubmitting.value = false;
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
    const detected = await fetchDetectFarmWxQuickLogin(wxSessionId.value);
    if (detected.error || !detected.data?.authorizeUuid) {
      throw new Error((detected.error as any)?.message || '未检测到可用的桌面微信');
    }
    wxQuickPort.value = Number(detected.data.port);
    wxQuickProfile.value = {
      authorizeUuid: String(detected.data.authorizeUuid),
      nickname: detected.data.nickname,
      headimgurl: detected.data.headimgurl
    };
    wxStatus.value = wxQuickProfile.value.nickname
      ? `${wxQuickProfile.value.nickname} · 请在电脑微信中确认`
      : '本机微信已就绪，请点击授权';
  } catch (err: any) {
    wxError.value = err?.message || '本机微信不可用';
    wxStatus.value = '本机快速授权不可用，已切换到扫码';
    wxMode.value = 'qr';
    void startWxLogin();
  } finally {
    wxLoading.value = false;
  }
}

async function authorizeLocalWechat() {
  const port = wxQuickPort.value;
  const profile = wxQuickProfile.value;
  if (!port || !profile?.authorizeUuid || !wxSessionId.value) {
    wxError.value = '请先检测本机微信';
    return;
  }
  wxSubmitting.value = true;
  wxError.value = '';
  wxStatus.value = '等待电脑微信确认...';
  try {
    const pos = authorizePosition();
    const authorized = await fetchAuthorizeFarmWxQuickLogin(wxSessionId.value, {
      port,
      authorizeUuid: profile.authorizeUuid,
      x: pos.x,
      y: pos.y
    });
    if (authorized.error || !authorized.data?.redirectUrl) {
      const message = (authorized.error as any)?.message || '桌面微信未返回有效授权结果';
      if (String(message).includes('仅支持扫码授权')) {
        wxMode.value = 'qr';
        void startWxLogin();
      }
      throw new Error(message);
    }
    const { data, error } = await fetchConfirmFarmWxQuickLogin(
      wxSessionId.value,
      String(authorized.data.redirectUrl)
    );
    if (error || !data?.code) {
      throw new Error((error as any)?.message || '快速授权确认失败');
    }
    await saveWxCode(String(data.code));
  } catch (err: any) {
    wxError.value = err?.message || '快速授权失败';
    wxStatus.value = '快速授权失败';
  } finally {
    wxSubmitting.value = false;
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

async function startWxLogin() {
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

function onLoginTabChange(tab: string | number) {
  const next = (tab === 'wx' ? 'wx' : 'code') as LoginTab;
  activeLoginTab.value = next;
  if (next === 'wx') {
    model.value.platform = 'wx';
    if (wxMode.value === 'local') {
      void detectLocalWechat();
    } else {
      void startWxLogin();
    }
  } else {
    resetWxLogin();
  }
}

function onWxModeChange(mode: string | number) {
  wxMode.value = mode === 'qr' ? 'qr' : 'local';
  resetWxLogin();
  if (wxMode.value === 'local') {
    void detectLocalWechat();
  } else {
    void startWxLogin();
  }
}

function handleInitModel() {
  model.value = createDefaultModel();
  urlHint.value = '';
  activeLoginTab.value = 'code';
  resetWxLogin();

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
  }
});

onBeforeUnmount(() => {
  resetWxLogin();
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
        </NTabs>

        <template v-if="!isWxTab">
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
          <NTabs :value="wxMode" type="line" class="mb-12px" @update:value="onWxModeChange">
            <NTab name="local" tab="本机微信" />
            <NTab name="qr" tab="扫码" />
          </NTabs>

          <div v-if="isWxLocalMode" class="mb-12px flex flex-col items-center gap-12px">
            <NSpin :show="wxLoading || wxSubmitting">
              <div class="min-h-180px w-full flex flex-col items-center justify-center gap-8px rounded-8px bg-#f5f5f5 p-16px">
                <img
                  v-if="wxQuickProfile?.headimgurl"
                  :src="wxQuickProfile.headimgurl"
                  alt="微信头像"
                  class="h-72px w-72px rounded-full object-cover"
                />
                <span v-else class="text-40px">微</span>
                <p class="text-14px font-600">{{ wxQuickProfile?.nickname || '本机微信' }}</p>
                <p class="text-center text-13px text-#666">{{ wxStatus || '准备检测本机微信' }}</p>
              </div>
            </NSpin>
            <p v-if="wxError" class="text-13px text-error">{{ wxError }}</p>
            <NSpace>
              <NButton
                size="small"
                type="primary"
                :loading="wxSubmitting"
                :disabled="!wxQuickPort"
                @click="authorizeLocalWechat"
              >
                使用本机微信授权
              </NButton>
              <NButton size="small" :loading="wxLoading" @click="detectLocalWechat">重新检测</NButton>
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
            <p v-if="wxError" class="text-13px text-error">{{ wxError }}</p>
            <NButton size="small" :loading="wxLoading" @click="startWxLogin">刷新二维码</NButton>
          </div>
        </template>

        <NFormItem :label="$t('page.farm.account.remark')" path="remark">
          <NInput v-model:value="model.remark" type="textarea" :placeholder="$t('page.farm.account.remark')" />
        </NFormItem>
      </NForm>
      <template #footer>
        <NSpace :size="16">
          <NButton @click="closeDrawer">{{ $t('common.cancel') }}</NButton>
          <NButton v-if="!isWxTab" type="primary" @click="handleSubmit">
            {{ $t('common.confirm') }}
          </NButton>
        </NSpace>
      </template>
    </NDrawerContent>
  </NDrawer>
</template>

<style scoped></style>
