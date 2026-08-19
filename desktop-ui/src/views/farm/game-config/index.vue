<script setup lang="tsx">
import { computed, onMounted, reactive, ref, shallowRef } from 'vue';
import { NButton, NPopconfirm, NTag } from 'naive-ui';
import { useBoolean } from '@sa/hooks';
import {
  fetchDeleteFarmGameConfigFruit,
  fetchDeleteFarmGameConfigItem,
  fetchDeleteFarmGameConfigSeed,
  fetchGetFarmGameConfigFruits,
  fetchGetFarmGameConfigItems,
  fetchGetFarmGameConfigItemTypes,
  fetchGetFarmGameConfigSeeds
} from '@/service/api';
import type { FlatResponseData } from '@sa/axios';
import { useAppStore } from '@/store/modules/app';
import { defaultTransform, useNaivePaginatedTable } from '@/hooks/common/table';
import { $t } from '@/locales';
import {
  formatGrowTime,
  formatPrice,
  rarityLabelMap,
  resolveCatalogImage,
  type GameConfigTab
} from './shared';
import GameConfigSearch from './modules/game-config-search.vue';
import SeedOperateDrawer from './modules/seed-operate-drawer.vue';
import FruitOperateDrawer from './modules/fruit-operate-drawer.vue';
import ItemOperateDrawer from './modules/item-operate-drawer.vue';

defineOptions({ name: 'FarmGameConfig' });

type CatalogRow = Api.Farm.GameConfigSeed | Api.Farm.GameConfigFruit | Api.Farm.GameConfigItem;
type CatalogList = Api.Common.PaginatingQueryRecord<CatalogRow>;

const appStore = useAppStore();

const activeTab = ref<GameConfigTab>('seeds');
const forceReload = ref(false);
const imageErrors = ref<Record<string | number, boolean>>({});

const seedCache = ref<Api.Farm.GameConfigSeed[]>([]);
const fruitCache = ref<Api.Farm.GameConfigFruit[]>([]);
const itemCache = ref<Api.Farm.GameConfigItem[]>([]);
const cacheLoaded = reactive({ seeds: false, fruits: false, items: false });
const itemTypeOptions = ref<{ label: string; value: number }[]>([]);

const searchParams = ref<Api.Farm.GameConfigSearchParams>({
  current: 1,
  size: 10,
  keyword: null,
  seasons: null,
  rarity: null,
  itemType: null
});

const { bool: seedDrawerVisible, setTrue: openSeedDrawer } = useBoolean();
const { bool: fruitDrawerVisible, setTrue: openFruitDrawer } = useBoolean();
const { bool: itemDrawerVisible, setTrue: openItemDrawer } = useBoolean();

const operateType = ref<NaiveUI.TableOperateType>('add');
const editingSeed = ref<Api.Farm.GameConfigSeed | null>(null);
const editingFruit = ref<Api.Farm.GameConfigFruit | null>(null);
const editingItem = ref<Api.Farm.GameConfigItem | null>(null);
const checkedRowKeys = shallowRef<Array<string | number>>([]);

const itemTypeLabelMap = computed(() => {
  const map: Record<number, string> = {};
  itemTypeOptions.value.forEach(t => {
    map[t.value] = t.label;
  });
  return map;
});

function asSeed(row: CatalogRow) {
  return row as Api.Farm.GameConfigSeed;
}

function asFruit(row: CatalogRow) {
  return row as Api.Farm.GameConfigFruit;
}

function asItem(row: CatalogRow) {
  return row as Api.Farm.GameConfigItem;
}

function matchesKeyword(name: string | undefined, ...ids: Array<string | number | null | undefined>) {
  const kw = searchParams.value.keyword?.trim().toLowerCase();
  if (!kw) return true;
  if (name?.toLowerCase().includes(kw)) return true;
  return ids.some(id => id != null && String(id).includes(kw));
}

function filterSeeds(list: Api.Farm.GameConfigSeed[]) {
  return list.filter(row => {
    if (!matchesKeyword(row.name, row.seedId)) return false;
    if (searchParams.value.seasons != null && row.seasons !== searchParams.value.seasons) return false;
    return true;
  });
}

function filterFruits(list: Api.Farm.GameConfigFruit[]) {
  return list.filter(row => {
    if (!matchesKeyword(row.name, row.id, row.plantId)) return false;
    if (searchParams.value.rarity != null && row.rarity !== searchParams.value.rarity) return false;
    return true;
  });
}

function filterItems(list: Api.Farm.GameConfigItem[]) {
  return list.filter(row => {
    if (!matchesKeyword(row.name, row.id)) return false;
    if (searchParams.value.itemType != null && row.type !== searchParams.value.itemType) return false;
    if (searchParams.value.rarity != null && row.rarity !== searchParams.value.rarity) return false;
    return true;
  });
}

