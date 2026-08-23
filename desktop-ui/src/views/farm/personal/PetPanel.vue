<script setup lang="ts">
import { ref } from 'vue';
import {
  NButton,
  NCard,
  NDescriptions,
  NDescriptionsItem,
  NEmpty,
  NInputNumber,
  NList,
  NListItem,
  NModal,
  NSpace,
  NTag,
  NTooltip
} from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { NDataTable } from 'naive-ui';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import {
  fetchClaimDogSkillGifts,
  fetchGetDogSkillGifts,
  fetchGetPetInfo,
  fetchGetPetProtectLogs,
  fetchPetDeploy,
  fetchPetFoodUse,
  fetchPetWithdraw
} from '@/service/api';
import { $t } from '@/locales';

defineOptions({ name: 'PetPanel' });

const emit = defineEmits<{ refresh: [] }>();

const farmAccountStore = useFarmAccountStore();
const loading = ref(false);
const info = ref<any>(null);
const pendingGifts = ref(0);
const logsVisible = ref(false);
const logs = ref<any[]>([]);
const foodCounts = ref<Record<number, number>>({});

function days(seconds: number): string {
  if (!seconds || seconds <= 0) return '0 天';
  return `${Math.floor(seconds / 86400)} 天 ${Math.floor((seconds % 86400) / 3600)} 时`;
}

async function load() {
  if (!farmAccountStore.currentAccountId) return;
  loading.value = true;
  try {
    const [infoRes, giftRes] = await Promise.all([
      fetchGetPetInfo(farmAccountStore.currentAccountId),
      fetchGetDogSkillGifts(farmAccountStore.currentAccountId)
    ]);
    if (!infoRes.error) info.value = infoRes.data;
    if (!giftRes.error) pendingGifts.value = Number(giftRes.data?.pending ?? 0);
  } finally {
    loading.value = false;
  }
}

async function deploy(dogId: number) {
  if (!farmAccountStore.currentAccountId) return;
  const { error } = await fetchPetDeploy(farmAccountStore.currentAccountId, dogId);
  if (!error) {
    window.$message?.success($t('page.farm.personal.pet.deploySuccess'));
    await load();
  }
}

async function withdraw() {
  if (!farmAccountStore.currentAccountId) return;
  const { error } = await fetchPetWithdraw(farmAccountStore.currentAccountId);
  if (!error) {
    window.$message?.success($t('page.farm.personal.pet.withdrawSuccess'));
    await load();
  }
}

async function useFood(itemId: number) {
  if (!farmAccountStore.currentAccountId) return;
  const count = foodCounts.value[itemId] ?? 1;
  const { error } = await fetchPetFoodUse(farmAccountStore.currentAccountId, itemId, count);
  if (!error) {
    window.$message?.success($t('page.farm.personal.pet.foodSuccess'));
    await load();
  }
}

async function claimGifts() {
  if (!farmAccountStore.currentAccountId) return;
  const { error, data } = await fetchClaimDogSkillGifts(farmAccountStore.currentAccountId);
  if (!error) {
    window.$message?.success(`${$t('page.farm.personal.pet.giftClaimed')}: ${data?.claimed ?? 0}`);
    await load();
    emit('refresh');
  }
}

const logColumns: DataTableColumns<any> = [
  { title: $t('page.farm.personal.pet.logFriend'), key: 'friendName', width: 140 },
  { title: $t('page.farm.personal.pet.logTime'), key: 'timestamp', width: 170, render: (row: any) => new Date(Number(row.timestamp) * 1000).toLocaleString() },
  { title: $t('page.farm.personal.pet.logStolen'), key: 'stolenCount', width: 90 },
  { title: $t('page.farm.personal.pet.logProtected'), key: 'protectedGold', width: 110 }
];

async function openLogs() {
  if (!farmAccountStore.currentAccountId) return;
  const { data, error } = await fetchGetPetProtectLogs(farmAccountStore.currentAccountId);
  if (!error) {
    logs.value = data?.logs ?? [];
    logsVisible.value = true;
  }
}

defineExpose({ refresh: load });

void load();
</script>

