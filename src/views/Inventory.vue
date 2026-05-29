<script setup lang="ts">
import { ref, onMounted, h } from 'vue';
import { NDataTable, NButton, NModal, NForm, NFormItem, NInput, NInputNumber, NSpace, NTag, useMessage } from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';

interface StockItem {
  product_id: number; product_name: string; sku: string;
  quantity: number; locked_quantity: number; available_quantity: number;
  min_stock: number; unit: string; updated_at: string;
}

const message = useMessage();
const loading = ref(false);
const items = ref<StockItem[]>([]);
const keyword = ref('');
const lowStockOnly = ref(false);

const showModal = ref(false);
const modalMode = ref<'in' | 'out'>('in');
const selected = ref<StockItem | null>(null);
const qty = ref(1);
const notes = ref('');

const columns: DataTableColumns<StockItem> = [
  { title: 'SKU', key: 'sku', width: 120 },
  { title: '产品', key: 'product_name', ellipsis: { tooltip: true } },
  {
    title: '库存量', key: 'quantity', width: 100,
    render(row) {
      const isLow = row.quantity <= row.min_stock;
      return h(NSpace, { justify: 'center' }, {
        default: () => [
          h('span', {}, row.quantity.toString()),
          isLow ? h(NTag, { type: 'error', size: 'small' }, { default: () => '低' }) : null,
        ],
      });
    },
  },
  { title: '可用', key: 'available_quantity', width: 80 },
  { title: '最低库存', key: 'min_stock', width: 90 },
  { title: '单位', key: 'unit', width: 60 },
  { title: '更新时间', key: 'updated_at', width: 160 },
  {
    title: '操作', key: 'actions', width: 160,
    render(row) {
      return h(NSpace, {}, {
        default: () => [
          h(NButton, { size: 'small', type: 'primary', onClick: () => openStockOp(row, 'in') }, { default: () => '入库' }),
          h(NButton, { size: 'small', type: 'warning', onClick: () => openStockOp(row, 'out') }, { default: () => '出库' }),
        ],
      });
    },
  },
];

async function loadData() {
  loading.value = true;
  try {
    items.value = await invoke<StockItem[]>('get_stock', {
      keyword: keyword.value || null, lowStockOnly: lowStockOnly.value || null,
    });
  } catch (e: any) { message.error(typeof e === 'string' ? e : '加载失败'); }
  finally { loading.value = false; }
}

function openStockOp(row: StockItem, mode: 'in' | 'out') {
  selected.value = row; modalMode.value = mode; qty.value = 1; notes.value = '';
  showModal.value = true;
}

async function confirmStock() {
  if (qty.value <= 0) { message.warning('数量必须大于0'); return; }
  try {
    if (modalMode.value === 'in') {
      await invoke('stock_in', { productId: selected.value!.product_id, quantity: qty.value, notes: notes.value || null });
      message.success(`入库成功: +${qty.value}`);
    } else {
      await invoke('stock_out', { productId: selected.value!.product_id, quantity: qty.value, notes: notes.value || null });
      message.success(`出库成功: -${qty.value}`);
    }
    showModal.value = false; loadData();
  } catch (e: any) { message.error(typeof e === 'string' ? e : '操作失败'); }
}

onMounted(loadData);
</script>

<template>
  <div>
    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:16px">
      <NSpace>
        <NInput v-model:value="keyword" placeholder="搜索产品..." style="width:240px" clearable @keyup.enter="loadData()" />
        <NButton @click="loadData()">搜索</NButton>
        <NButton :type="lowStockOnly ? 'warning' : 'default'" @click="lowStockOnly=!lowStockOnly;loadData()">
          仅看低库存
        </NButton>
      </NSpace>
    </div>
    <NDataTable :columns="columns" :data="items" :loading="loading" :row-class-name="(row:StockItem) => row.quantity <= row.min_stock ? 'row-low-stock' : ''" />

    <NModal v-model:show="showModal" :title="modalMode==='in' ? '入库' : '出库'">
      <NForm style="padding:24px">
        <p style="margin-bottom:12px">
          <strong>产品：</strong>{{ selected?.product_name }}（{{ selected?.sku }}）
        </p>
        <p style="margin-bottom:16px">
          <strong>当前库存：</strong>{{ selected?.quantity }} {{ selected?.unit }}
        </p>
        <NFormItem label="数量" required>
          <NInputNumber v-model:value="qty" :min="1" style="width:200px" />
        </NFormItem>
        <NFormItem label="备注"><NInput v-model:value="notes" /></NFormItem>
        <NSpace justify="end" style="width:100%">
          <NButton @click="showModal=false">取消</NButton>
          <NButton :type="modalMode==='in' ? 'primary' : 'warning'" @click="confirmStock">
            {{ modalMode === 'in' ? '确认入库' : '确认出库' }}
          </NButton>
        </NSpace>
      </NForm>
    </NModal>
  </div>
</template>

<style scoped>
.row-low-stock td { background: #fff2e8; }
</style>
