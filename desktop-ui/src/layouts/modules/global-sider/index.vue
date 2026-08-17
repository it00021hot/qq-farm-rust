<script setup lang="ts">
import { computed } from 'vue';
import { GLOBAL_SIDER_MENU_ID } from '@/constants/app';
import { useAppStore } from '@/store/modules/app';
import { useThemeStore } from '@/store/modules/theme';
import { isDesktopMac } from '@/utils/desktop';
import GlobalLogo from '../global-logo/index.vue';
import FarmAccountSwitcher from './components/farm-account-switcher.vue';

defineOptions({
  name: 'GlobalSider'
});

const appStore = useAppStore();
const themeStore = useThemeStore();

const isTopHybridSidebarFirst = computed(() => themeStore.layout.mode === 'top-hybrid-sidebar-first');
const isTopHybridHeaderFirst = computed(() => themeStore.layout.mode === 'top-hybrid-header-first');
const darkMenu = computed(
  () =>
    !themeStore.darkMode && !isTopHybridSidebarFirst.value && !isTopHybridHeaderFirst.value && themeStore.sider.inverted
);
const showLogo = computed(() => themeStore.layout.mode === 'vertical');
const macDesktop = computed(() => isDesktopMac());
</script>

<template>
  <DarkModeContainer class="size-full flex-col-stretch shadow-sider" :inverted="darkMenu">
    <!-- Mac: clear top strip for native traffic lights + window drag -->
    <div
      v-if="macDesktop"
      class="desktop-drag-region desktop-mac-traffic-spacer shrink-0"
      :style="{ height: `${themeStore.header.height}px` }"
    />

    <div :id="GLOBAL_SIDER_MENU_ID" class="flex-1-hidden desktop-no-drag"></div>

    <div v-if="!appStore.siderCollapse || showLogo" class="sider-footer desktop-no-drag shrink-0">
      <FarmAccountSwitcher v-if="!appStore.siderCollapse" />
      <GlobalLogo
        v-if="showLogo"
        class="border-t border-gray-100 dark:border-gray-800"
        :show-title="!appStore.siderCollapse"
        :style="{ height: themeStore.header.height + 'px' }"
      />
    </div>
  </DarkModeContainer>
</template>

<style scoped>
.sider-footer {
  display: flex;
  flex-direction: column;
}
</style>
