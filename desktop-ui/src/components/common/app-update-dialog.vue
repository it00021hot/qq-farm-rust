<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { openUrl } from '@tauri-apps/plugin-opener';
import { checkAppUpdate, getAppInfo } from '@/service/api/application';
import type { AppInfo, UpdateResult } from '@/service/api/application';
import { useAppStore } from '@/store/modules/app';

const visible = defineModel<boolean>('show', { default: false });
const appStore = useAppStore();
const en = computed(() => appStore.locale === 'en-US');
const info = ref<AppInfo>();
const loading = ref(false);
const opening = ref(false);
const error = ref('');
const result = ref<UpdateResult>();
const release = computed(() => (result.value?.kind === 'release' ? result.value : undefined));

watch(visible, async show => {
  if (!show || info.value) return;
  try {
    info.value = await getAppInfo();
  } catch (e) {
    error.value = String(e);
  }
});

async function check() {
  if (loading.value) return;
  loading.value = true;
  error.value = '';
  result.value = undefined;
  try {
    info.value = await getAppInfo();
    result.value = await checkAppUpdate();
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function download() {
  if (!release.value?.hasApk || opening.value) return;
  opening.value = true;
  try {
    await openUrl(release.value.releaseUrl);
  } catch (e) {
    error.value = String(e);
  } finally {
    opening.value = false;
  }
}
</script>

<template>
  <NModal v-model:show="visible" preset="card" class="app-update-dialog" :title="en ? 'About & updates' : '关于与更新'">
    <div class="flex flex-col gap-16px" aria-live="polite">
      <div class="text-20px font-semibold">QQ Farm</div>
      <div>{{ en ? 'Installed version' : '当前版本' }}：{{ info?.version ?? '—' }}</div>
      <NAlert v-if="error" type="error" :title="en ? 'Request failed. Please retry.' : '操作失败，请重试'">
        {{ error }}
      </NAlert>
      <template v-if="release">
        <NAlert :type="release.available ? 'info' : 'success'">
          {{
            release.available
              ? en
                ? 'New version available'
                : '发现新版本'
              : en
                ? 'No newer version available'
                : '当前已无更新版本'
          }}
          <span v-if="release.available">：{{ release.version }}</span>
        </NAlert>
        <NAlert v-if="release.available && !release.hasApk" type="warning">
          {{ en ? 'This release does not include an Android APK yet.' : '此版本尚未提供安卓 APK 安装包。' }}
        </NAlert>
        <NButton v-if="release.available && release.hasApk" type="primary" :loading="opening" @click="download">
          {{ en ? 'Go to download' : '前往下载' }}
        </NButton>
      </template>
      <NButton :loading="loading" :disabled="loading" @click="check">
        {{ en ? 'Check for updates' : '检查更新' }}
      </NButton>
    </div>
  </NModal>
</template>

<style scoped>
.app-update-dialog {
  width: min(480px, calc(100vw - 24px));
}
</style>
