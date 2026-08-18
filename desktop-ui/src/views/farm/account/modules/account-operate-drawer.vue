<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { farmPlatformOptions, translateStringOptions } from '@/constants/business';
import {
  fetchAddFarmAccount,
  fetchConfirmFarmWxLogin,
  fetchCreateFarmWxLoginTask,
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
type LoginTab = 'code' | 'wx_qr';

const model = ref<Model>(createDefaultModel());
const urlHint = ref('');
const activeLoginTab = ref<LoginTab>('code');
const wxTaskId = ref('');
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
  if (activeLoginTab.value !== 'wx_qr') {
    base.code = defaultRequiredRule;
  }
  return base;
});

const platformOptions = computed(() => translateStringOptions(farmPlatformOptions));
const isAddMode = computed(() => props.operateType === 'add');
const isWxQrTab = computed(() => activeLoginTab.value === 'wx_qr');

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
  wxStatus.value = '';
  wxError.value = '';
  wxQrUrl.value = '';
  wxLoading.value = false;
  wxSubmitting.value = false;
}

async function getWxCodeAndSave() {
  wxSubmitting.value = true;
  wxStatus.value = '正在获取登录 Code...';
  const { data, error } = await fetchFarmWxLoginCode(wxTaskId.value);
  if (error || !data?.code) {
    throw new Error((error as any)?.message || '未获取到登录 Code');
  }
  const code = String(data.code).trim();
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
  const next = (tab === 'wx_qr' ? 'wx_qr' : 'code') as LoginTab;
  activeLoginTab.value = next;
  if (next === 'wx_qr') {
    model.value.platform = 'wx';
    void startWxLogin();
  } else {
    resetWxLogin();
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
  if (activeLoginTab.value === 'wx_qr') {
    window.$message?.info(isAddMode.value ? '请使用微信扫码完成添加' : '请使用微信扫码完成更新');
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
          <NTab name="wx_qr" tab="微信扫码" />
        </NTabs>

        <template v-if="!isWxQrTab">
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
          <div class="mb-12px flex flex-col items-center gap-12px">
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
          <NButton v-if="!isWxQrTab" type="primary" @click="handleSubmit">
            {{ $t('common.confirm') }}
          </NButton>
        </NSpace>
      </template>
    </NDrawerContent>
  </NDrawer>
</template>

<style scoped></style>
