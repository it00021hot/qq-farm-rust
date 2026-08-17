<script setup lang="tsx">
import { computed, onMounted, ref } from 'vue';
import type { SelectOption } from 'naive-ui';
import { NButton, NImage, NSpace, NTag } from 'naive-ui';
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
import { useAppStore } from '@/store/modules/app';
import { $t } from '@/locales';
import {
  formatGrowTime,
  formatPrice,
  rarityLabelMap,
  rarityOptions,
  resolveCatalogImage,
  seasonOptions
} from './shared';
import SeedOperateDrawer from './modules/seed-operate-drawer.vue';
import FruitOperateDrawer from './modules/fruit-operate-drawer.vue';
import ItemOperateDrawer from './modules/item-operate-drawer.vue';

defineOptions({ name: 'FarmGameConfig' });

type TabKey = 'seeds' | 'fruits' | 'items';

const appStore = useAppStore();

const activeTab = ref<TabKey>('seeds');
const loading = ref(false);
const searchKeyword = ref('');

const seedList = ref<Api.Farm.GameConfigSeed[]>([]);
const fruitList = ref<Api.Farm.GameConfigFruit[]>([]);
const itemList = ref<Api.Farm.GameConfigItem[]>([]);

const seedSeasonFilter = ref<number | null>(null);
const seedSort = ref('name');
const fruitRarityFilter = ref<number | null>(null);
const fruitSort = ref('name');
const itemTypeFilter = ref<number | null>(null);
const itemRarityFilter = ref<number | null>(null);
const itemSort = ref('name');
const itemTypeOptions = ref<{ label: string; value: number }[]>([]);

const { bool: seedDrawerVisible, setTrue: openSeedDrawer } = useBoolean();
const { bool: fruitDrawerVisible, setTrue: openFruitDrawer } = useBoolean();
const { bool: itemDrawerVisible, setTrue: openItemDrawer } = useBoolean();

const operateType = ref<NaiveUI.TableOperateType>('add');
const editingSeed = ref<Api.Farm.GameConfigSeed | null>(null);
const editingFruit = ref<Api.Farm.GameConfigFruit | null>(null);
const editingItem = ref<Api.Farm.GameConfigItem | null>(null);

const seedSortOptions = [
  { label: '名称', value: 'name' },
  { label: '种子ID', value: 'seedId' },
  { label: '价格', value: 'price' },
  { label: '等级', value: 'requiredLevel' },
  { label: '生长时间', value: 'growTime' }
];

const fruitSortOptions = [
  { label: '名称', value: 'name' },
  { label: '果实ID', value: 'id' },
  { label: '售价', value: 'price' }
];

const itemSortOptions = [
  { label: '名称', value: 'name' },
  { label: '物品ID', value: 'id' },
  { label: '价格', value: 'price' },
  { label: '类型', value: 'type' }
];

const rarityFilterOptions = [{ label: '全部稀有度', value: null }, ...rarityOptions] as SelectOption[];
const seasonFilterOptions = [{ label: '全部季节', value: null }, ...seasonOptions] as SelectOption[];

const itemTypeFilterOptions = computed(
  () => [{ label: '全部类型', value: null }, ...itemTypeOptions.value] as SelectOption[]
);

const itemTypeLabelMap = computed(() => {
  const map: Record<number, string> = {};
  itemTypeOptions.value.forEach(t => {
    map[t.value] = t.label;
  });
  return map;
});

function sortByKey<T extends Record<string, any>>(list: T[], key: string): T[] {
  return [...list].sort((a, b) => {
    const va = a[key] ?? '';
    const vb = b[key] ?? '';
    if (typeof va === 'number' && typeof vb === 'number') return va - vb;
    return String(va).localeCompare(String(vb), 'zh-CN');
  });
}

