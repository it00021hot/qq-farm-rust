<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { NButton, NCard, NCheckbox, NEmpty, NPopconfirm, NSpace, NSpin, NTag, useMessage } from 'naive-ui';
import { fetchGetFarmBag, fetchSellFarmBag, fetchUseFarmBag } from '@/service/api';
import {
  COUPON_ITEM_ID,
  COLLECTION_NORMAL_ID,
  COLLECTION_RARE_ID,
  GOLD_BEAN_ITEM_ID,
  NORMAL_FERTILIZER_ID,
  ORGANIC_FERTILIZER_ID
} from '@/constants/items';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import { $t } from '@/locales';

defineOptions({ name: 'FarmPersonalBagPanel' });

const props = defineProps<{
  connected: boolean;
}>();

type CategoryValue = 'all' | 'fruit' | 'seed' | 'tool' | 'other';

const HIDDEN_IDS = new Set([1, 1001, COUPON_ITEM_ID, 1101, NORMAL_FERTILIZER_ID, ORGANIC_FERTILIZER_ID, COLLECTION_NORMAL_ID, COLLECTION_RARE_ID]);

const farmAccountStore = useFarmAccountStore();
const message = useMessage();

const bagLoading = ref(false);
const sellingId = ref<number | null>(null);
const usingId = ref<number | null>(null);
const batchSelling = ref(false);
const batchMode = ref(false);
const selectedIds = ref<Set<number>>(new Set());
const selectedCategory = ref<CategoryValue>('fruit');
const bagItems = ref<Api.Farm.BagItem[]>([]);
const originalItems = ref<Api.Farm.BagOriginalItem[]>([]);

const categoryOptions = computed(() => [
  { label: $t('page.farm.personal.bagCatAll'), value: 'all' as const },
  { label: $t('page.farm.personal.bagCatFruit'), value: 'fruit' as const },
  { label: $t('page.farm.personal.bagCatSeed'), value: 'seed' as const },
  { label: $t('page.farm.personal.bagCatTool'), value: 'tool' as const },
  { label: $t('page.farm.personal.bagCatOther'), value: 'other' as const }
]);

function getItemCategory(item: Api.Farm.BagItem): CategoryValue {
  const itemType = Number(item.itemType || 0);
  if (itemType === 17 || itemType === 6) return 'fruit';
  if (itemType === 5) return 'seed';
  if (itemType === 11) return 'tool';
  return 'other';
}

const visibleItems = computed(() => bagItems.value.filter(item => !HIDDEN_IDS.has(Number(item.id))));

const categoryCounts = computed(() => {
  const counts: Record<CategoryValue, number> = {
    all: visibleItems.value.length,
    fruit: 0,
    seed: 0,
    tool: 0,
    other: 0
  };
  for (const item of visibleItems.value) {
    counts[getItemCategory(item)] += 1;
  }
  return counts;
});

const filteredItems = computed(() => {
  if (selectedCategory.value === 'all') return visibleItems.value;
  return visibleItems.value.filter(item => getItemCategory(item) === selectedCategory.value);
});

function categoryLabel(cat: CategoryValue) {
  switch (cat) {
    case 'fruit':
      return $t('page.farm.personal.bagCatFruit');
    case 'seed':
      return $t('page.farm.personal.bagCatSeed');
    case 'tool':
      return $t('page.farm.personal.bagCatTool');
    case 'other':
      return $t('page.farm.personal.bagCatOther');
    default:
      return $t('page.farm.personal.bagCatAll');
  }
}

function canSell(item: Api.Farm.BagItem) {
  const t = Number(item.itemType || 0);
  return (t === 17 || t === 6) && item.sellable === true;
}

function canUse(item: Api.Farm.BagItem) {
  return Number(item.itemType || 0) === 11;
}

function bagImageSrc(item: Api.Farm.BagItem) {
  return resolveCatalogImage(item.image);
}

function priceClass(item: Api.Farm.BagItem) {
  const priceId = Number(item.priceId || 0);
  if (priceId === GOLD_BEAN_ITEM_ID) return 'text-amber-500';
  if (priceId === COUPON_ITEM_ID) return 'text-sky-500';
  return 'opacity-60';
}

async function loadBag() {
  if (!farmAccountStore.currentAccountId || !props.connected) {
    bagItems.value = [];
    originalItems.value = [];
    return;
  }
  bagLoading.value = true;
  try {
    const { data, error } = await fetchGetFarmBag(farmAccountStore.currentAccountId);
    if (!error && data) {
      bagItems.value = data.items || [];
      originalItems.value = data.originalItems || [];
      selectedIds.value = new Set();
    }
  } finally {
    bagLoading.value = false;
  }
}

