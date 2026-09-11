<script setup lang="ts" generic="Row extends object">
import { computed } from 'vue';
import type { DataTableBaseColumn, PaginationProps } from 'naive-ui';
import { useAppStore } from '@/store/modules/app';

const props = defineProps<{
  data: Row[];
  columns: NaiveUI.TableColumn<Row>[];
  rowKey: (row: Row) => string | number;
  primaryKeys: string[];
  pagination: PaginationProps;
  loading: boolean;
}>();
const checked = defineModel<Array<string | number>>('checkedRowKeys', { default: () => [] });
const appStore = useAppStore();
const en = computed(() => appStore.locale === 'en-US');
type Field = NaiveUI.SetTableColumnKey<DataTableBaseColumn<Row>, Row>;
const fields = computed(() =>
  props.columns.filter(
    (column): column is Field => 'key' in column && !('children' in column) && column.key !== 'index'
  )
);
const primary = computed(() => fields.value.filter(column => props.primaryKeys.includes(String(column.key))));
const secondary = computed(() =>
  fields.value.filter(column => !props.primaryKeys.includes(String(column.key)) && column.key !== 'operate')
);
const actions = computed(() => fields.value.find(column => column.key === 'operate'));
const allChecked = computed(
  () => props.data.length > 0 && props.data.every(row => checked.value.includes(props.rowKey(row)))
);
const partiallyChecked = computed(
  () => !allChecked.value && props.data.some(row => checked.value.includes(props.rowKey(row)))
);

function selectRow(row: Row, value: boolean) {
  const key = props.rowKey(row);
  checked.value = value ? [...new Set([...checked.value, key])] : checked.value.filter(item => item !== key);
}

function selectPage(value: boolean) {
  const pageKeys = props.data.map(props.rowKey);
  checked.value = value
    ? [...new Set([...checked.value, ...pageKeys])]
    : checked.value.filter(key => !pageKeys.includes(key));
}

function Cell({ column, row, index }: { column: Field; row: Row; index: number }) {
  return column.render ? column.render(row, index) : String(row[column.key as keyof Row] ?? '—');
}

function Label({ column }: { column: Field }) {
  return typeof column.title === 'function' ? column.title(column as DataTableBaseColumn) : column.title;
}
</script>

<template>
  <NSpin :show="loading">
    <div class="mobile-record-list flex flex-col gap-12px">
      <NCheckbox
        v-if="data.length"
        :checked="allChecked"
        :indeterminate="partiallyChecked"
        @update:checked="selectPage"
      >
        {{ en ? 'Select this page' : '选择本页' }} · {{ checked.length }}
      </NCheckbox>
      <NEmpty v-if="!data.length && !loading" :description="en ? 'No records' : '暂无数据'" class="py-24px" />
      <article v-for="(row, index) in data" :key="rowKey(row)" class="record-card">
        <div class="flex items-start gap-12px">
          <NCheckbox
            :checked="checked.includes(rowKey(row))"
            :aria-label="en ? 'Select record' : '选择记录'"
            @update:checked="selectRow(row, $event)"
          />
          <dl class="min-w-0 flex-1 space-y-8px">
            <div v-for="column in primary" :key="String(column.key)" class="record-field">
              <dt><Label :column="column" /></dt>
              <dd><Cell :column="column" :row="row" :index="index" /></dd>
            </div>
          </dl>
        </div>
        <details v-if="secondary.length" class="mt-8px">
          <summary>{{ en ? 'Details' : '详细信息' }}</summary>
          <dl class="space-y-8px pb-12px">
            <div v-for="column in secondary" :key="String(column.key)" class="record-field">
              <dt><Label :column="column" /></dt>
              <dd><Cell :column="column" :row="row" :index="index" /></dd>
            </div>
          </dl>
        </details>
        <div v-if="actions" class="record-actions mt-12px"><Cell :column="actions" :row="row" :index="index" /></div>
      </article>
      <NPagination v-if="pagination.itemCount" v-bind="pagination" class="record-pagination" />
    </div>
  </NSpin>
</template>

<style scoped>
.record-card {
  border: 1px solid var(--n-border-color, #8884);
  border-radius: 10px;
  padding: 12px;
}
.record-field {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px 12px;
}
.record-field dt {
  opacity: 0.65;
  font-size: 12px;
}
.record-field dd {
  min-width: 0;
  overflow-wrap: anywhere;
}
summary {
  min-height: 44px;
  line-height: 44px;
  cursor: pointer;
}
.record-actions :deep(.flex-center) {
  flex-wrap: wrap;
  justify-content: flex-start;
}
.record-pagination {
  flex-wrap: wrap;
  justify-content: center;
  gap: 8px;
}
</style>