const filteredSeeds = computed(() => {
  let list = seedList.value;
  const kw = searchKeyword.value.trim().toLowerCase();
  if (kw) {
    list = list.filter(
      s =>
        s.name?.toLowerCase().includes(kw) ||
        String(s.seedId).includes(kw)
    );
  }
  if (seedSeasonFilter.value != null) {
    list = list.filter(s => s.seasons === seedSeasonFilter.value);
  }
  return sortByKey(list, seedSort.value);
});

const filteredFruits = computed(() => {
  let list = fruitList.value;
  const kw = searchKeyword.value.trim().toLowerCase();
  if (kw) {
    list = list.filter(
      f =>
        f.name?.toLowerCase().includes(kw) ||
        String(f.id).includes(kw) ||
        String(f.plantId ?? '').includes(kw)
    );
  }
  if (fruitRarityFilter.value != null) {
    list = list.filter(f => f.rarity === fruitRarityFilter.value);
  }
  return sortByKey(list, fruitSort.value);
});

const filteredItems = computed(() => {
  let list = itemList.value;
  const kw = searchKeyword.value.trim().toLowerCase();
  if (kw) {
    list = list.filter(i => i.name?.toLowerCase().includes(kw) || String(i.id).includes(kw));
  }
  if (itemTypeFilter.value != null) {
    list = list.filter(i => i.type === itemTypeFilter.value);
  }
  if (itemRarityFilter.value != null) {
    list = list.filter(i => i.rarity === itemRarityFilter.value);
  }
  return sortByKey(list, itemSort.value);
});

const filteredCount = computed(() => {
  if (activeTab.value === 'seeds') return filteredSeeds.value.length;
  if (activeTab.value === 'fruits') return filteredFruits.value.length;
  return filteredItems.value.length;
});

async function loadSeeds() {
  loading.value = true;
  const { data, error } = await fetchGetFarmGameConfigSeeds();
  loading.value = false;
  if (!error) seedList.value = data || [];
}

async function loadFruits() {
  loading.value = true;
  const { data, error } = await fetchGetFarmGameConfigFruits();
  loading.value = false;
  if (!error) fruitList.value = data || [];
}

async function loadItems() {
  loading.value = true;
  const { data, error } = await fetchGetFarmGameConfigItems();
  loading.value = false;
  if (!error) itemList.value = data || [];
}

async function loadItemTypes() {
  const { data, error } = await fetchGetFarmGameConfigItemTypes();
  if (!error && data) {
    itemTypeOptions.value = data.map((t: any) => ({ label: t.label, value: t.value }));
  }
}

function loadCurrentTab() {
  if (activeTab.value === 'seeds') return loadSeeds();
  if (activeTab.value === 'fruits') return loadFruits();
  return loadItems();
}

