<script setup lang="ts">
import { computed, ref } from 'vue';
import {
  NCard,
  NEmpty,
  NGrid,
  NGridItem,
  NProgress,
  NTag,
  NTabs,
  NTabPane
} from 'naive-ui';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { fetchGetIllustratedSnapshot } from '@/service/api';
import { $t } from '@/locales';

defineOptions({ name: 'IllustratedPanel' });

const farmAccountStore = useFarmAccountStore();
const loading = ref(false);
const snapshot = ref<any>(null);
const book = ref<'crop' | 'mutant'>('crop');

const current = computed(() => snapshot.value?.[book.value] ?? null);

async function load() {
  if (!farmAccountStore.currentAccountId) return;
  loading.value = true;
  try {
    const { data, error } = await fetchGetIllustratedSnapshot(farmAccountStore.currentAccountId);
    if (!error) snapshot.value = data;
  } finally {
    loading.value = false;
  }
}

function progressPercent(b: any): number {
  if (!b || !b.nextLevelProgress) return 0;
  return Math.min(100, Math.round((Number(b.progress) / Number(b.nextLevelProgress)) * 100));
}

defineExpose({ refresh: load });

void load();
</script>

<template>
  <div class="flex flex-col gap-12px">
    <NCard size="small" :title="$t('page.farm.personal.illustrated.title')">
      <template #header-extra>
        <NTag v-if="current" size="small" type="info">
          {{ $t('page.farm.personal.illustrated.level') }} {{ current.level }}
        </NTag>
      </template>
      <NEmpty v-if="!snapshot" :description="$t('page.farm.personal.illustrated.empty')" />
      <template v-else>
        <NTabs v-model:value="book" type="segment" size="small" class="mb-12px">
          <NTabPane name="crop" :tab="$t('page.farm.personal.illustrated.cropTab')" />
          <NTabPane name="mutant" :tab="$t('page.farm.personal.illustrated.mutantTab')" />
        </NTabs>

        <div v-if="current" class="mb-12px flex items-center gap-12px">
          <NProgress
            type="line"
            :percentage="progressPercent(current)"
            :height="10"
            class="flex-1"
          />
          <span class="text-12px text-gray-500">
            {{ current.progress }} / {{ current.nextLevelProgress }}
          </span>
        </div>

        <div v-if="current?.currentBuffs?.length" class="mb-12px flex flex-wrap gap-8px">
          <NTag v-for="buff in current.currentBuffs" :key="buff.id" size="small" type="warning" :bordered="false">
            {{ buff.name }} +{{ buff.value }}{{ buff.valueType === 'probability' ? '%' : '' }}
          </NTag>
        </div>

        <NGrid :cols="4" :x-gap="8" :y-gap="8" responsive="screen" item-responsive>
          <NGridItem v-for="item in current?.items ?? []" :key="item.seedId" span="s:6 m:4 l:3">
            <div
              class="flex flex-col items-center gap-4px rounded-6px border border-gray-200 p-8px dark:border-gray-700"
              :class="{ 'opacity-50': !item.unlocked }"
            >
              <NTag size="tiny" :type="item.unlocked ? 'success' : 'default'" :bordered="false">
                {{ item.unlocked ? $t('page.farm.personal.illustrated.unlocked') : `${item.progress ?? 0}` }}
              </NTag>
              <span class="max-w-full truncate text-12px">{{ item.name }}</span>
              <span v-if="item.reward" class="text-11px text-gray-400">
                {{ item.reward.name }} x{{ item.reward.count }}
              </span>
            </div>
          </NGridItem>
        </NGrid>
      </template>
    </NCard>
  </div>
</template>
