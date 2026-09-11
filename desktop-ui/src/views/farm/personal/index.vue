<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';
import { NCard, NEmpty, NTabPane, NTabs } from 'naive-ui';
import { fetchGetFarmStatusDetail } from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { $t } from '@/locales';
import { useAppStore } from '@/store/modules/app';
import BagPanel from './BagPanel.vue';
import FarmPanel from './FarmPanel.vue';
import IllustratedPanel from './IllustratedPanel.vue';
import InteractionItemsPanel from './InteractionItemsPanel.vue';
import PetPanel from './PetPanel.vue';
import TaskPanel from './TaskPanel.vue';

const appStore = useAppStore();

defineOptions({ name: 'FarmPersonal' });

const farmAccountStore = useFarmAccountStore();
const statusLoading = ref(false);
const connected = ref(false);
const activeTab = ref<'farm' | 'bag' | 'task' | 'pet' | 'illustrated' | 'interaction'>('farm');

const farmPanelRef = ref<{ refresh: () => Promise<void> } | null>(null);
const bagPanelRef = ref<{ refresh: () => Promise<void> } | null>(null);
const taskPanelRef = ref<{ refresh: () => Promise<void> } | null>(null);
const petPanelRef = ref<{ refresh: () => Promise<void> } | null>(null);
const illustratedPanelRef = ref<{ refresh: () => Promise<void> } | null>(null);
const interactionPanelRef = ref<{ refresh: () => Promise<void> } | null>(null);

async function loadStatus() {
  if (!farmAccountStore.currentAccountId) {
    connected.value = false;
    return;
  }
  statusLoading.value = true;
  try {
    const { data, error } = await fetchGetFarmStatusDetail(farmAccountStore.currentAccountId);
    connected.value = !error && data?.runStatus === 1;
  } finally {
    statusLoading.value = false;
  }
}

async function refreshAll() {
  await farmAccountStore.loadAccounts();
  await loadStatus();
  if (!connected.value) return;
  await Promise.all([
    farmPanelRef.value?.refresh?.(),
    bagPanelRef.value?.refresh?.(),
    taskPanelRef.value?.refresh?.(),
    petPanelRef.value?.refresh?.(),
    illustratedPanelRef.value?.refresh?.(),
    interactionPanelRef.value?.refresh?.()
  ]);
}

watch(
  () => farmAccountStore.currentAccountId,
  () => {
    void refreshAll();
  }
);

onMounted(async () => {
  await refreshAll();
});
</script>

<template>
  <div class="min-h-500px flex-col-stretch gap-16px overflow-auto">
    <h2 class="text-18px font-medium">{{ $t('page.farm.personal.title') }}</h2>

    <NCard v-if="!farmAccountStore.currentAccountId" :bordered="false" size="small" class="card-wrapper">
      <NEmpty :description="$t('page.farm.common.selectAccount')" />
    </NCard>

    <NCard v-else-if="!connected && !statusLoading" :bordered="false" size="small" class="card-wrapper">
      <NEmpty :description="$t('page.farm.personal.notRunning')" />
    </NCard>

    <template v-else>
      <NTabs v-model:value="activeTab" :type="appStore.isMobile ? 'line' : 'segment'" size="medium" animated>
        <NTabPane name="farm" :tab="$t('page.farm.personal.tabFarm')">
          <FarmPanel ref="farmPanelRef" :connected="connected" />
        </NTabPane>
        <NTabPane name="bag" :tab="$t('page.farm.personal.tabBag')">
          <BagPanel ref="bagPanelRef" :connected="connected" />
        </NTabPane>
        <NTabPane name="task" :tab="$t('page.farm.personal.tabTask')">
          <TaskPanel ref="taskPanelRef" :connected="connected" />
        </NTabPane>
        <NTabPane name="pet" :tab="$t('page.farm.personal.tabPet')">
          <PetPanel ref="petPanelRef" />
        </NTabPane>
        <NTabPane name="illustrated" :tab="$t('page.farm.personal.tabIllustrated')">
          <IllustratedPanel ref="illustratedPanelRef" />
        </NTabPane>
        <NTabPane name="interaction" :tab="$t('page.farm.personal.tabInteraction')">
          <InteractionItemsPanel ref="interactionPanelRef" mode="self" />
        </NTabPane>
      </NTabs>
    </template>
  </div>
</template>