function handleTabChange(tab: string) {
  activeTab.value = tab as TabKey;
  searchKeyword.value = '';
  seedSeasonFilter.value = null;
  fruitRarityFilter.value = null;
  itemTypeFilter.value = null;
  itemRarityFilter.value = null;
  loadCurrentTab();
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

function confirmDelete(title: string, content: string, action: () => Promise<void>) {
  window.$dialog?.warning({
    title,
    content,
    positiveText: $t('common.confirm'),
    negativeText: $t('common.cancel'),
    onPositiveClick: action
  });
}

function handleDeleteSeed(row: Api.Farm.GameConfigSeed) {
  confirmDelete(
    $t('page.farm.gameConfig.deleteSeed'),
    `确定删除种子「${row.name}」(ID:${row.seedId})？关联植物与果实会一并删除。`,
    async () => {
      const { error } = await fetchDeleteFarmGameConfigSeed(row.seedId);
      if (!error) {
        window.$message?.success($t('common.deleteSuccess'));
        await loadSeeds();
      }
    }
  );
}

function handleDeleteFruit(row: Api.Farm.GameConfigFruit) {
  confirmDelete(
    $t('page.farm.gameConfig.deleteFruit'),
    `确定删除果实「${row.name}」(ID:${row.id})？`,
    async () => {
      const { error } = await fetchDeleteFarmGameConfigFruit(row.id);
      if (!error) {
        window.$message?.success($t('common.deleteSuccess'));
        await loadFruits();
      }
    }
  );
}

function handleDeleteItem(row: Api.Farm.GameConfigItem) {
  confirmDelete(
    $t('page.farm.gameConfig.deleteItem'),
    `确定删除道具「${row.name}」(ID:${row.id})？`,
    async () => {
      const { error } = await fetchDeleteFarmGameConfigItem(row.id);
      if (!error) {
        window.$message?.success($t('common.deleteSuccess'));
        await loadItems();
      }
    }
  );
}

function renderIcon(src: string, fallback: string) {
  const url = resolveCatalogImage(src);
  if (!url) {
    return <span class="text-gray-400">{fallback}</span>;
  }
  return (
    <NImage
      src={url}
      width={32}
      height={32}
      object-fit="contain"
      fallback-src=""
      preview-disabled
      style="background: var(--n-color); border-radius: 6px;"
    />
  );
}

const seedColumns = computed(() => [
  {
    key: 'name',
    title: $t('page.farm.gameConfig.seed'),
    minWidth: 160,
    render: (row: Api.Farm.GameConfigSeed) => (
      <div class="flex items-center gap-8px">
        {renderIcon(row.image, '🌱')}
        <span class="font-medium">{row.name}</span>
      </div>
    )
  },
  { key: 'seedId', title: $t('page.farm.gameConfig.seedId'), width: 100, align: 'center' as const },
  {
    key: 'requiredLevel',
    title: $t('page.farm.gameConfig.requiredLevel'),
    width: 80,
    align: 'center' as const,
    render: (row: Api.Farm.GameConfigSeed) => `Lv.${row.requiredLevel}`
  },
  {
    key: 'seasons',
    title: $t('page.farm.gameConfig.seasons'),
    width: 90,
    align: 'center' as const,
    render: (row: Api.Farm.GameConfigSeed) => (
      <NTag size="small" type={row.seasons === 2 ? 'info' : 'success'} bordered={false}>
        {row.seasons === 2 ? '双季' : '单季'}
      </NTag>
    )
  },
  {
    key: 'growTime',
    title: $t('page.farm.gameConfig.growTime'),
    width: 100,
    align: 'center' as const,
    render: (row: Api.Farm.GameConfigSeed) => formatGrowTime(row.growTime)
  },
  {
    key: 'harvestCount',
    title: $t('page.farm.gameConfig.harvestCount'),
    width: 90,
    align: 'center' as const
  },
  { key: 'exp', title: $t('page.farm.gameConfig.exp'), width: 80, align: 'center' as const },
  {
    key: 'price',
    title: $t('page.farm.gameConfig.price'),
    width: 110,
    align: 'center' as const,
    render: (row: Api.Farm.GameConfigSeed) => formatPrice(row.price, row.priceId)
  },
  {
    key: 'operate',
    title: $t('common.operate'),
    width: 140,
    align: 'center' as const,
    render: (row: Api.Farm.GameConfigSeed) => (
      <NSpace justify="center">
        {(
          <NButton quaternary type="primary" size="small" onClick={() => handleEditSeed(row)}>
            {$t('common.edit')}
          </NButton>
        )}
        {(
          <NButton quaternary type="error" size="small" onClick={() => handleDeleteSeed(row)}>
            {$t('common.delete')}
          </NButton>
        )}
      </NSpace>
    )
  }
]);

const fruitColumns = computed(() => [
  {
    key: 'name',
    title: $t('page.farm.gameConfig.fruit'),
    minWidth: 160,
    render: (row: Api.Farm.GameConfigFruit) => (
      <div class="flex items-center gap-8px">
        {renderIcon(row.image, '🍎')}
        <span class="font-medium">{row.name}</span>
      </div>
    )
  },
  { key: 'id', title: $t('page.farm.gameConfig.fruitId'), width: 100, align: 'center' as const },
  {
    key: 'plantName',
    title: $t('page.farm.gameConfig.plant'),
    width: 120,
    align: 'center' as const,
    render: (row: Api.Farm.GameConfigFruit) => row.plantName || '-'
  },
  {
    key: 'rarity',
    title: $t('page.farm.gameConfig.rarity'),
    width: 90,
    align: 'center' as const,
    render: (row: Api.Farm.GameConfigFruit) => rarityLabelMap[row.rarity] || row.rarity
  },
  {
    key: 'price',
    title: $t('page.farm.gameConfig.price'),
    width: 110,
    align: 'center' as const,
    render: (row: Api.Farm.GameConfigFruit) => formatPrice(row.price, row.priceId)
  },
  {
    key: 'operate',
    title: $t('common.operate'),
    width: 140,
    align: 'center' as const,
    render: (row: Api.Farm.GameConfigFruit) => (
      <NSpace justify="center">
        {(
          <NButton quaternary type="primary" size="small" onClick={() => handleEditFruit(row)}>
            {$t('common.edit')}
          </NButton>
        )}
        {(
          <NButton quaternary type="error" size="small" onClick={() => handleDeleteFruit(row)}>
            {$t('common.delete')}
          </NButton>
        )}
      </NSpace>
    )
  }
]);

const itemColumns = computed(() => [
  {
    key: 'name',
    title: $t('page.farm.gameConfig.item'),
    minWidth: 160,
    render: (row: Api.Farm.GameConfigItem) => (
      <div class="flex items-center gap-8px">
        {renderIcon(row.image, '🎒')}
        <span class="font-medium">{row.name}</span>
      </div>
    )
  },
  { key: 'id', title: $t('page.farm.gameConfig.itemId'), width: 100, align: 'center' as const },
  {
    key: 'type',
    title: $t('page.farm.gameConfig.itemType'),
    width: 110,
    align: 'center' as const,
    render: (row: Api.Farm.GameConfigItem) => itemTypeLabelMap.value[row.type] || row.type
  },
  {
    key: 'rarity',
    title: $t('page.farm.gameConfig.rarity'),
    width: 90,
    align: 'center' as const,
    render: (row: Api.Farm.GameConfigItem) => rarityLabelMap[row.rarity] || row.rarity
  },
  {
    key: 'price',
    title: $t('page.farm.gameConfig.price'),
    width: 110,
    align: 'center' as const,
    render: (row: Api.Farm.GameConfigItem) => formatPrice(row.price, row.priceId)
  },
  {
    key: 'operate',
    title: $t('common.operate'),
    width: 140,
    align: 'center' as const,
    render: (row: Api.Farm.GameConfigItem) => (
      <NSpace justify="center">
        {(
          <NButton quaternary type="primary" size="small" onClick={() => handleEditItem(row)}>
            {$t('common.edit')}
          </NButton>
        )}
        {(
          <NButton quaternary type="error" size="small" onClick={() => handleDeleteItem(row)}>
            {$t('common.delete')}
          </NButton>
        )}
      </NSpace>
    )
  }
]);

const tableColumns = computed(() => {
  if (activeTab.value === 'seeds') return seedColumns.value;
  if (activeTab.value === 'fruits') return fruitColumns.value;
  return itemColumns.value;
});

const tableData = computed(() => {
  if (activeTab.value === 'seeds') return filteredSeeds.value;
  if (activeTab.value === 'fruits') return filteredFruits.value;
  return filteredItems.value;
});

const rowKey = (row: any) => {
  if (activeTab.value === 'seeds') return row.seedId;
  return row.id;
};

const addLabel = computed(() => {
  if (activeTab.value === 'seeds') return $t('page.farm.gameConfig.addSeed');
  if (activeTab.value === 'fruits') return $t('page.farm.gameConfig.addFruit');
  return $t('page.farm.gameConfig.addItem');
});

onMounted(async () => {
  await loadItemTypes();
  await loadCurrentTab();
});
</script>

<template>
  <div class="min-h-500px flex-col-stretch gap-16px overflow-hidden lt-sm:overflow-auto">
    <NCard
      :title="$t('page.farm.gameConfig.title')"
      :bordered="false"
      size="small"
      class="card-wrapper sm:flex-1-hidden"
    >
      <NTabs :value="activeTab" type="line" @update:value="handleTabChange">
        <NTabPane
          name="seeds"
          :tab="`${$t('page.farm.gameConfig.tabSeeds')}${seedList.length ? ` (${seedList.length})` : ''}`"
        />
        <NTabPane
          name="fruits"
          :tab="`${$t('page.farm.gameConfig.tabFruits')}${fruitList.length ? ` (${fruitList.length})` : ''}`"
        />
        <NTabPane
          name="items"
          :tab="`${$t('page.farm.gameConfig.tabItems')}${itemList.length ? ` (${itemList.length})` : ''}`"
        />
      </NTabs>

      <div class="mb-12px mt-12px flex flex-wrap items-center gap-12px">
        <NButton size="small" type="primary" ghost @click="handleAdd">
          {{ addLabel }}
        </NButton>
        <NInput
          v-model:value="searchKeyword"
          clearable
          size="small"
          class="w-200px"
          :placeholder="$t('page.farm.gameConfig.keyword')"
        />
        <NSelect
          v-if="activeTab === 'seeds'"
          v-model:value="seedSeasonFilter"
          size="small"
          class="w-140px"
          :options="seasonFilterOptions"
          :consistent-menu-width="false"
        />
        <NSelect
          v-if="activeTab === 'seeds'"
          v-model:value="seedSort"
          size="small"
          class="w-140px"
          :options="seedSortOptions"
        />
        <NSelect
          v-if="activeTab === 'fruits'"
          v-model:value="fruitRarityFilter"
          size="small"
          class="w-140px"
          :options="rarityFilterOptions"
        />
        <NSelect
          v-if="activeTab === 'fruits'"
          v-model:value="fruitSort"
          size="small"
          class="w-140px"
          :options="fruitSortOptions"
        />
        <NSelect
          v-if="activeTab === 'items'"
          v-model:value="itemTypeFilter"
          size="small"
          class="w-160px"
          clearable
          :options="itemTypeFilterOptions"
        />
        <NSelect
          v-if="activeTab === 'items'"
          v-model:value="itemRarityFilter"
          size="small"
          class="w-140px"
          :options="rarityFilterOptions"
        />
        <NSelect
          v-if="activeTab === 'items'"
          v-model:value="itemSort"
          size="small"
          class="w-140px"
          :options="itemSortOptions"
        />
        <span class="text-12px text-gray-400">{{ filteredCount }} 条</span>
        <div class="flex-1" />
        <NButton size="small" quaternary :loading="loading" @click="loadCurrentTab">
          {{ $t('common.refresh') }}
        </NButton>
      </div>

      <NDataTable
        :columns="tableColumns"
        :data="tableData"
        size="small"
        :flex-height="!appStore.isMobile"
        :loading="loading"
        :row-key="rowKey"
        class="sm:h-full"
      />
    </NCard>

    <SeedOperateDrawer
      v-model:visible="seedDrawerVisible"
      :operate-type="operateType"
      :row-data="editingSeed"
      @submitted="loadSeeds"
    />
    <FruitOperateDrawer
      v-model:visible="fruitDrawerVisible"
      :operate-type="operateType"
      :row-data="editingFruit"
      @submitted="loadFruits"
    />
    <ItemOperateDrawer
      v-model:visible="itemDrawerVisible"
      :operate-type="operateType"
      :row-data="editingItem"
      @submitted="loadItems"
    />
  </div>
</template>
