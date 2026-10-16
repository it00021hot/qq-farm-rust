<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton, NCard, NEmpty, NInputNumber, NModal, NProgress, NSpace, NTag, NTabs, NTabPane, useMessage } from 'naive-ui';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { fetchGetIllustratedSnapshot, fetchPurchaseSeed } from '@/service/api';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import { $t } from '@/locales';

defineOptions({ name: 'IllustratedPanel' });

const farmAccountStore = useFarmAccountStore();
const message = useMessage();
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

// ============ 种子购买（bot 0f09702：图鉴直达商店购买） ============

const purchaseOpen = ref(false);
const purchaseItem = ref<any>(null);
const purchaseCount = ref(1);
const purchasing = ref(false);

/** 可直接购买：有商品 ID 且未锁定/未售罄/非仅本地目录 */
function canBuy(item: any): boolean {
  const seed = item?.shopSeed;
  return Boolean(seed?.goodsId && !seed.locked && !seed.soldOut && !seed.unknownMeta);
}

/** 不可购买时的可用性提示（对齐 bot seedAvailability 文案语义） */
function seedAvailability(item: any): string {
  const seed = item?.shopSeed;
  if (!seed) return $t('page.farm.personal.illustrated.noShopInfo');
  if (seed.unknownMeta) return $t('page.farm.personal.illustrated.fromOtherSource');
  if (seed.locked) {
    return seed.requiresUnlockCard
      ? $t('page.farm.personal.illustrated.needUnlockCard')
      : seed.requiredLevel
        ? $t('page.farm.personal.illustrated.needLevel', { level: seed.requiredLevel })
        : $t('page.farm.personal.illustrated.needOtherCondition');
  }
  if (seed.soldOut) return $t('page.farm.personal.illustrated.soldOut');
  return $t('page.farm.personal.illustrated.cannotBuy');
}

function purchaseLimit(item: any): number {
  const remaining = Number(item?.shopSeed?.remainingPurchaseCount);
  return Number.isFinite(remaining) && remaining > 0 ? remaining : 9999;
}

function openSeedPurchase(item: any) {
  purchaseItem.value = item;
  purchaseCount.value = 1;
  purchaseOpen.value = true;
}

async function purchaseSeed() {
  const item = purchaseItem.value;
  const seed = item?.shopSeed;
  const accountId = farmAccountStore.currentAccountId;
  if (!accountId || !seed?.goodsId || purchasing.value) return;
  const count = Math.max(1, Math.min(purchaseLimit(item), Math.floor(Number(purchaseCount.value) || 1)));
  purchasing.value = true;
  try {
    const { error } = await fetchPurchaseSeed(accountId, Number(seed.goodsId), count);
    if (error) throw new Error(error.message);
    message.success($t('page.farm.personal.illustrated.buySuccess', { name: item.name }));
    purchaseOpen.value = false;
    purchaseItem.value = null;
    await load();
  } catch (cause) {
    message.error((cause as Error)?.message || $t('page.farm.personal.illustrated.buyFailed'));
  } finally {
    purchasing.value = false;
  }
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
          <!-- 自闭合 NTabPane 在 naive-ui 2.45 的 label 会渲染成空，label 必须走 #tab 具名插槽 -->
          <NTabPane name="crop">
            <template #tab>{{ $t('page.farm.personal.illustrated.cropTab') }}</template>
          </NTabPane>
          <NTabPane name="mutant">
            <template #tab>{{ $t('page.farm.personal.illustrated.mutantTab') }}</template>
          </NTabPane>
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
            :title="`${item.name} · ${item.unlocked ? $t('page.farm.personal.illustrated.unlocked') : $t('page.farm.personal.illustrated.locked')}${item.reward ? ` · ${item.reward.name} x${item.reward.count}` : ''}${book === 'crop' && !canBuy(item) ? ` · ${seedAvailability(item)}` : ''}`"
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
            <NButton
              v-if="book === 'crop' && canBuy(item)"
              size="tiny"
              type="primary"
              :loading="purchasing"
              :disabled="purchasing"
              class="mt-2px"
              @click="openSeedPurchase(item)"
            >
              {{ $t('page.farm.personal.illustrated.buy') }}
            </NButton>
          </div>
        </div>

        <NModal v-model:show="purchaseOpen">
          <NCard
            size="small"
            class="w-340px"
            :title="`${$t('page.farm.personal.illustrated.buyTitle')} · ${purchaseItem?.name ?? ''}`"
            closable
            @close="purchaseOpen = false"
          >
            <div class="flex flex-col gap-12px">
              <span class="text-12px text-gray-500">
                {{
                  $t('page.farm.personal.illustrated.batch', {
                    count: purchaseItem?.shopSeed?.itemCount ?? 1,
                    price: purchaseItem?.shopSeed?.price ?? 0
                  })
                }}
              </span>
              <div class="flex items-center justify-between">
                <span>{{ $t('page.farm.personal.illustrated.batches') }}</span>
                <NInputNumber
                  v-model:value="purchaseCount"
                  size="small"
                  :min="1"
                  :max="purchaseLimit(purchaseItem)"
                  :disabled="purchasing"
                  class="w-140px"
                />
              </div>
              <div class="flex items-center justify-between border-t border-gray-200 pt-8px dark:border-gray-700">
                <span>{{ $t('page.farm.personal.illustrated.total') }}</span>
                <strong>
                  {{ ((purchaseItem?.shopSeed?.price ?? 0) * (Number(purchaseCount) || 0)).toLocaleString() }}
                </strong>
              </div>
              <NSpace justify="end">
                <NButton size="small" :disabled="purchasing" @click="purchaseOpen = false">
                  {{ $t('page.farm.personal.illustrated.cancel') }}
                </NButton>
                <NButton size="small" type="primary" :loading="purchasing" @click="purchaseSeed">
                  {{ $t('page.farm.personal.illustrated.confirmBuy') }}
                </NButton>
              </NSpace>
            </div>
          </NCard>
        </NModal>
      </template>
    </NCard>
  </div>
</template>
