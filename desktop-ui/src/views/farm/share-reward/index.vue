<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NButton, NCard, NSpace, NSpin, NTag, useMessage } from 'naive-ui';
import { fetchShareRewardOperate, fetchShareRewardSnapshot } from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import { $t } from '@/locales';

defineOptions({ name: 'FarmShareReward' });

/** 当前有效期实例（bot share-reward-instances.json 的 2026110104） */
const ACTIVITY_ID = '2026110104';

const farmAccountStore = useFarmAccountStore();
const message = useMessage();

const data = ref<any>(null);
const busy = ref(false);
const error = ref('');
const logTab = ref<1 | 0>(1);
const logs = ref<any[]>([]);
let sequence = 0;

const connected = computed(() => farmAccountStore.currentAccount?.runStatus === 1);
const icon = computed(() => resolveCatalogImage(data.value?.icon));

async function request(action = '', input: Record<string, unknown> = {}) {
  const accountId = farmAccountStore.currentAccountId;
  if (!accountId || !connected.value || busy.value) return;
  const revision = ++sequence;
  busy.value = true;
  error.value = '';
  try {
    const response = action
      ? await fetchShareRewardOperate(accountId, ACTIVITY_ID, action, input)
      : await fetchShareRewardSnapshot(accountId, ACTIVITY_ID);
    if (revision !== sequence) return;
    if (response.error || !response.data) {
      throw new Error(response.error?.message || $t('page.farm.shareReward.requestFailed'));
    }
    const result: any = response.data;
    if (action) {
      const text =
        (result.rewards ?? []).map((reward: any) => `${reward.name} ×${reward.count}`).join('、') ||
        $t('page.farm.shareReward.actionSuccess');
      if (result.refreshRequired) {
        message.warning(`${text}。${$t('page.farm.shareReward.refreshRequired')}`);
      } else {
        message.success(text);
      }
      data.value = result.activity ?? null;
      if (!data.value) await request();
      if (action === 'logs' && result.result) logs.value = result.result.logs ?? [];
    } else {
      data.value = result;
    }
  } catch (cause) {
    if (revision === sequence) {
      error.value = (cause as Error)?.message || $t('page.farm.shareReward.requestFailed');
      if (action) message.error(error.value);
    }
  } finally {
    if (revision === sequence) busy.value = false;
  }
}

async function loadLogs(tab: 1 | 0) {
  logTab.value = tab;
  await request('logs', { tab });
}

watch(
  () => [farmAccountStore.currentAccountId, connected.value] as const,
  () => {
    sequence++;
    data.value = null;
    logs.value = [];
    error.value = '';
    void request();
  },
  { immediate: true, flush: 'sync' }
);

defineExpose({ refresh: () => request() });
</script>

