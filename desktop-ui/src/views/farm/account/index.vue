<script setup lang="tsx">
import { onMounted, ref } from 'vue';
import dayjs from 'dayjs';
import { NButton, NPopconfirm, NTag, NTooltip } from 'naive-ui';
import { farmAuthStatusRecord, farmPlatformRecord, farmRunStatusRecord } from '@/constants/business';
import {
  fetchDeleteFarmAccount,
  fetchGetFarmAccountList,
  fetchStartFarmAccount,
  fetchStopFarmAccount
} from '@/service/api';
import type { FlatResponseData } from '@sa/axios';
import { useAppStore } from '@/store/modules/app';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { useFarmWs } from '@/hooks/business/farm-ws';
import { defaultTransform, useNaivePaginatedTable, useTableOperate } from '@/hooks/common/table';
import { $t } from '@/locales';
import AccountOperateDrawer from './modules/account-operate-drawer.vue';
import AccountSearch from './modules/account-search.vue';

defineOptions({
  name: 'FarmAccount'
});

const appStore = useAppStore();
const farmAccountStore = useFarmAccountStore();

const searchParams = ref<Api.Farm.AccountSearchParams>({
  current: 1,
  size: 10,
  keyword: null,
  platform: null,
  runStatus: null,
  authStatus: null
});

function authStatusOf(row: Api.Farm.Account): 'authorized' | 'unauthorized' | 'rescanRecommended' {
  if (!row.wxAuthorized) {
    return 'unauthorized';
  }
  return row.wxRescanRecommended ? 'rescanRecommended' : 'authorized';
}

const { columns, columnChecks, data, getData, getDataByPage, loading, mobilePagination } =
  useNaivePaginatedTable<FlatResponseData<any, Api.Farm.AccountList>, Api.Farm.Account>({
  api: () => fetchGetFarmAccountList(searchParams.value),
  transform: response => defaultTransform(response),
  immediate: false,
  onPaginationParamsChange: params => {
    searchParams.value.current = params.page;
    searchParams.value.size = params.pageSize;
  },
  columns: () => [
    {
      type: 'selection',
      align: 'center',
      width: 48
    },
    {
      key: 'index',
      title: $t('common.index'),
      align: 'center',
      width: 64,
      render: (_, index) => index + 1
    },
    {
      key: 'name',
      title: $t('page.farm.account.name'),
      align: 'center'
    },
    {
      key: 'platform',
      title: $t('page.farm.account.platform'),
      align: 'center',
      render: row => {
        const key = row.platform as Api.Farm.Platform;
        return farmPlatformRecord[key] ? $t(farmPlatformRecord[key]) : row.platform;
      }
    },
    {
      key: 'runStatus',
      title: $t('page.farm.account.runStatus'),
      align: 'center',
      render: row => {
        const tagMap: Record<Api.Farm.RunStatus, NaiveUI.ThemeColor> = {
          0: 'default',
          1: 'success',
          2: 'error'
        };
        return <NTag type={tagMap[row.runStatus]}>{$t(farmRunStatusRecord[row.runStatus])}</NTag>;
      }
    },
    {
      key: 'wxAuthorized',
      title: $t('page.farm.account.authStatus'),
      align: 'center',
      render: row => {
        const status = authStatusOf(row);
        const tagMap: Record<'authorized' | 'unauthorized' | 'rescanRecommended', NaiveUI.ThemeColor> = {
          authorized: 'success',
          unauthorized: 'warning',
          rescanRecommended: 'warning'
        };
        const tag = <NTag type={tagMap[status]}>{$t(farmAuthStatusRecord[status])}</NTag>;
        if (status !== 'rescanRecommended') {
          return tag;
        }
        return (
          <NTooltip>
            {{
              default: () => $t('page.farm.common.authStatus.rescanHint'),
              trigger: () => tag
            }}
          </NTooltip>
        );
      }
    },
    {
      key: 'lastOnlineAt',
      title: $t('page.farm.account.lastOnlineAt'),
      align: 'center',
      render: row => (row.lastOnlineAt ? dayjs.unix(row.lastOnlineAt).format('YYYY-MM-DD HH:mm') : '-')
    },
    {
      key: 'operate',
      title: $t('common.operate'),
      align: 'center',
      width: 280,
      render: row => (
        <div class="flex-center gap-8px">
          {(
            <NButton type="primary" ghost size="small" onClick={() => edit(row.id)}>
              {$t('common.edit')}
            </NButton>
          )}
          {row.runStatus !== 1 && (
            <NButton type="success" ghost size="small" onClick={() => handleStart(row.id)}>
              {$t('page.farm.account.start')}
            </NButton>
          )}
          {row.runStatus === 1 && (
            <NButton type="warning" ghost size="small" onClick={() => handleStop(row.id)}>
              {$t('page.farm.account.stop')}
            </NButton>
          )}
          {(
            <NPopconfirm onPositiveClick={() => handleDelete(row.id)}>
              {{
                default: () => $t('page.farm.account.deleteConfirm'),
                trigger: () => (
                  <NButton type="error" ghost size="small">
                    {$t('common.delete')}
                  </NButton>
                )
              }}
            </NPopconfirm>
          )}
        </div>
      )
    }
  ]
});

