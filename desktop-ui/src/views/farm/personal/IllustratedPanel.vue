<script setup lang="ts">
import { computed, ref } from 'vue';
import { NCard, NEmpty, NProgress, NTag, NTabs, NTabPane } from 'naive-ui';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { fetchGetIllustratedSnapshot } from '@/service/api';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
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

/** 收藏奖励档位（proto 注释：2=40 点、3=80 点、4=200 点） */
function tier(item: any): number {
  return Number(item.rewardCategory ?? 0);
}

/** 等级用星级展示：档位 2/3/4 → 1/2/3 星 */
function stars(item: any): string {
  const t = tier(item);
  if (t <= 1) return '';
  return '★'.repeat(Math.min(t - 1, 3));
}

function imageSrc(item: any): string {
  return resolveCatalogImage(item.image);
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

        <div v-if="current" class="mb-8px flex items-center gap-12px">
          <NProgress type="line" :percentage="progressPercent(current)" :height="8" class="flex-1" />
          <span class="text-12px text-gray-500">{{ current.progress }} / {{ current.nextLevelProgress }}</span>
        </div>

        <div v-if="current?.currentBuffs?.length" class="mb-8px flex flex-wrap gap-6px">
          <NTag v-for="buff in current.currentBuffs" :key="buff.id" size="small" type="warning" :bordered="false">
            {{ buff.name }} +{{ buff.value }}{{ buff.valueType === 'probability' ? '%' : '' }}
          </NTag>
        </div>

        <div class="grid grid-cols-5 gap-6px sm:grid-cols-7 lg:grid-cols-9">
          <div
            v-for="item in current?.items ?? []"
            :key="item.seedId"
            class="cv-auto relative flex flex-col items-center gap-2px rounded-6px px-2px py-6px"
            :title="`${item.name} · ${item.unlocked ? $t('page.farm.personal.illustrated.unlocked') : $t('page.farm.personal.illustrated.locked')}${item.reward ? ` · ${item.reward.name} x${item.reward.count}` : ''}`"
          >
            <span
              class="absolute right-2px top-0 text-11px"
              :class="item.unlocked ? 'text-emerald-500' : 'text-gray-400'"
            >
              {{ item.unlocked ? '✓' : '🔒' }}
            </span>
            <img
              v-if="imageSrc(item)"
              :src="imageSrc(item)"
              :alt="item.name"
              class="h-40px w-40px object-contain"
              :class="{ 'grayscale-[0.9]': !item.unlocked }"
              loading="lazy"
              referrerpolicy="no-referrer"
            />
            <span v-else class="text-22px opacity-30">🌱</span>
            <span v-if="stars(item)" class="text-10px leading-none text-amber-500">
              {{ stars(item) }}
            </span>
            <span class="text-11px leading-tight" :class="item.unlocked ? 'font-semibold' : 'text-gray-400'">
              {{ item.progress ?? 0 }}点
            </span>
          </div>
        </div>
      </template>
    </NCard>
  </div>
</template>