function toggleSelect(id: number) {
  const next = new Set(selectedIds.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  selectedIds.value = next;
}

const selectedSellableCount = computed(
  () => filteredItems.value.filter(item => selectedIds.value.has(Number(item.id)) && canSell(item)).length
);

async function handleSell(item: Api.Farm.BagItem) {
  if (!farmAccountStore.currentAccountId) return;
  const sellItems = originalItems.value.filter(it => Number(it.id) === Number(item.id));
  if (sellItems.length === 0) {
    message.warning($t('page.farm.personal.sellNotFound'));
    return;
  }
  sellingId.value = item.id;
  try {
    const { error } = await fetchSellFarmBag({
      accountId: farmAccountStore.currentAccountId,
      items: sellItems
    });
    if (error) {
      message.error(error.message || $t('page.farm.personal.sellFailed'));
      return;
    }
    message.success($t('page.farm.personal.sellSuccess'));
    await loadBag();
  } finally {
    sellingId.value = null;
  }
}

async function handleUse(item: Api.Farm.BagItem) {
  if (!farmAccountStore.currentAccountId) return;
  usingId.value = item.id;
  try {
    const { error } = await fetchUseFarmBag({
      accountId: farmAccountStore.currentAccountId,
      itemId: item.id,
      count: Math.max(1, Number(item.count || 1))
    });
    if (error) {
      message.error(error.message || $t('page.farm.personal.useFailed'));
      return;
    }
    message.success($t('page.farm.personal.useSuccess'));
    await loadBag();
  } finally {
    usingId.value = null;
  }
}

async function handleBatchSell() {
  if (!farmAccountStore.currentAccountId) return;
  const selected = new Set(
    filteredItems.value
      .filter(item => selectedIds.value.has(Number(item.id)) && canSell(item))
      .map(item => Number(item.id))
  );
  const sellItems = originalItems.value.filter(it => selected.has(Number(it.id)));
  if (sellItems.length === 0) {
    message.warning($t('page.farm.personal.batchSellEmpty'));
    return;
  }
  batchSelling.value = true;
  try {
    const { error } = await fetchSellFarmBag({
      accountId: farmAccountStore.currentAccountId,
      items: sellItems
    });
    if (error) {
      message.error(error.message || $t('page.farm.personal.sellFailed'));
      return;
    }
    message.success($t('page.farm.personal.batchSellSuccess'));
    selectedIds.value = new Set();
    batchMode.value = false;
    await loadBag();
  } finally {
    batchSelling.value = false;
  }
}

function toggleBatchMode() {
  batchMode.value = !batchMode.value;
  if (!batchMode.value) selectedIds.value = new Set();
}

function selectAllSellable() {
  const next = new Set<number>();
  for (const item of filteredItems.value) {
    if (canSell(item)) next.add(Number(item.id));
  }
  selectedIds.value = next;
}

watch(
  () => [farmAccountStore.currentAccountId, props.connected] as const,
  () => {
    void loadBag();
  },
  { immediate: true }
);

let refreshTimer: ReturnType<typeof setInterval> | null = null;
onMounted(() => {
  refreshTimer = setInterval(() => {
    void loadBag();
  }, 60000);
});
onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer);
});

defineExpose({ refresh: loadBag });
</script>