const { drawerVisible, operateType, editingData, handleAdd, handleEdit, checkedRowKeys } = useTableOperate(
  data,
  'id',
  getData
);

function onAdd() {
  handleAdd();
}

function edit(id: number) {
  handleEdit(id);
}

async function handleStart(id: number) {
  const { error } = await fetchStartFarmAccount(id);
  if (!error) {
    window.$message?.success($t('common.updateSuccess'));
    await getDataByPage();
  }
}

async function handleStop(id: number) {
  const { error } = await fetchStopFarmAccount(id);
  if (!error) {
    window.$message?.success($t('common.updateSuccess'));
    await getDataByPage();
  }
}

async function handleDelete(id: number) {
  const { error } = await fetchDeleteFarmAccount(id);
  if (!error) {
    window.$message?.success($t('common.deleteSuccess'));
    await getDataByPage();
  }
}

async function refreshList(page: number = 1) {
  await getDataByPage(page);
  void farmAccountStore.loadAccounts();
}

onMounted(async () => {
  await refreshList();
});

useFarmWs({
  onMessage(type, payload) {
    if (type === 'account_status') {
      void refreshList(searchParams.value.current || 1);
      return;
    }
    if (type === 'status:update') {
      const body = payload as { status?: { wxAuthorized?: boolean } };
      if (body?.status?.wxAuthorized === false) {
        void refreshList(searchParams.value.current || 1);
      }
    }
  }
});
</script>

<template>
  <div class="min-h-500px flex-col-stretch gap-16px overflow-hidden lt-sm:overflow-auto">
    <AccountSearch v-model:model="searchParams" @search="() => refreshList()" />
    <NCard :title="$t('page.farm.account.title')" :bordered="false" size="small" class="card-wrapper sm:flex-1-hidden">
      <template #header-extra>
        <TableHeaderOperation
          v-model:columns="columnChecks"
          :disabled-delete="checkedRowKeys.length === 0"
          :loading="loading"
          @add="onAdd"
          @refresh="() => refreshList()"
        >
          <template #default>
            <NButton size="small" ghost type="primary" @click="onAdd">
              <template #icon>
                <icon-ic-round-plus class="text-icon" />
              </template>
              {{ $t('common.add') }}
            </NButton>
          </template>
        </TableHeaderOperation>
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
        :row-key="row => row.id"
        :pagination="mobilePagination"
        class="sm:h-full"
      />
    </NCard>
    <AccountOperateDrawer
      v-model:visible="drawerVisible"
      :operate-type="operateType"
      :row-data="editingData"
      @submitted="() => refreshList()"
    />
  </div>
</template>

<style scoped></style>

<style scoped></style>
