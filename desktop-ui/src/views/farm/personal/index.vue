<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';
import { NCard, NEmpty, NTabPane, NTabs } from 'naive-ui';
import { fetchGetFarmStatusDetail } from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { $t } from '@/locales';
import BagPanel from './BagPanel.vue';
import FarmPanel from './FarmPanel.vue';
import TaskPanel from './TaskPanel.vue';

defineOptions({ name: 'FarmPersonal' });

const farmAccountStore = useFarmAccountStore();
const statusLoading = ref(false);
const connected = ref(false);
const activeTab = ref<'farm' | 'bag' | 'task'>('farm');

const farmPanelRef = ref<{ refresh: () => Promise<void> } | null>(null);
const bagPanelRef = ref<{ refresh: () => Promise<void> } | null>(null);
const taskPanelRef = ref<{ refresh: () => Promise<void> } | null>(null);

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
  await Promise.all([farmPanelRef.value?.refresh?.(), bagPanelRef.value?.refresh?.(), taskPanelRef.value?.refresh?.()]);
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
      <NTabs v-model:value="activeTab" type="segment" size="medium" animated>
        <NTabPane name="farm" :tab="$t('page.farm.personal.tabFarm')">
          <FarmPanel ref="farmPanelRef" :connected="connected" />
        </NTabPane>
        <NTabPane name="bag" :tab="$t('page.farm.personal.tabBag')">
          <BagPanel ref="bagPanelRef" :connected="connected" />
        </NTabPane>
        <NTabPane name="task" :tab="$t('page.farm.personal.tabTask')">
          <TaskPanel ref="taskPanelRef" :connected="connected" />
        </NTabPane>
      </NTabs>
    </template>
  </div>
</template>
