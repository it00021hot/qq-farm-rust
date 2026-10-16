<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NCard, NModal, NTag } from 'naive-ui';
import { fetchGetFarmBag, fetchGetFarmStatusDetail } from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import { $t } from '@/locales';

/**
 * 土地加成与升级/解锁条件详情弹窗（bot e5f3552 LandDetailModal.vue）。
 * 数据来自 land.landInfo（服务端 buildLandUpgradeDetail）；余额对比在打开时拉取状态+背包。
 * 注意：成本图片是 /game-config 相对路径，必须经 resolveCatalogImage 转协议地址；
 * 土壤图是 /land-assets dist 静态文件，按原样使用。
 */
const props = defineProps<{ land: Api.Farm.LandRow | null; connected: boolean }>();
const emit = defineEmits<{ close: [] }>();

const farmAccountStore = useFarmAccountStore();

const info = computed(() => props.land?.landInfo ?? null);

const balances = ref<{ gold: number | null; coupon: number | null; goldBean: number | null; level: number | null }>(
  { gold: null, coupon: null, goldBean: null, level: null }
);

watch(
  () => [props.land?.id, props.connected] as const,
  async ([landId, connected]) => {
    if (!landId || !connected || !farmAccountStore.currentAccountId) return;
    const accountId = farmAccountStore.currentAccountId;
    const [statusRes, bagRes] = await Promise.all([
      fetchGetFarmStatusDetail(accountId),
      fetchGetFarmBag(accountId)
    ]);
    if (!statusRes.error && statusRes.data) {
      balances.value.level = Number(statusRes.data.level ?? 0) || null;
      balances.value.gold = Number(statusRes.data.gold ?? 0) || null;
    }
    if (!bagRes.error && bagRes.data) {
      const item = (id: number) =>
        Number((bagRes.data?.items || []).find((entry: Api.Farm.BagItem) => Number(entry.id) === id)?.count || 0);
      balances.value.coupon = item(1002);
      balances.value.goldBean = item(1005);
    }
  },
  { immediate: true }
);

function balanceOf(costId: string): number | null {
  switch (String(costId)) {
    case '1001':
      return balances.value.gold;
    case '1002':
      return balances.value.coupon;
    case '1005':
      return balances.value.goldBean;
    default:
      return null;
  }
}

interface CostRow {
  id: string;
  count: string;
  name: string;
  image: string;
  balance: bigint | null;
  missing: bigint | null;
}

const costRows = computed<CostRow[]>(() =>
  (info.value?.costs || []).map(cost => {
    const text = String(cost.count ?? '');
    const required = /^\d+$/.test(text) ? BigInt(text) : 0n;
    const raw = balanceOf(cost.id);
    const balance = typeof raw === 'number' && Number.isFinite(raw) ? BigInt(Math.max(0, Math.floor(raw))) : null;
    return {
      ...cost,
      balance,
      missing: balance === null ? null : required > balance ? required - balance : 0n
    };
  })
);

const levelMet = computed(
  () =>
    props.connected &&
    balances.value.level !== null &&
    Number(balances.value.level) >= Number(info.value?.needLevel ?? 0)
);

const metrics = [
  { key: 'yieldBonus', label: 'landDetail.yieldBonus', unit: '%', i18n: true },
  { key: 'growthReduction', label: 'landDetail.growthReduction', unit: '%', i18n: true },
  { key: 'expBonus', label: 'landDetail.expBonus', unit: '%', i18n: true },
  { key: 'mutantMultiplier', label: 'landDetail.mutantMultiplier', unit: 'x', i18n: true },
  { key: 'resonanceBonus', label: 'landDetail.resonanceBonus', unit: '%', i18n: true }
] as const;

function metric(level: any, key: (typeof metrics)[number]['key'], unit: string): string {
  if (!level) return $t('page.farm.personal.landDetail.none');
  const value = Number(level[key]) || 0;
  if (key === 'mutantMultiplier' && !value) {
    return $t('page.farm.personal.landDetail.noMutation');
  }
  return `${value}${unit}`;
}

