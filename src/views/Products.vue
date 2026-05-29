<script setup lang="ts">
import { ref, onMounted, h } from 'vue';
import { NDataTable, NButton, NModal, NForm, NFormItem, NInput, NInputNumber, NSelect, NSpace, NPopconfirm, useMessage } from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';

interface Product {
  id: number; name: string; sku: string; brand_id: number | null; brand_name: string | null;
  category_id: number | null; category_name: string | null; unit: string; spec: string;
  purchase_price: number; sale_price: number; min_stock: number;
  is_active: boolean; notes: string; created_at: string;
}

const message = useMessage();
const loading = ref(false);
const items = ref<Product[]>([]);
const total = ref(0);
const page = ref(1);
const keyword = ref('');
const brandFilter = ref<number | null>(null);
const brands = ref<{ id: number; name: string }[]>([]);
const categories = ref<{ id: number; name: string }[]>([]);

const showModal = ref(false);
const editing = ref<Product | null>(null);
const form = ref({ name: '', sku: '', brand_id: null as number | null, category_id: null as number | null,
  unit: '个', spec: '', purchase_price: 0, sale_price: 0, min_stock: 0, notes: '' });

const columns: DataTableColumns<Product> = [
  { title: 'SKU', key: 'sku', width: 120 },
  { title: '名称', key: 'name', ellipsis: { tooltip: true } },
  { title: '品牌', key: 'brand_name', width: 80 },
  { title: '分类', key: 'category_name', width: 80 },
  { title: '采购价', key: 'purchase_price', width: 90, render: r => `¥${r.purchase_price}` },
  { title: '销售价', key: 'sale_price', width: 90, render: r => `¥${r.sale_price}` },
  { title: '单位', key: 'unit', width: 60 },
  {
    title: '操作', key: 'actions', width: 160,
    render(row) {
      return h(NSpace, {}, {
        default: () => [
          h(NButton, { size: 'small', onClick: () => editItem(row) }, { default: () => '编辑' }),
          h(NPopconfirm, { onPositiveClick: () => removeItem(row.id) }, {
            trigger: () => h(NButton, { size: 'small', type: 'error' }, { default: () => '删除' }),
            default: () => '确定删除该产品？',
          }),
        ],
      });
    },
  },
];

async function loadBrandsCategories() {
  try {
    const [b, c] = await Promise.all([
      invoke<{ id: number; name: string }[]>('list_brands'),
      invoke<{ id: number; name: string }[]>('list_categories'),
    ]);
    brands.value = b;
    categories.value = c;
  } catch (_) {}
}

async function loadData() {
  loading.value = true;
  try {
    const res = await invoke<{ products: Product[]; total: number }>('list_products', {
      page: page.value, pageSize: 20,
      keyword: keyword.value || null, brandId: brandFilter.value,
    });
    items.value = res.products; total.value = res.total;
  } catch (e: any) { message.error(typeof e === 'string' ? e : '加载失败'); }
  finally { loading.value = false; }
}

function openCreate() {
  editing.value = null; form.value = { name: '', sku: '', brand_id: null, category_id: null,
    unit: '个', spec: '', purchase_price: 0, sale_price: 0, min_stock: 0, notes: '' };
  showModal.value = true;
}
function editItem(row: Product) {
  editing.value = row; form.value = { name: row.name, sku: row.sku, brand_id: row.brand_id,
    category_id: row.category_id, unit: row.unit, spec: row.spec,
    purchase_price: row.purchase_price, sale_price: row.sale_price,
    min_stock: row.min_stock, notes: row.notes };
  showModal.value = true;
}

async function saveItem() {
  if (!form.value.name.trim() || !form.value.sku.trim()) { message.warning('名称和SKU必填'); return; }
  try {
    if (editing.value) {
      await invoke('update_product', { id: editing.value.id, data: form.value });
      message.success('更新成功');
    } else {
      await invoke('create_product', { data: form.value });
      message.success('创建成功');
    }
    showModal.value = false; loadData();
  } catch (e: any) { message.error(typeof e === 'string' ? e : '操作失败'); }
}

async function removeItem(id: number) {
  try { await invoke('delete_product', { id }); message.success('已删除'); loadData(); }
  catch (e: any) { message.error(typeof e === 'string' ? e : '删除失败'); }
}

onMounted(() => { loadBrandsCategories(); loadData(); });
</script>

<template>
  <div>
    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:16px; flex-wrap:wrap; gap:8px">
      <NSpace>
        <NInput v-model:value="keyword" placeholder="搜索产品..." style="width:200px" clearable @keyup.enter="page=1;loadData()" />
        <NSelect v-model:value="brandFilter" :options="brands.map(b=>({label:b.name,value:b.id}))" placeholder="品牌筛选" clearable style="width:140px" @update:value="page=1;loadData()" />
        <NButton @click="page=1;loadData()">搜索</NButton>
      </NSpace>
      <NButton type="primary" @click="openCreate">新增产品</NButton>
    </div>
    <NDataTable :columns="columns" :data="items" :loading="loading"
      :pagination="{ page, pageSize: 20, itemCount: total, onChange: (p:number) => { page=p; loadData(); } }" />

    <NModal v-model:show="showModal" :title="editing ? '编辑产品' : '新增产品'" style="width:640px">
      <NForm style="padding:24px" label-width="80">
        <NFormItem label="名称" required><NInput v-model:value="form.name" /></NFormItem>
        <NFormItem label="SKU" required><NInput v-model:value="form.sku" /></NFormItem>
        <NSpace>
          <NFormItem label="品牌"><NSelect v-model:value="form.brand_id" :options="brands.map(b=>({label:b.name,value:b.id}))" clearable style="width:200px" /></NFormItem>
          <NFormItem label="分类"><NSelect v-model:value="form.category_id" :options="categories.map(c=>({label:c.name,value:c.id}))" clearable style="width:200px" /></NFormItem>
        </NSpace>
        <NSpace>
          <NFormItem label="采购价"><NInputNumber v-model:value="form.purchase_price" :min="0" style="width:160px"><template #prefix>¥</template></NInputNumber></NFormItem>
          <NFormItem label="销售价"><NInputNumber v-model:value="form.sale_price" :min="0" style="width:160px"><template #prefix>¥</template></NInputNumber></NFormItem>
        </NSpace>
        <NSpace>
          <NFormItem label="单位"><NInput v-model:value="form.unit" style="width:120px" /></NFormItem>
          <NFormItem label="规格"><NInput v-model:value="form.spec" style="width:160px" /></NFormItem>
          <NFormItem label="最低库存"><NInputNumber v-model:value="form.min_stock" :min="0" style="width:120px" /></NFormItem>
        </NSpace>
        <NFormItem label="备注"><NInput v-model:value="form.notes" type="textarea" /></NFormItem>
        <NSpace justify="end" style="width:100%">
          <NButton @click="showModal=false">取消</NButton>
          <NButton type="primary" @click="saveItem">保存</NButton>
        </NSpace>
      </NForm>
    </NModal>
  </div>
</template>
