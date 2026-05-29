<script setup lang="ts">
import { ref, onMounted, h } from 'vue';
import { NDataTable, NButton, NModal, NForm, NFormItem, NInput, NInputNumber, NSelect, NSpace, NTag, useMessage } from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';

interface SalesOrder {
  id: number; order_no: string; customer_id: number; customer_name: string;
  total_amount: number; profit: number; status: string;
  order_date: string; delivery_date: string | null; items_count: number;
}

const message = useMessage();
const loading = ref(false);
const orders = ref<SalesOrder[]>([]);
const statusFilter = ref<string | null>(null);
const page = ref(1);

const customers = ref<{ id: number; name: string }[]>([]);
const products = ref<{ id: number; name: string; sku: string; sale_price: number; purchase_price: number }[]>([]);
const showModal = ref(false);
const form = ref({ customer_id: null as number | null, delivery_date: '', notes: '',
  items: [] as { _uid: number; product_id: number | null; quantity: number; unit_price: number; cost_price: number | null }[] });

const statusMap: Record<string, { label: string; type: string }> = {
  draft: { label: '草稿', type: 'default' },
  confirmed: { label: '已确认', type: 'info' },
  shipped: { label: '已发货', type: 'success' },
  cancelled: { label: '已取消', type: 'error' },
};

const columns: DataTableColumns<SalesOrder> = [
  { title: '订单号', key: 'order_no', width: 160 },
  { title: '客户', key: 'customer_name' },
  { title: '金额', key: 'total_amount', width: 100, render: r => `¥${r.total_amount}` },
  { title: '毛利', key: 'profit', width: 100, render: r => h('span', { style: { color: r.profit >= 0 ? '#18a058' : '#d03050' } }, `¥${r.profit}`) },
  { title: '状态', key: 'status', width: 80, render: r => h(NTag, { type: (statusMap[r.status]?.type || 'default') as any, size: 'small' }, { default: () => statusMap[r.status]?.label || r.status }) },
  { title: '日期', key: 'order_date', width: 100 },
  { title: '项数', key: 'items_count', width: 60 },
  {
    title: '操作', key: 'actions', width: 180,
    render(row) {
      return h(NSpace, {}, {
        default: () => [
          row.status === 'draft' ? h(NButton, { size: 'small', type: 'primary', onClick: () => confirmOrder(row.id) }, { default: () => '确认' }) : null,
          row.status === 'confirmed' ? h(NButton, { size: 'small', type: 'warning', onClick: () => shipOrder(row.id) }, { default: () => '发货' }) : null,
        ],
      });
    },
  },
];

async function loadCustomersProducts() {
  try {
    const [c, res] = await Promise.all([
      invoke<{ id: number; name: string }[]>('list_customers'),
      invoke<{ products: any[] }>('list_products', { pageSize: 200 }),
    ]);
    customers.value = c;
    products.value = res.products.map(p => ({ id: p.id, name: p.name, sku: p.sku, sale_price: p.sale_price, purchase_price: p.purchase_price }));
  } catch (_) {}
}

async function loadOrders() {
  loading.value = true;
  try {
    orders.value = await invoke<SalesOrder[]>('list_sales_orders', {
      status: statusFilter.value || null, page: page.value, pageSize: 20,
    });
  } catch (e: any) { message.error(typeof e === 'string' ? e : '加载失败'); }
  finally { loading.value = false; }
}

let nextUid = 1;

function openCreate() {
  form.value = { customer_id: null, delivery_date: '', notes: '', items: [{ _uid: nextUid++, product_id: null, quantity: 1, unit_price: 0, cost_price: null }] };
  showModal.value = true;
}

function addItem() { form.value.items.push({ _uid: nextUid++, product_id: null, quantity: 1, unit_price: 0, cost_price: null }); }
function removeItem(idx: number) { if (form.value.items.length > 1) form.value.items.splice(idx, 1); }

async function saveOrder() {
  if (!form.value.customer_id) { message.warning('请选择客户'); return; }
  const validItems = form.value.items.filter(i => i.product_id && i.quantity > 0);
  if (!validItems.length) { message.warning('请至少添加一项有效的销售明细'); return; }
  try {
    await invoke('create_sales_order', { data: { ...form.value, items: validItems } });
    message.success('销售单已创建');
    showModal.value = false; loadOrders();
  } catch (e: any) { message.error(typeof e === 'string' ? e : '创建失败'); }
}

async function confirmOrder(id: number) {
  try { await invoke('confirm_sales_order', { id }); message.success('已确认'); loadOrders(); }
  catch (e: any) { message.error(typeof e === 'string' ? e : '确认失败'); }
}

async function shipOrder(id: number) {
  try { await invoke('ship_sales_order', { id }); message.success('已发货（库存已扣减）'); loadOrders(); }
  catch (e: any) { message.error(typeof e === 'string' ? e : '发货失败'); }
}

onMounted(() => { loadCustomersProducts(); loadOrders(); });
</script>

<template>
  <div>
    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:16px">
      <NSpace>
        <NSelect v-model:value="statusFilter" :options="[{label:'全部',value:'all'},{label:'草稿',value:'draft'},{label:'已确认',value:'confirmed'},{label:'已发货',value:'shipped'}]" placeholder="状态" clearable style="width:120px" @update:value="page=1;loadOrders()" />
        <NButton @click="page=1;loadOrders()">刷新</NButton>
      </NSpace>
      <NButton type="primary" @click="openCreate">新建销售单</NButton>
    </div>
    <NDataTable :columns="columns" :data="orders" :loading="loading" :pagination="{ page, pageSize: 20, onChange: (p:number) => { page=p; loadOrders(); } }" />

    <NModal v-model:show="showModal" title="新建销售单" style="width:740px">
      <NForm style="padding:24px">
        <NSpace>
          <NFormItem label="客户" required><NSelect v-model:value="form.customer_id" :options="customers.map(c=>({label:c.name,value:c.id}))" style="width:240px" /></NFormItem>
          <NFormItem label="预计交付"><NInput v-model:value="form.delivery_date" placeholder="YYYY-MM-DD" style="width:180px" /></NFormItem>
        </NSpace>
        <div style="margin-bottom:12px; font-weight:bold">销售明细</div>
        <div v-for="(item, idx) in form.items" :key="item._uid" style="display:flex; gap:8px; align-items:center; margin-bottom:8px">
          <NSelect v-model:value="item.product_id" :options="products.map(p=>({label:`${p.name}(${p.sku})`,value:p.id}))" placeholder="产品" style="flex:2" @update:value="(v:number)=>{const p=products.find(x=>x.id===v); if(p){item.unit_price=p.sale_price; item.cost_price=p.purchase_price;}}" />
          <NInputNumber v-model:value="item.quantity" :min="1" placeholder="数量" style="width:80px" />
          <NInputNumber v-model:value="item.unit_price" :min="0" placeholder="单价" style="width:100px"><template #prefix>¥</template></NInputNumber>
          <NButton size="small" type="error" @click="removeItem(idx)" :disabled="form.items.length<=1">×</NButton>
        </div>
        <NButton size="small" @click="addItem" style="margin-bottom:16px">+ 添加明细</NButton>
        <NFormItem label="备注"><NInput v-model:value="form.notes" type="textarea" /></NFormItem>
        <NSpace justify="end" style="width:100%">
          <NButton @click="showModal=false">取消</NButton>
          <NButton type="primary" @click="saveOrder">保存草稿</NButton>
        </NSpace>
      </NForm>
    </NModal>
  </div>
</template>
