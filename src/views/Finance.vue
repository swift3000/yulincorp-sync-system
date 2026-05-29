<script setup lang="ts">
import { ref, onMounted, h, computed } from 'vue';
import {
  NDataTable, NButton, NModal, NForm, NFormItem, NInput, NInputNumber,
  NSelect, NSpace, NTag, NPopconfirm, useMessage, NTabs, NTabPane,
  NCard, NStatistic, NGrid, NGi,
} from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '@/stores';

interface ARItem { customer_id: number; customer_name: string; total_sales: number; total_received: number; balance: number; last_transaction_date: string; }
interface APItem { supplier_id: number; supplier_name: string; total_purchases: number; total_paid: number; balance: number; last_transaction_date: string; }
interface ProfitSummary { total_revenue: number; total_cost: number; gross_profit: number; gross_margin: number; period_start: string; period_end: string; }
interface PaymentTransaction { id: number; transaction_type: string; reference_type: string; reference_id: number | null; party_type: string; party_id: number; party_name: string; amount: number; payment_method: string; transaction_date: string; notes: string; }

const message = useMessage();
const appStore = useAppStore();
const isBoss = computed(() => appStore.userRole === 'boss' || appStore.userRole === 'admin');

const receiptTotal = computed(() => transactions.value.filter(t => t.transaction_type === 'receipt').reduce((s, t) => s + t.amount, 0));
const paymentTotal = computed(() => transactions.value.filter(t => t.transaction_type === 'payment').reduce((s, t) => s + t.amount, 0));

// AR data
const arItems = ref<ARItem[]>([]);
const arLoading = ref(false);

// AP data
const apItems = ref<APItem[]>([]);
const apLoading = ref(false);

// Profit
const profit = ref<ProfitSummary | null>(null);
const profitStart = ref(new Date(new Date().getFullYear(), 0, 1).toISOString().slice(0, 10));
const profitEnd = ref(new Date().toISOString().slice(0, 10));

// Transactions
const transactions = ref<PaymentTransaction[]>([]);
const txLoading = ref(false);
const txTotal = ref(0); const txAmount = ref(0);
const txTypeFilter = ref<string | null>(null);
const txPage = ref(1);

// Forms
const showTxModal = ref(false);
const txForm = ref({
  transaction_type: 'receipt', reference_type: 'manual', reference_id: null as number | null,
  party_type: 'customer', party_id: null as number | null,
  amount: 0, payment_method: '', transaction_date: new Date().toISOString().slice(0, 10), notes: '',
});

const suppliers = ref<{ id: number; name: string }[]>([]);
const customers = ref<{ id: number; name: string }[]>([]);

// ── 表格列定义 ────────────────────────────────────────

const arColumns: DataTableColumns<ARItem> = [
  { title: '客户', key: 'customer_name' },
  { title: '销售总额', key: 'total_sales', render: r => `¥${r.total_sales.toLocaleString()}` },
  { title: '已收金额', key: 'total_received', render: r => h('span', { style: { color: '#18a058' } }, `¥${r.total_received.toLocaleString()}`) },
  {
    title: '应收余额', key: 'balance',
    render: r => h('span', { style: { color: r.balance > 0 ? '#d03050' : '#18a058', fontWeight: 'bold' } }, `¥${r.balance.toLocaleString()}`),
  },
  { title: '最近交易', key: 'last_transaction_date', width: 110 },
  {
    title: '操作', key: 'actions', width: 100,
    render(row) {
      return h(NButton, { size: 'small', type: 'primary', onClick: () => openReceipt(row) }, { default: () => '收款' });
    },
  },
];

const apColumns: DataTableColumns<APItem> = [
  { title: '供应商', key: 'supplier_name' },
  { title: '采购总额', key: 'total_purchases', render: r => `¥${r.total_purchases.toLocaleString()}` },
  { title: '已付金额', key: 'total_paid', render: r => h('span', { style: { color: '#18a058' } }, `¥${r.total_paid.toLocaleString()}`) },
  {
    title: '应付余额', key: 'balance',
    render: r => h('span', { style: { color: r.balance > 0 ? '#d03050' : '#18a058', fontWeight: 'bold' } }, `¥${r.balance.toLocaleString()}`),
  },
  { title: '最近交易', key: 'last_transaction_date', width: 110 },
  {
    title: '操作', key: 'actions', width: 100,
    render(row) {
      return h(NButton, { size: 'small', type: 'primary', onClick: () => openPayment(row) }, { default: () => '付款' });
    },
  },
];