function isImproved(key: string): boolean {
  const current = Number(info.value?.current?.[key as 'yieldBonus'] ?? 0);
  const next = Number(info.value?.next?.[key as 'yieldBonus'] ?? 0);
  return next > current;
}

function formatAmount(value: unknown): string {
  const text = String(value ?? '');
  return /^\d+$/.test(text) ? BigInt(text).toLocaleString('zh-CN') : '—';
}

function costIconSrc(cost: { image: string }): string {
  return resolveCatalogImage(cost.image);
}
</script>

<template>
  <NModal :show="!!land" @update:show="!$event && emit('close')">
    <NCard
      size="small"
      class="w-440px max-w-[calc(100vw-32px)]"
      :bordered="false"
      closable
      :title="$t('page.farm.personal.landDetail.title')"
      @close="emit('close')"
    >
      <template #header-extra>
        <span class="text-13px text-gray-400">#{{ land?.id ?? '' }}</span>
      </template>
      <template v-if="info">
        <!-- 当前 → 升级后 土壤对比 -->
        <div class="mb-16px flex items-center justify-center gap-20px">
          <div v-if="info.current" class="flex flex-col items-center gap-6px">
            <img
              v-if="info.current.image"
              :src="info.current.image"
              :alt="info.current.name"
              class="h-64px object-contain drop-shadow-sm"
            />
            <span class="text-13px font-semibold">{{ info.current.name }}</span>
            <span class="text-11px text-gray-400">{{ $t('page.farm.personal.landDetail.current') }}</span>
          </div>
          <span
            v-if="info.next && info.current"
            class="text-18px text-gray-400"
            aria-hidden="true"
          >
            →
          </span>
          <div v-if="info.next" class="flex flex-col items-center gap-6px">
            <img
              v-if="info.next.image"
              :src="info.next.image"
              :alt="info.next.name"
              class="h-64px object-contain drop-shadow-sm"
            />
            <span class="text-13px font-semibold">{{ info.next.name }}</span>
            <span class="text-11px text-gray-400">
              {{
                $t(
                  info.current
                    ? 'page.farm.personal.landDetail.afterUpgrade'
                    : 'page.farm.personal.landDetail.afterUnlock'
                )
              }}
            </span>
          </div>
        </div>

        <!-- 加成对比 -->
        <div class="overflow-hidden rounded-8px bg-gray-50 dark:bg-gray-800/50">
          <div
            class="grid grid-cols-[1fr_92px_92px] items-center border-b border-gray-100 px-12px py-6px text-11px text-gray-400 dark:border-gray-700/60"
          >
            <span>{{ $t('page.farm.personal.landDetail.bonusHeader') }}</span>
            <span class="text-right">
              {{
                $t(
                  info.current ? 'page.farm.personal.landDetail.current' : 'page.farm.personal.landDetail.afterUnlock'
                )
              }}
            </span>
            <span v-if="info.current && info.next" class="text-right">
              {{ $t('page.farm.personal.landDetail.afterUpgrade') }}
            </span>
          </div>
          <div
            v-for="row in metrics"
            :key="row.key"
            class="grid grid-cols-[1fr_92px_92px] items-center px-12px py-7px text-13px"
          >
            <span class="text-gray-500 dark:text-gray-400">{{ $t(`page.farm.personal.${row.label}`) }}</span>
            <span class="text-right tabular-nums">{{ metric(info.current || info.next, row.key, row.unit) }}</span>
            <span
              v-if="info.current && info.next"
              class="text-right tabular-nums"
              :class="isImproved(row.key) ? 'font-medium text-success' : ''"
            >
              {{ metric(info.next, row.key, row.unit) }}
              <span v-if="isImproved(row.key)">↑</span>
            </span>
          </div>
        </div>

        <NTag
          v-if="info.mode === 'max'"
          type="success"
          :bordered="false"
          class="mt-16px w-full justify-center"
        >
          {{ $t('page.farm.personal.landDetail.maxLevel') }}
        </NTag>

        <!-- 解锁 / 升级条件 -->
        <div v-else class="mt-16px flex flex-col gap-4px">
          <div class="mb-4px text-13px font-medium">
            {{
              $t(
                info.mode === 'unlock'
                  ? 'page.farm.personal.landDetail.unlockCondition'
                  : 'page.farm.personal.landDetail.upgradeCondition'
              )
            }}
          </div>
          <div
            v-if="info.needLevel !== null && info.needLevel !== undefined"
            class="flex items-center justify-between rounded-8px bg-gray-50 px-12px py-8px text-13px dark:bg-gray-800/50"
          >
            <span class="text-gray-500 dark:text-gray-400">
              {{ $t('page.farm.personal.landDetail.needLevel', { level: info.needLevel }) }}
            </span>
            <strong
              class="tabular-nums"
              :class="levelMet ? 'text-success' : 'text-error'"
            >
              {{ connected ? (balances.level ?? '—') : '—' }} / {{ info.needLevel }}
            </strong>
          </div>
          <div
            v-if="info.prerequisite"
            class="flex items-center justify-between rounded-8px bg-gray-50 px-12px py-8px text-13px dark:bg-gray-800/50"
          >
            <span class="text-gray-500 dark:text-gray-400">
              {{ $t('page.farm.personal.landDetail.needLand', { id: info.prerequisite.landId }) }}
            </span>
            <strong :class="info.prerequisite.met ? 'text-success' : 'text-error'">
              {{
                $t(
                  info.prerequisite.met
                    ? 'page.farm.personal.landDetail.met'
                    : 'page.farm.personal.landDetail.unmet'
                )
              }}
            </strong>
          </div>
          <div
            v-if="info.current && info.next"
            class="flex items-center justify-between rounded-8px bg-gray-50 px-12px py-8px text-13px dark:bg-gray-800/50"
          >
            <span class="text-gray-500 dark:text-gray-400">
              {{ $t('page.farm.personal.landDetail.currentSoil', { name: info.current.name }) }}
            </span>
            <strong class="text-success">{{ $t('page.farm.personal.landDetail.met') }}</strong>
          </div>

          <!-- 成本清单 -->
          <div
            v-for="cost in costRows"
            :key="cost.id"
            class="flex items-center gap-10px rounded-8px bg-gray-50 px-12px py-8px dark:bg-gray-800/50"
          >
            <div
              class="flex h-30px w-30px shrink-0 items-center justify-center rounded-6px border border-gray-200 bg-white p-3px dark:border-gray-600 dark:bg-gray-900"
            >
              <img
                v-if="cost.image"
                :src="costIconSrc(cost)"
                :alt="cost.name"
                class="h-full w-full object-contain"
                loading="lazy"
              />
            </div>
            <span class="min-w-0 flex-1 truncate text-13px">{{ cost.name }}</span>
            <strong class="tabular-nums">{{ formatAmount(cost.count) }}</strong>
            <span
              class="w-72px shrink-0 text-right text-12px"
              :class="
                cost.missing === null
                  ? 'text-gray-400'
                  : cost.missing === 0n
                    ? 'text-success'
                    : 'text-error'
              "
            >
              {{
                cost.missing === null
                  ? $t('page.farm.personal.landDetail.balancePending')
                  : cost.missing > 0n
                    ? $t('page.farm.personal.landDetail.missing', { amount: cost.missing.toLocaleString('zh-CN') })
                    : $t('page.farm.personal.landDetail.enough')
              }}
            </span>
          </div>

          <div v-if="info.unknown" class="mt-4px text-12px text-error">
            {{ $t('page.farm.personal.landDetail.unknownCondition') }}
          </div>
        </div>
      </template>
    </NCard>
  </NModal>
</template>