function toPaged(list: CatalogRow[]): FlatResponseData<any, CatalogList> {
  const current = searchParams.value.current || 1;
  const size = searchParams.value.size || 10;
  const start = (current - 1) * size;
  return {
    data: {
      records: list.slice(start, start + size),
      current,
      size,
      total: list.length
    },
    error: null,
    response: {} as any
  };
}

async function loadTab(tab: GameConfigTab) {
  if (tab === 'seeds') {
    const { data, error } = await fetchGetFarmGameConfigSeeds();
    if (!error) {
      seedCache.value = data || [];
      cacheLoaded.seeds = true;
    }
    return;
  }
  if (tab === 'fruits') {
    const { data, error } = await fetchGetFarmGameConfigFruits();
    if (!error) {
      fruitCache.value = data || [];
      cacheLoaded.fruits = true;
    }
    return;
  }
  const { data, error } = await fetchGetFarmGameConfigItems();
  if (!error) {
    itemCache.value = data || [];
    cacheLoaded.items = true;
  }
}

async function fetchCatalogPage() {
  const tab = activeTab.value;
  if (forceReload.value || !cacheLoaded[tab]) {
    await loadTab(tab);
    forceReload.value = false;
  }
  if (tab === 'seeds') return toPaged(filterSeeds(seedCache.value));
  if (tab === 'fruits') return toPaged(filterFruits(fruitCache.value));
  return toPaged(filterItems(itemCache.value));
}

function iconErrorKey(row: CatalogRow, tab: GameConfigTab) {
  if (tab === 'seeds') return `seed-${asSeed(row).seedId ?? 0}`;
  return `item-${'id' in row ? row.id : 0}`;
}

function renderIcon(src: string, fallback: string, errorKey: string) {
  const url = resolveCatalogImage(src);
  if (!url || imageErrors.value[errorKey]) {
    return <span class="text-gray-400">{fallback}</span>;
  }
  return (
    <img
      src={url}
      alt=""
      width={32}
      height={32}
      class="h-32px w-32px object-contain"
      loading="lazy"
      onError={() => {
        imageErrors.value[errorKey] = true;
      }}
    />
  );
}

function renderOperate(onEdit: () => void, onDelete: () => Promise<void> | void) {
  return (
    <div class="flex-center gap-8px">
      <NButton type="primary" ghost size="small" onClick={onEdit}>
        {$t('common.edit')}
      </NButton>
      <NPopconfirm onPositiveClick={onDelete}>
        {{
          default: () => $t('common.confirmDelete'),
          trigger: () => (
            <NButton type="error" ghost size="small">
              {$t('common.delete')}
            </NButton>
          )
        }}
      </NPopconfirm>
    </div>
  );
}

