<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { useFullscreen } from '@vueuse/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { GLOBAL_HEADER_MENU_ID } from '@/constants/app';
import { useAppStore } from '@/store/modules/app';
import { useThemeStore } from '@/store/modules/theme';
import { isTauriRuntime } from '@/service/tauri/client';
import { desktopIsFullscreen, desktopStartDragging, desktopToggleFullscreen, isDesktopWindows, isDesktopShell } from '@/utils/desktop';
import AppUpdateDialog from '@/components/common/app-update-dialog.vue';
import FarmAccountSwitcher from '../global-sider/components/farm-account-switcher.vue';
import SearchModal from '../global-search/components/search-modal.vue';
import GlobalLogo from '../global-logo/index.vue';
import GlobalBreadcrumb from '../global-breadcrumb/index.vue';
import GlobalSearch from '../global-search/index.vue';
import ThemeButton from './components/theme-button.vue';
import WindowControls from './components/window-controls.vue';

defineOptions({
  name: 'GlobalHeader'
});

interface Props {
  /** Whether to show the logo */
  showLogo?: App.Global.HeaderProps['showLogo'];
  /** Whether to show the menu toggler */
  showMenuToggler?: App.Global.HeaderProps['showMenuToggler'];
  /** Whether to show the menu */
  showMenu?: App.Global.HeaderProps['showMenu'];
}

defineProps<Props>();

const appStore = useAppStore();
const themeStore = useThemeStore();
const webFullscreen = useFullscreen();
const nativeFullscreen = ref(false);
const updateVisible = ref(false);
const searchVisible = ref(false);
const en = computed(() => appStore.locale === 'en-US');
const moreOptions = computed(() => [
  ...(appStore.isMobile ? [
    ...(themeStore.header.globalSearch.visible ? [{ key: 'search', label: en.value ? 'Search pages' : '搜索页面' }] : []),
    { key: 'theme', label: en.value ? 'Theme' : '主题模式', children: [
      { key: 'auto', label: `${themeStore.themeScheme === 'auto' ? '✓ ' : ''}${en.value ? 'Follow system' : '跟随系统'}` },
      { key: 'light', label: `${themeStore.themeScheme === 'light' ? '✓ ' : ''}${en.value ? 'Light' : '浅色'}` },
      { key: 'dark', label: `${themeStore.themeScheme === 'dark' ? '✓ ' : ''}${en.value ? 'Dark' : '深色'}` }
    ] },
    { key: 'settings', label: en.value ? 'Theme settings' : '主题设置' },
    ...(themeStore.header.multilingual.visible ? [{ key: 'language', label: en.value ? '切换中文' : 'Switch to English' }] : [])
  ] : []),
  { key: 'updates', label: en.value ? 'About & updates' : '关于与更新' }
]);

function handleMore(key: string) {
  if (key === 'updates') updateVisible.value = true;
  else if (key === 'search') searchVisible.value = true;
  else if (key === 'settings') appStore.openThemeDrawer();
  else if (key === 'language') appStore.changeLocale(en.value ? 'zh-CN' : 'en-US');
  else if (key === 'auto' || key === 'light' || key === 'dark') themeStore.setThemeScheme(key);
}
let unlistenResize: (() => void) | undefined;

const isFullscreen = computed(() => (isDesktopShell.value ? nativeFullscreen.value : webFullscreen.isFullscreen.value));

async function syncNativeFullscreen() {
  nativeFullscreen.value = await desktopIsFullscreen();
}

async function toggleFullscreen() {
  if (isDesktopShell.value) {
    nativeFullscreen.value = await desktopToggleFullscreen();
    return;
  }
  await webFullscreen.toggle();
}

onMounted(async () => {
  if (!isDesktopShell.value) return;
  await syncNativeFullscreen();
  unlistenResize = await getCurrentWindow().onResized(() => {
    void syncNativeFullscreen();
  });
});

onUnmounted(() => {
  unlistenResize?.();
});

const windowsDesktop = computed(() => isDesktopWindows());

function startWindowDrag(event: MouseEvent) {
  if (!isDesktopShell.value || event.button !== 0) return;
  const target = event.target;
  if (
    !(target instanceof Element) ||
    target.closest('a, button, input, textarea, select, [contenteditable="true"], [role="button"], .desktop-no-drag')
  ) {
    return;
  }
  void desktopStartDragging();
}
</script>

<template>
  <DarkModeContainer
    class="h-full flex-y-center px-12px shadow-header desktop-drag-region"
    @mousedown="startWindowDrag"
  >
    <GlobalLogo v-if="showLogo" class="h-full" :style="{ width: themeStore.sider.width + 'px' }" />
    <MenuToggler
      v-if="showMenuToggler"
      class="desktop-no-drag"
      :collapsed="appStore.siderCollapse"
      @click="appStore.toggleSiderCollapse"
    />
    <div v-if="showMenu" :id="GLOBAL_HEADER_MENU_ID" class="h-full flex-y-center flex-1-hidden"></div>
    <div v-else class="h-full flex-y-center flex-1-hidden">
      <GlobalBreadcrumb v-if="!appStore.isMobile" class="ml-12px" />
      <FarmAccountSwitcher v-else class="min-w-0 flex-1 desktop-no-drag" />
    </div>
    <div class="h-full flex-y-center justify-end desktop-no-drag">
      <GlobalSearch v-if="!appStore.isMobile && themeStore.header.globalSearch.visible" />
      <FullScreen v-if="!appStore.isMobile && (isDesktopShell || !isTauriRuntime())" :full="isFullscreen" @click="toggleFullscreen" />
      <LangSwitch
        v-if="!appStore.isMobile && themeStore.header.multilingual.visible"
        :lang="appStore.locale"
        :lang-options="appStore.localeOptions"
        @change-lang="appStore.changeLocale"
      />
      <ThemeSchemaSwitch
        v-if="!appStore.isMobile"
        :theme-schema="themeStore.themeScheme"
        :is-dark="themeStore.darkMode"
        @switch="themeStore.toggleThemeScheme"
      />
      <ThemeButton v-if="!appStore.isMobile" />
      <NDropdown trigger="click" :options="moreOptions" @select="handleMore">
        <ButtonIcon icon="mdi:dots-vertical" :aria-label="en ? 'More' : '更多'" :tooltip-content="en ? 'More' : '更多'" />
      </NDropdown>
      <WindowControls v-if="windowsDesktop" />
    </div>
  </DarkModeContainer>
  <AppUpdateDialog v-model:show="updateVisible" />
  <SearchModal v-model:show="searchVisible" />
</template>

<style scoped></style>