const txColumns: DataTableColumns<PaymentTransaction> = [
  {
    title: '类型', key: 'transaction_type', width: 70,
    render: r => h(NTag, { type: r.transaction_type === 'receipt' ? 'success' : 'warning', size: 'small' as const }, { default: () => r.transaction_type === 'receipt' ? '收款' : '付款' }),
  },
  { title: '对方', key: 'party_name', width: 120 },
  { title: '金额', key: 'amount', width: 110, render: r => h('span', { style: { color: r.transaction_type === 'receipt' ? '#18a058' : '#d03050', fontWeight: 'bold' } }, `¥${r.amount.toLocaleString()}`) },
  { title: '方式', key: 'payment_method', width: 90 },
  { title: '日期', key: 'transaction_date', width: 100 },
  { title: '关联', key: 'reference_type', width: 80 },
  { title: '备注', key: 'notes', ellipsis: { tooltip: true } },
  {
    title: '操作', key: 'actions', width: 80,
    render: r => h(NPopconfirm, { onPositiveClick: () => deleteTransaction(r.id) }, {
      trigger: () => h(NButton, { size: 'tiny', type: 'error', text: true }, { default: () => '删除' }),
      default: () => '确认删除此记录？',
    }),
  },
];

// ── 加载 ──────────────────────────────────────────────

async function loadOptions() {
  try { suppliers.value = (await invoke<any[]>('list_suppliers')).map(s => ({ id: s.id, name: s.name })); } catch (_) {}
  try { customers.value = (await invoke<any[]>('list_customers')).map(c => ({ id: c.id, name: c.name })); } catch (_) {}
}
async function loadAR() { arLoading.value = true; try { arItems.value = await invoke<ARItem[]>('list_ar'); } catch (e: any) { message.error(typeof e === 'string' ? e : '加载失败'); } finally { arLoading.value = false; } }
async function loadAP() { apLoading.value = true; try { apItems.value = await invoke<APItem[]>('list_ap'); } catch (e: any) { message.error(typeof e === 'string' ? e : '加载失败'); } finally { apLoading.value = false; } }
async function loadProfit() {
  try { profit.value = await invoke<ProfitSummary>('get_profit_summary', { startDate: profitStart.value, endDate: profitEnd.value }); }
  catch (e: any) { message.error(typeof e === 'string' ? e : '查询利润失败'); }
}
async function loadTransactions() {
  txLoading.value = true;
  try {
    const res = await invoke<{ items: PaymentTransaction[]; total_count: number; total_amount: number }>('list_transactions', {
      transactionType: txTypeFilter.value || null, page: txPage.value, pageSize: 30,
    });
    transactions.value = res.items; txTotal.value = res.total_count; txAmount.value = res.total_amount;
  } catch (e: any) { message.error(typeof e === 'string' ? e : '加载失败'); }
  finally { txLoading.value = false; }
}

// ── 收款/付款 ─────────────────────────────────────────

function openReceipt(row: ARItem) {
  txForm.value = {
    transaction_type: 'receipt', reference_type: 'manual', reference_id: null,
    party_type: 'customer', party_id: row.customer_id,
    amount: row.balance, payment_method: '', transaction_date: new Date().toISOString().slice(0, 10), notes: '',
  };
  showTxModal.value = true;
}
function openPayment(row: APItem) {
  txForm.value = {
    transaction_type: 'payment', reference_type: 'manual', reference_id: null,
    party_type: 'supplier', party_id: row.supplier_id,
    amount: row.balance, payment_method: '', transaction_date: new Date().toISOString().slice(0, 10), notes: '',
  };
  showTxModal.value = true;
}
async function saveTransaction() {
  if (!txForm.value.party_id) { message.warning('请选择对方'); return; }
  if (txForm.value.amount <= 0) { message.warning('金额必须大于0'); return; }
  try {
    await invoke('record_transaction', { input: txForm.value });
    message.success('记录成功');
    showTxModal.value = false;
    loadAR(); loadAP(); loadTransactions();
  } catch (e: any) { message.error(typeof e === 'string' ? e : '记录失败'); }
}
async function deleteTransaction(id: number) {
  try { await invoke('delete_transaction', { id }); message.success('已删除'); loadAR(); loadAP(); loadTransactions(); }
  catch (e: any) { message.error(typeof e === 'string' ? e : '删除失败'); }
}

onMounted(() => { loadOptions(); });
</script>

