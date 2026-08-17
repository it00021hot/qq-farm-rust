<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { useFullscreen } from '@vueuse/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { GLOBAL_HEADER_MENU_ID } from '@/constants/app';
import { useAppStore } from '@/store/modules/app';
import { useThemeStore } from '@/store/modules/theme';
import { isTauriRuntime } from '@/service/tauri/client';
import {
  desktopIsFullscreen,
  desktopToggleFullscreen,
  isDesktopWindows
} from '@/utils/desktop';
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
let unlistenResize: (() => void) | undefined;

const isFullscreen = computed(() =>
  isTauriRuntime() ? nativeFullscreen.value : webFullscreen.isFullscreen.value
);

async function syncNativeFullscreen() {
  nativeFullscreen.value = await desktopIsFullscreen();
}

async function toggleFullscreen() {
  if (isTauriRuntime()) {
    nativeFullscreen.value = await desktopToggleFullscreen();
    return;
  }
  await webFullscreen.toggle();
}

onMounted(async () => {
  if (!isTauriRuntime()) return;
  await syncNativeFullscreen();
  unlistenResize = await getCurrentWindow().onResized(() => {
    void syncNativeFullscreen();
  });
});

onUnmounted(() => {
  unlistenResize?.();
});

const windowsDesktop = computed(() => isDesktopWindows());
</script>

<template>
  <DarkModeContainer class="h-full flex-y-center px-12px shadow-header desktop-drag-region">
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
    </div>
    <div class="h-full flex-y-center justify-end desktop-no-drag">
      <GlobalSearch v-if="themeStore.header.globalSearch.visible" />
      <FullScreen v-if="!appStore.isMobile" :full="isFullscreen" @click="toggleFullscreen" />
      <LangSwitch
        v-if="themeStore.header.multilingual.visible"
        :lang="appStore.locale"
        :lang-options="appStore.localeOptions"
        @change-lang="appStore.changeLocale"
      />
      <ThemeSchemaSwitch
        :theme-schema="themeStore.themeScheme"
        :is-dark="themeStore.darkMode"
        @switch="themeStore.toggleThemeScheme"
      />
      <ThemeButton />
      <WindowControls v-if="windowsDesktop" />
    </div>
  </DarkModeContainer>
</template>

<style scoped></style>