<template>
  <NCard :title="$t('page.farm.personal.bagTitle')" :bordered="false" size="small" class="card-wrapper">
    <template #header-extra>
      <NSpace size="small">
        <NButton
          v-if="selectedCategory === 'fruit' || selectedCategory === 'all'"
          size="small"
          quaternary
          @click="toggleBatchMode"
        >
          {{ batchMode ? $t('common.cancel') : $t('page.farm.personal.batchSell') }}
        </NButton>
        <NButton v-if="batchMode" size="small" quaternary @click="selectAllSellable">
          {{ $t('common.selectAll') }}
        </NButton>
        <NPopconfirm v-if="batchMode" :disabled="selectedSellableCount === 0" @positive-click="handleBatchSell">
          <template #trigger>
            <NButton size="small" type="error" ghost :loading="batchSelling" :disabled="selectedSellableCount === 0">
              {{ $t('page.farm.personal.batchSell') }}
              <span v-if="selectedSellableCount > 0" class="ml-4px">({{ selectedSellableCount }})</span>
            </NButton>
          </template>
          {{ $t('page.farm.personal.batchSellConfirm', { count: selectedSellableCount }) }}
        </NPopconfirm>
      </NSpace>
    </template>

    <div class="mb-12px flex flex-wrap gap-8px">
      <NButton
        v-for="opt in categoryOptions"
        :key="opt.value"
        size="small"
        :type="selectedCategory === opt.value ? 'primary' : 'default'"
        :secondary="selectedCategory !== opt.value"
        @click="selectedCategory = opt.value"
      >
        {{ opt.label }}
        <span class="ml-4px opacity-70">({{ categoryCounts[opt.value] }})</span>
      </NButton>
    </div>

    <NSpin :show="bagLoading">
      <div v-if="filteredItems.length === 0" class="py-24px">
        <NEmpty :description="$t('common.noData')" />
      </div>
      <div v-else class="farm-bag-grid">
        <div
          v-for="item in filteredItems"
          :key="item.id"
          class="farm-bag-card"
          :class="{ 'is-selected': batchMode && selectedIds.has(Number(item.id)) }"
          @click="batchMode && canSell(item) ? toggleSelect(Number(item.id)) : undefined"
        >
          <div class="flex-y-center justify-between gap-4px">
            <span class="text-12px opacity-45">#{{ item.id }}</span>
            <NCheckbox
              v-if="batchMode && canSell(item)"
              :checked="selectedIds.has(Number(item.id))"
              @click.stop
              @update:checked="toggleSelect(Number(item.id))"
            />
            <NTag v-else size="tiny" :bordered="false">{{ categoryLabel(getItemCategory(item)) }}</NTag>
          </div>
          <div class="flex-center min-h-56px py-6px">
            <img
              v-if="bagImageSrc(item)"
              :src="bagImageSrc(item)"
              :alt="item.name"
              class="max-h-48px max-w-48px object-contain"
              loading="lazy"
              referrerpolicy="no-referrer"
            />
            <span v-else class="text-22px opacity-35">📦</span>
          </div>
          <div class="truncate text-center text-13px font-medium" :title="item.name">
            {{ item.name || `#${item.id}` }}
          </div>
          <div class="mt-4px flex-center flex-wrap gap-6px text-12px">
            <span v-if="item.level" class="opacity-55">Lv.{{ item.level }}</span>
            <span v-if="item.price" :class="priceClass(item)">{{ item.price }}{{ item.priceUnit || '' }}</span>
            <span
              v-if="getItemCategory(item) === 'fruit'"
              :class="canSell(item) ? 'text-green-500' : 'text-gray-400'"
              :title="item.sellCondition || ''"
            >
              {{ canSell(item) ? '可出售' : item.sellStatus === 'conditional' ? '条件出售' : '不可出售' }}
            </span>
          </div>
          <div class="mt-4px text-center text-14px font-medium">
            {{ item.hoursText || `x${item.count}` }}
          </div>
          <div v-if="!batchMode" class="mt-8px flex-center gap-6px">
            <NPopconfirm v-if="canSell(item)" @positive-click="handleSell(item)">
              <template #trigger>
                <NButton size="tiny" type="error" ghost :loading="sellingId === item.id">
                  {{ $t('page.farm.personal.sell') }}
                </NButton>
              </template>
              {{ $t('page.farm.personal.sellConfirm') }}
            </NPopconfirm>
            <NPopconfirm v-if="canUse(item)" @positive-click="handleUse(item)">
              <template #trigger>
                <NButton size="tiny" type="primary" ghost :loading="usingId === item.id">
                  {{ $t('page.farm.personal.use') }}
                </NButton>
              </template>
              {{ $t('page.farm.personal.useConfirm') }}
            </NPopconfirm>
          </div>
        </div>
      </div>
    </NSpin>
  </NCard>
</template>

<style scoped>
.farm-bag-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 12px;
}

.farm-bag-card {
  border: 1px solid rgba(0, 0, 0, 0.08);
  border-radius: 12px;
  padding: 10px;
  background: var(--n-color);
  transition:
    border-color 0.15s ease,
    box-shadow 0.15s ease;
}

.farm-bag-card:hover {
  border-color: rgba(0, 0, 0, 0.16);
}

.farm-bag-card.is-selected {
  border-color: var(--n-primary-color);
  box-shadow: 0 0 0 1px var(--n-primary-color);
}
</style>