<template>
  <div>
    <h2 style="margin-bottom:16px">财务报表</h2>

    <NTabs type="line" animated @update:value="(v: string) => {
      if (v==='ar') loadAR();
      else if (v==='ap') loadAP();
      else if (v==='profit' && isBoss) loadProfit();
      else if (v==='tx') loadTransactions();
    }">
      <NTabPane name="ar" tab="应收账款">
        <NDataTable :columns="arColumns" :data="arItems" :loading="arLoading" :row-key="(r: ARItem) => r.customer_id" />
        <div style="margin-top:8px; color:#999; font-size:12px">基于销售单（非草稿）与实际收款记录自动计算</div>
      </NTabPane>

      <NTabPane name="ap" tab="应付账款">
        <NDataTable :columns="apColumns" :data="apItems" :loading="apLoading" :row-key="(r: APItem) => r.supplier_id" />
        <div style="margin-top:8px; color:#999; font-size:12px">基于采购单（非草稿）与实际付款记录自动计算</div>
      </NTabPane>

      <NTabPane v-if="isBoss" name="profit" tab="利润分析">
        <NSpace style="margin-bottom:16px">
          <NInput v-model:value="profitStart" placeholder="起始日期" style="width:140px" />
          <span>至</span>
          <NInput v-model:value="profitEnd" placeholder="截止日期" style="width:140px" />
          <NButton type="primary" @click="loadProfit">查询</NButton>
        </NSpace>
        <NGrid v-if="profit" :cols="4" :x-gap="16">
          <NGi><NCard><NStatistic label="销售收入" :value="profit.total_revenue"><template #prefix>¥</template></NStatistic></NCard></NGi>
          <NGi><NCard><NStatistic label="销售成本" :value="profit.total_cost"><template #prefix>¥</template></NStatistic></NCard></NGi>
          <NGi><NCard><NStatistic label="毛利润" :value="profit.gross_profit"><template #prefix>¥</template></NStatistic></NCard></NGi>
          <NGi><NCard><NStatistic label="毛利率" :value="profit.gross_margin"><template #suffix>%</template></NStatistic></NCard></NGi>
        </NGrid>
        <div v-if="!isBoss" style="color:#999;text-align:center;padding:40px">利润数据仅对老板/管理员可见</div>
      </NTabPane>

      <NTabPane name="tx" tab="收付款记录">
        <div style="margin-bottom:12px; display:flex; justify-content:space-between; align-items:center">
          <NSpace>
            <NSelect v-model:value="txTypeFilter" :options="[{label:'全部',value:'all'},{label:'收款',value:'receipt'},{label:'付款',value:'payment'}]" placeholder="类型" clearable style="width:100px" @update:value="txPage=1;loadTransactions()" />
            <NButton @click="txPage=1;loadTransactions()">刷新</NButton>
          </NSpace>
          <div>
            <span style="color:#999; margin-right:16px">共 {{ txTotal }} 条</span>
            <span style="font-weight:bold">
              收款 <span style="color:#18a058">¥{{ receiptTotal.toLocaleString() }}</span>
              &nbsp;付款 <span style="color:#d03050">¥{{ paymentTotal.toLocaleString() }}</span>
            </span>
          </div>
        </div>
        <NDataTable :columns="txColumns" :data="transactions" :loading="txLoading" :row-key="(r: PaymentTransaction) => r.id"
          :pagination="{ page: txPage, pageSize: 30, onChange: (p: number) => { txPage = p; loadTransactions(); } }" />
      </NTabPane>
    </NTabs>

    <!-- 收付款记录 Modal -->
    <NModal v-model:show="showTxModal" title="记录收付款" style="width:450px">
      <NForm style="padding:24px" label-placement="left" label-width="80">
        <NFormItem label="类型" required>
          <NSelect v-model:value="txForm.transaction_type" :options="[{label:'收款（客户付款）',value:'receipt'},{label:'付款（向供应商付款）',value:'payment'}]" />
        </NFormItem>
        <NFormItem v-if="txForm.transaction_type==='receipt'" label="客户" required>
          <NSelect v-model:value="txForm.party_id" :options="customers.map(c=>({label:c.name,value:c.id}))" filterable style="width:100%" @update:value="txForm.party_type='customer'" />
        </NFormItem>
        <NFormItem v-else label="供应商" required>
          <NSelect v-model:value="txForm.party_id" :options="suppliers.map(s=>({label:s.name,value:s.id}))" filterable style="width:100%" @update:value="txForm.party_type='supplier'" />
        </NFormItem>
        <NFormItem label="金额" required><NInputNumber v-model:value="txForm.amount" :min="0.01" style="width:100%"><template #prefix>¥</template></NInputNumber></NFormItem>
        <NFormItem label="日期"><NInput v-model:value="txForm.transaction_date" placeholder="YYYY-MM-DD" /></NFormItem>
        <NFormItem label="方式"><NInput v-model:value="txForm.payment_method" placeholder="如：银行转账" /></NFormItem>
        <NFormItem label="备注"><NInput v-model:value="txForm.notes" type="textarea" /></NFormItem>
        <NSpace justify="end" style="width:100%">
          <NButton @click="showTxModal=false">取消</NButton>
          <NButton type="primary" @click="saveTransaction">确认</NButton>
        </NSpace>
      </NForm>
    </NModal>
  </div>
</template>