const { columns, columnChecks, data, getDataByPage, loading, mobilePagination, reloadColumns } =
  useNaivePaginatedTable<FlatResponseData<any, CatalogList>, CatalogRow>({
    api: () => fetchCatalogPage(),
    transform: response => defaultTransform(response),
    immediate: false,
    paginationProps: {
      pageSizes: [10, 20, 50, 100]
    },
    onPaginationParamsChange: params => {
      searchParams.value.current = params.page;
      searchParams.value.size = params.pageSize;
    },
    columns: () => {
      const center = 'center' as const;
      const selection = {
        type: 'selection' as const,
        align: center,
        width: 48
      };

      if (activeTab.value === 'seeds') {
        return [
          selection,
          {
            key: 'name',
            title: $t('page.farm.gameConfig.seed'),
            align: center,
            render: (row: CatalogRow) => {
              const seed = asSeed(row);
              return (
                <div class="flex-center gap-8px">
                  {renderIcon(seed.image, '🌱', iconErrorKey(seed, 'seeds'))}
                  <span class="font-medium">{seed.name}</span>
                </div>
              );
            }
          },
          { key: 'seedId', title: $t('page.farm.gameConfig.seedId'), align: center },
          {
            key: 'requiredLevel',
            title: $t('page.farm.gameConfig.requiredLevel'),
            align: center,
            render: (row: CatalogRow) => `Lv.${asSeed(row).requiredLevel}`
          },
          {
            key: 'seasons',
            title: $t('page.farm.gameConfig.seasons'),
            align: center,
            render: (row: CatalogRow) => (
              <NTag size="small" type={asSeed(row).seasons === 2 ? 'info' : 'success'} bordered={false}>
                {asSeed(row).seasons === 2 ? '双季' : '单季'}
              </NTag>
            )
          },
          {
            key: 'growTime',
            title: $t('page.farm.gameConfig.growTime'),
            align: center,
            render: (row: CatalogRow) => formatGrowTime(asSeed(row).growTime)
          },
          {
            key: 'harvestCount',
            title: $t('page.farm.gameConfig.harvestCount'),
            align: center
          },
          { key: 'exp', title: $t('page.farm.gameConfig.exp'), align: center },
          {
            key: 'price',
            title: $t('page.farm.gameConfig.price'),
            align: center,
            render: (row: CatalogRow) => formatPrice(asSeed(row).price, asSeed(row).priceId)
          },
          {
            key: 'operate',
            title: $t('common.operate'),
            align: center,
            render: (row: CatalogRow) => {
              const seed = asSeed(row);
              return renderOperate(
                () => handleEditSeed(seed),
                () => handleDeleteSeed(seed)
              );
            }
          }
        ];
      }

      if (activeTab.value === 'fruits') {
        return [
          selection,
          {
            key: 'name',
            title: $t('page.farm.gameConfig.fruit'),
            align: center,
            render: (row: CatalogRow) => {
              const fruit = asFruit(row);
              return (
                <div class="flex-center gap-8px">
                  {renderIcon(fruit.image, '🍎', iconErrorKey(fruit, 'fruits'))}
                  <span class="font-medium">{fruit.name}</span>
                </div>
              );
            }
          },
          { key: 'id', title: $t('page.farm.gameConfig.fruitId'), align: center },
          {
            key: 'plantName',
            title: $t('page.farm.gameConfig.plant'),
            align: center,
            render: (row: CatalogRow) => asFruit(row).plantName || '-'
          },
          {
            key: 'rarity',
            title: $t('page.farm.gameConfig.rarity'),
            align: center,
            render: (row: CatalogRow) => rarityLabelMap[asFruit(row).rarity] || asFruit(row).rarity
          },
          {
            key: 'price',
            title: $t('page.farm.gameConfig.price'),
            align: center,
            render: (row: CatalogRow) => formatPrice(asFruit(row).price, asFruit(row).priceId)
          },
          {
            key: 'operate',
            title: $t('common.operate'),
            align: center,
            render: (row: CatalogRow) => {
              const fruit = asFruit(row);
              return renderOperate(
                () => handleEditFruit(fruit),
                () => handleDeleteFruit(fruit)
              );
            }
          }
        ];
      }

      return [
        selection,
        {
          key: 'name',
          title: $t('page.farm.gameConfig.item'),
          align: center,
          render: (row: CatalogRow) => {
            const item = asItem(row);
            return (
              <div class="flex-center gap-8px">
                {renderIcon(item.image, '🎒', iconErrorKey(item, 'items'))}
                <span class="font-medium">{item.name}</span>
              </div>
            );
          }
        },
        { key: 'id', title: $t('page.farm.gameConfig.itemId'), align: center },
        {
          key: 'type',
          title: $t('page.farm.gameConfig.itemType'),
          align: center,
          render: (row: CatalogRow) => itemTypeLabelMap.value[asItem(row).type] || asItem(row).type
        },
        {
          key: 'rarity',
          title: $t('page.farm.gameConfig.rarity'),
          align: center,
          render: (row: CatalogRow) => rarityLabelMap[asItem(row).rarity] || asItem(row).rarity
        },
        {
          key: 'price',
          title: $t('page.farm.gameConfig.price'),
          align: center,
          render: (row: CatalogRow) => formatPrice(asItem(row).price, asItem(row).priceId)
        },
        {
          key: 'operate',
          title: $t('common.operate'),
          align: center,
          render: (row: CatalogRow) => {
            const item = asItem(row);
            return renderOperate(
              () => handleEditItem(item),
              () => handleDeleteItem(item)
            );
          }
        }
      ];
    }
  });

function rowKey(row: CatalogRow) {
  if (activeTab.value === 'seeds') return asSeed(row).seedId;
  return asItem(row).id;
}

async function loadItemTypes() {
  const { data, error } = await fetchGetFarmGameConfigItemTypes();
  if (!error && data) {
    itemTypeOptions.value = data.map((t: { label: string; value: number }) => ({
      label: t.label,
      value: t.value
    }));
  }
}

function resetFilters() {
  searchParams.value.keyword = null;
  searchParams.value.seasons = null;
  searchParams.value.rarity = null;
  searchParams.value.itemType = null;
}

async function refreshList(page: number = 1, force = true) {
  if (force) forceReload.value = true;
  await getDataByPage(page);
}

function handleTabChange(tab: string) {
  activeTab.value = tab as GameConfigTab;
  resetFilters();
  checkedRowKeys.value = [];
  imageErrors.value = {};
  reloadColumns();
  void refreshList(1, false);
}

function handleAdd() {
  operateType.value = 'add';
  if (activeTab.value === 'seeds') {
    editingSeed.value = null;
    openSeedDrawer();
  } else if (activeTab.value === 'fruits') {
    editingFruit.value = null;
    openFruitDrawer();
  } else {
    editingItem.value = null;
    openItemDrawer();
  }
}