<template>
  <div class="flex flex-col gap-12px">
    <NCard size="small" :title="$t('page.farm.personal.pet.title')">
      <template #header-extra>
        <NSpace>
          <NTag v-if="pendingGifts > 0" type="warning" round>
            {{ $t('page.farm.personal.pet.giftPending') }}: {{ pendingGifts }}
          </NTag>
          <NButton v-if="pendingGifts > 0" size="small" type="primary" @click="claimGifts">
            {{ $t('page.farm.personal.pet.giftClaim') }}
          </NButton>
          <NButton size="small" @click="openLogs">{{ $t('page.farm.personal.pet.logTitle') }}</NButton>
          <NButton size="small" @click="load">{{ $t('page.farm.personal.pet.refresh') }}</NButton>
        </NSpace>
      </template>
      <NEmpty v-if="!info" :description="$t('page.farm.personal.pet.empty')" />
      <template v-else>
        <NDescriptions :column="3" size="small" label-placement="left" class="mb-12px">
          <NDescriptionsItem :label="$t('page.farm.personal.pet.protectRemaining')">
            {{ days(info.protectDuration) }}
          </NDescriptionsItem>
          <NDescriptionsItem :label="$t('page.farm.personal.pet.protectMax')">
            {{ days(info.maxProtectDuration) }}
          </NDescriptionsItem>
          <NDescriptionsItem :label="$t('page.farm.personal.pet.activeDog')">
            {{ info.dogs?.find((d: any) => d.id === info.activeDogId)?.name ?? $t('page.farm.personal.pet.none') }}
          </NDescriptionsItem>
        </NDescriptions>

        <NList bordered size="small">
          <NListItem v-for="dog in info.dogs ?? []" :key="dog.id">
            <div class="flex items-center justify-between gap-12px">
              <div class="flex items-center gap-8px">
                <span class="font-medium">{{ dog.name }}</span>
                <NTag size="small" :type="dog.active ? 'success' : 'default'">
                  {{ dog.active ? $t('page.farm.personal.pet.onDuty') : dog.owned ? $t('page.farm.personal.pet.owned') : $t('page.farm.personal.pet.locked') }}
                </NTag>
                <NTag size="small" :bordered="false">{{ dog.rarityLabel }}</NTag>
                <NTooltip trigger="hover">
                  <template #trigger>
                    <span class="cursor-help text-12px text-gray-400">{{ dog.skillDescription?.slice(0, 12) }}…</span>
                  </template>
                  {{ dog.skillDescription }}
                </NTooltip>
              </div>
              <NButton
                v-if="dog.owned && !dog.active"
                size="tiny"
                type="primary"
                secondary
                @click="deploy(dog.id)"
              >
                {{ $t('page.farm.personal.pet.deploy') }}
              </NButton>
            </div>
          </NListItem>
        </NList>

        <div class="mt-8px flex items-center gap-8px">
          <NButton
            v-if="info.activeDogId"
            size="small"
            type="warning"
            secondary
            @click="withdraw"
          >
            {{ $t('page.farm.personal.pet.withdraw') }}
          </NButton>
        </div>
      </template>
    </NCard>

    <NCard size="small" :title="$t('page.farm.personal.pet.foodTitle')">
      <NEmpty v-if="!info?.foods?.length" :description="$t('page.farm.personal.pet.foodEmpty')" />
      <div v-else class="flex flex-col gap-8px">
        <div v-for="food in info.foods" :key="food.id" class="flex items-center gap-12px">
          <span class="w-120px truncate">{{ food.name }}</span>
          <NTag size="small" :bordered="false">{{ $t('page.farm.personal.pet.foodStock') }}: {{ food.count }}</NTag>
          <NTag size="small" :bordered="false" type="info">
            +{{ Math.floor(food.duration / 86400) }}{{ $t('page.farm.personal.pet.dayUnit') }}
          </NTag>
          <NInputNumber v-model:value="foodCounts[food.id]" size="small" :min="1" :max="99" class="w-100px" />
          <NButton size="small" :disabled="food.count <= 0" @click="useFood(food.id)">
            {{ $t('page.farm.personal.pet.foodUse') }}
          </NButton>
        </div>
      </div>
    </NCard>

    <NModal v-model:show="logsVisible" preset="card" :title="$t('page.farm.personal.pet.logTitle')" class="w-720px">
      <NDataTable :columns="logColumns" :data="logs" :max-height="420" size="small" />
    </NModal>
  </div>
</template>