<template>
  <div class="mx-auto max-w-900px">
    <NCard :title="$t('page.farm.shareReward.title')" size="small" :bordered="false" class="card-wrapper">
      <template #header-extra>
        <NSpace size="small">
          <NTag v-if="data" size="small" :type="data?.active ? 'success' : 'error'">
            {{ data?.active ? $t('page.farm.shareReward.active') : $t('page.farm.shareReward.inactive') }}
          </NTag>
          <NButton size="small" :loading="busy" :disabled="!farmAccountStore.currentAccountId || !connected" @click="request()">
            {{ $t('page.farm.shareReward.refresh') }}
          </NButton>
        </NSpace>
      </template>

      <NSpin :show="busy">
        <div v-if="!farmAccountStore.currentAccountId || !connected" class="py-32px text-center text-gray-500">
          <div class="text-28px">🎁</div>
          <strong>
            {{
              farmAccountStore.currentAccountId
                ? $t('page.farm.shareReward.startAccount')
                : $t('page.farm.shareReward.selectAccount')
            }}
          </strong>
        </div>
        <template v-else-if="data">
          <div class="mb-10px flex flex-wrap items-center gap-8px rounded-8px bg-gray-50 p-12px dark:bg-gray-800">
            <img v-if="icon" :src="icon" class="h-28px w-28px object-contain" loading="lazy" />
            <strong>{{ data.title }}</strong>
            <NTag size="small" type="warning">
              {{ $t('page.farm.shareReward.score') }} {{ data.score }}
            </NTag>
            <span class="text-12px text-gray-500">
              {{ $t('page.farm.shareReward.dailyReward') }} +{{ data.dailyReward }} ·
              {{ $t('page.farm.shareReward.firstShareReward') }} +{{ data.firstShareReward }}
            </span>
            <span class="ml-auto text-12px text-gray-500">
              {{ data.claimedCount }}/{{ data.claimLimit }} · {{ data.poolClaimedCount }}/{{ data.poolClaimLimit }}
            </span>
          </div>

          <div class="mb-10px flex flex-wrap gap-8px">
            <NButton size="small" type="primary" :disabled="busy || !data.canShare" @click="request('share', {})">
              {{ $t('page.farm.shareReward.share') }}
            </NButton>
            <NButton size="small" :disabled="busy || !data.canClaimDaily" @click="request('daily', {})">
              {{ $t('page.farm.shareReward.claimDaily') }}
            </NButton>
            <NButton
              size="small"
              :disabled="busy || !data.canClaimMilestones"
              @click="request('milestones', {})"
            >
              {{ $t('page.farm.shareReward.claimMilestones') }}
            </NButton>
          </div>

          <div class="mb-10px flex flex-col gap-6px">
            <div
              v-for="milestone in data.milestones ?? []"
              :key="milestone.id"
              class="flex flex-wrap items-center gap-8px rounded-6px border border-gray-200 px-10px py-8px text-13px dark:border-gray-700"
              :class="{ 'border-emerald-400': milestone.state === 2 }"
            >
              <strong>{{ milestone.threshold }}</strong>
              <span class="flex flex-wrap gap-6px">
                <span v-for="reward in milestone.rewards" :key="reward.id" class="flex items-center gap-4px text-12px">
                  <img
                    v-if="resolveCatalogImage(reward.image)"
                    :src="resolveCatalogImage(reward.image)"
                    class="h-18px w-18px object-contain"
                    loading="lazy"
                  />
                  {{ reward.name }} ×{{ reward.count }}
                </span>
              </span>
              <NTag size="tiny" class="ml-auto" :type="milestone.state === 2 ? 'success' : 'default'">
                {{ milestone.state === 2 ? $t('page.farm.shareReward.claimable') : $t('page.farm.shareReward.locked') }}
              </NTag>
            </div>
          </div>

          <div class="rounded-8px border border-gray-200 p-10px dark:border-gray-700">
            <div class="mb-8px flex items-center gap-8px">
              <strong class="text-13px">{{ $t('page.farm.shareReward.logs') }}</strong>
              <NButton size="tiny" :type="logTab === 1 ? 'primary' : 'default'" @click="loadLogs(1)">
                {{ $t('page.farm.shareReward.logsHost') }}
              </NButton>
              <NButton size="tiny" :type="logTab === 0 ? 'primary' : 'default'" @click="loadLogs(0)">
                {{ $t('page.farm.shareReward.logsGuest') }}
              </NButton>
            </div>
            <div class="flex flex-col gap-4px text-12px">
              <div v-for="log in logs" :key="log.seq" class="flex items-center gap-8px">
                <span>{{ log.actor?.name || '—' }}</span>
                <span :class="Number(log.score) >= 0 ? 'text-emerald-500' : 'text-red-400'">
                  {{ Number(log.score) >= 0 ? '+' : '' }}{{ log.score }}
                </span>
                <span class="ml-auto text-gray-400">{{ log.createdAt }}</span>
              </div>
              <span v-if="!logs.length" class="text-gray-500">{{ $t('page.farm.shareReward.logsEmpty') }}</span>
            </div>
          </div>
        </template>
        <div v-else class="py-32px text-center text-gray-500">
          <strong>{{ error || $t('page.farm.shareReward.loading') }}</strong>
        </div>
      </NSpin>
    </NCard>
  </div>
</template>