function handleEditSeed(row: Api.Farm.GameConfigSeed) {
  operateType.value = 'edit';
  editingSeed.value = row;
  openSeedDrawer();
}

function handleEditFruit(row: Api.Farm.GameConfigFruit) {
  operateType.value = 'edit';
  editingFruit.value = row;
  openFruitDrawer();
}

function handleEditItem(row: Api.Farm.GameConfigItem) {
  operateType.value = 'edit';
  editingItem.value = row;
  openItemDrawer();
}

async function handleDeleteSeed(row: Api.Farm.GameConfigSeed) {
  const { error } = await fetchDeleteFarmGameConfigSeed(row.seedId);
  if (!error) {
    window.$message?.success($t('common.deleteSuccess'));
    await refreshList(searchParams.value.current || 1);
  }
}

async function handleDeleteFruit(row: Api.Farm.GameConfigFruit) {
  const { error } = await fetchDeleteFarmGameConfigFruit(row.id);
  if (!error) {
    window.$message?.success($t('common.deleteSuccess'));
    await refreshList(searchParams.value.current || 1);
  }
}

async function handleDeleteItem(row: Api.Farm.GameConfigItem) {
  const { error } = await fetchDeleteFarmGameConfigItem(row.id);
  if (!error) {
    window.$message?.success($t('common.deleteSuccess'));
    await refreshList(searchParams.value.current || 1);
  }
}

async function handleBatchDelete() {
  if (!checkedRowKeys.value.length) return;
  const ids = checkedRowKeys.value.map(Number);
  let deletedCount = 0;
  if (activeTab.value === 'seeds') {
    for (const id of ids) {
      const { error } = await fetchDeleteFarmGameConfigSeed(id);
      if (!error) deletedCount += 1;
    }
  } else if (activeTab.value === 'fruits') {
    for (const id of ids) {
      const { error } = await fetchDeleteFarmGameConfigFruit(id);
      if (!error) deletedCount += 1;
    }
  } else {
    for (const id of ids) {
      const { error } = await fetchDeleteFarmGameConfigItem(id);
      if (!error) deletedCount += 1;
    }
  }
  checkedRowKeys.value = [];
  if (deletedCount) {
    window.$message?.success($t('common.deleteSuccess'));
    await refreshList(searchParams.value.current || 1);
  }
}

onMounted(async () => {
  await loadItemTypes();
  await refreshList(1, false);
});
</script>

<template>
  <div class="min-h-500px flex-col-stretch gap-16px overflow-hidden lt-sm:overflow-auto">
    <GameConfigSearch
      v-model:model="searchParams"
      :tab="activeTab"
      :item-type-options="itemTypeOptions"
      @search="() => refreshList(1, false)"
    />
    <NCard :bordered="false" size="small" class="card-wrapper sm:flex-1-hidden">
      <template #header>
        <NTabs :value="activeTab" type="line" pane-wrapper-style="display: none" @update:value="handleTabChange">
          <NTabPane name="seeds" :tab="$t('page.farm.gameConfig.tabSeeds')" />
          <NTabPane name="fruits" :tab="$t('page.farm.gameConfig.tabFruits')" />
          <NTabPane name="items" :tab="$t('page.farm.gameConfig.tabItems')" />
        </NTabs>
      </template>
      <template #header-extra>
        <TableHeaderOperation
          v-model:columns="columnChecks"
          :disabled-delete="checkedRowKeys.length === 0"
          :loading="loading"
          @add="handleAdd"
          @delete="handleBatchDelete"
          @refresh="() => refreshList()"
        />
      </template>
      <NDataTable
        v-model:checked-row-keys="checkedRowKeys"
        :columns="columns"
        :data="data"
        size="small"
        :flex-height="!appStore.isMobile"
        :scroll-x="appStore.isMobile ? 1100 : undefined"
        :loading="loading"
        remote
        :row-key="rowKey"
        :pagination="mobilePagination"
        class="sm:h-full"
      />
    </NCard>
    <SeedOperateDrawer
      v-model:visible="seedDrawerVisible"
      :operate-type="operateType"
      :row-data="editingSeed"
      @submitted="() => refreshList()"
    />
    <FruitOperateDrawer
      v-model:visible="fruitDrawerVisible"
      :operate-type="operateType"
      :row-data="editingFruit"
      @submitted="() => refreshList()"
    />
    <ItemOperateDrawer
      v-model:visible="itemDrawerVisible"
      :operate-type="operateType"
      :row-data="editingItem"
      @submitted="() => refreshList()"
    />
  </div>
</template>

<style scoped>
:deep(.n-data-table-table) {
  table-layout: fixed;
  width: 100%;
}
</style>
