<script setup lang="ts">
import { NButton, NCard } from 'naive-ui';
import { useRouter } from 'vue-router';
import { $t } from '@/locales';

defineOptions({ name: 'FarmActivityCenter' });

const router = useRouter();

const activities = [
  {
    key: 'solar-calendar',
    route: '/farm/solar-calendar',
    title: $t('page.farm.activityCenter.solarTitle'),
    description: $t('page.farm.activityCenter.solarDesc')
  },
  {
    key: 'milk-tea',
    route: '/farm/milk-tea',
    title: $t('page.farm.activityCenter.milkTitle'),
    description: $t('page.farm.activityCenter.milkDesc')
  },
  {
    key: 'share-reward',
    route: '/farm/share-reward',
    title: $t('page.farm.activityCenter.happyTitle'),
    description: $t('page.farm.activityCenter.happyDesc')
  }
];
</script>

<template>
  <div class="h-full min-h-500px flex-col-stretch gap-16px overflow-auto">
    <NCard :bordered="false" class="card-wrapper" :title="$t('page.farm.activityCenter.title')">
      <p class="mb-16px text-13px text-gray-500">{{ $t('page.farm.activityCenter.subtitle') }}</p>
      <div class="grid gap-16px sm:grid-cols-2 xl:grid-cols-3">
        <div
          v-for="item in activities"
          :key="item.key"
          class="activity-card flex flex-col gap-12px rounded-8px border border-gray-200 p-16px dark:border-gray-700"
        >
          <span class="text-16px font-semibold">{{ item.title }}</span>
          <p class="min-h-36px text-13px text-gray-500">{{ item.description }}</p>
          <NButton size="small" type="primary" secondary @click="router.push(item.route)">
            {{ $t('page.farm.activityCenter.enter') }}
          </NButton>
        </div>
      </div>
    </NCard>
  </div>
</template>
