<script setup lang="ts">
import { ref, onMounted, h } from 'vue';
import {
  NDataTable, NButton, NModal, NForm, NFormItem, NInput, NInputNumber,
  NSelect, NSpace, NTag, NDrawer, NDrawerContent, NPopconfirm, NDivider,
  NDescriptions, NDescriptionsItem, useMessage,
} from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '@/stores';

interface PaymentRecord {
  id: number; contract_id: number; amount: number;
  payment_date: string; payment_method: string; notes: string;
}
interface Contract {
  id: number; contract_code: string; name: string; direction: string;
  supplier_id: number | null; supplier_name: string;
  customer_id: number | null; customer_name: string;
  total_amount: number; paid_amount: number; unpaid_amount: number;
  sign_date: string; start_date: string; end_date: string;
  payment_method: string; payment_terms: string;
  brand_ids: string; fulfillment_status: string; status: string;
  file_path: string; notes: string;
  handled_by: number | null; handler_name: string;
  payments: PaymentRecord[];
}

const message = useMessage();
const appStore = useAppStore();
const loading = ref(false);
const contracts = ref<Contract[]>([]);
const directionFilter = ref<string | null>(null);
const statusFilter = ref<string | null>(null);
const searchKw = ref('');
const page = ref(1);
const total = ref(0);

// 下拉选项数据
const suppliers = ref<{ id: number; name: string }[]>([]);
const customers = ref<{ id: number; name: string }[]>([]);

// 创建/编辑表单
const showModal = ref(false);
const editingId = ref<number | null>(null);
const form = ref({
  name: '', direction: 'purchase', supplier_id: null as number | null,
  customer_id: null as number | null, total_amount: 0,
  sign_date: '', start_date: '', end_date: '',
  payment_method: '', payment_terms: '', brand_ids: '', file_path: '', notes: '',
});

// 详情抽屉
const showDrawer = ref(false);
const detail = ref<Contract | null>(null);

// 付款记录表单
const showPayModal = ref(false);
const payForm = ref({ amount: 0, payment_date: new Date().toISOString().slice(0, 10), payment_method: '', notes: '' });

const directionLabel = (d: string) => d === 'purchase' ? '采购合同' : '销售合同';

const statusMap: Record<string, { label: string; type: 'default' | 'info' | 'success' | 'warning' | 'error' }> = {
  draft: { label: '草稿', type: 'default' },
  active: { label: '生效中', type: 'info' },
  fulfilling: { label: '履行中', type: 'warning' },
  completed: { label: '已完成', type: 'success' },
  terminated: { label: '已终止', type: 'error' },
  expired: { label: '已过期', type: 'error' },
};
const fulfillMap: Record<string, string> = { pending: '未付款', partial: '部分付款', completed: '已付清' };

const columns: DataTableColumns<Contract> = [
  { title: '合同编号', key: 'contract_code', width: 160 },
  { title: '合同名称', key: 'name', ellipsis: { tooltip: true } },
  {
    title: '类型', key: 'direction', width: 90,
    render: r => h(NTag, { type: r.direction === 'purchase' ? 'info' : 'success', size: 'small' as const }, { default: () => directionLabel(r.direction) }),
  },
  {
    title: '对方', key: 'party', width: 120,
    render: r => r.direction === 'purchase' ? r.supplier_name : r.customer_name,
  },
  {
    title: '金额', key: 'total_amount', width: 110,
    render: r => h('span', { style: { fontWeight: 'bold' } }, `¥${r.total_amount.toLocaleString()}`),
  },
  {
    title: '已付/未付', key: 'paid', width: 130,
    render: r => h('span', {}, [
      h('span', { style: { color: '#18a058' } }, `¥${r.paid_amount.toLocaleString()}`),
      h('span', { style: { color: r.unpaid_amount > 0 ? '#d03050' : '#999', marginLeft: '4px' } }, `/ ¥${r.unpaid_amount.toLocaleString()}`),
    ]),
  },
  {
    title: '履行', key: 'fulfillment_status', width: 80,
    render: r => h(NTag, { type: r.fulfillment_status === 'completed' ? 'success' : r.fulfillment_status === 'partial' ? 'warning' : 'default', size: 'small' as const }, { default: () => fulfillMap[r.fulfillment_status] }),
  },
  {
    title: '状态', key: 'status', width: 80,
    render: r => h(NTag, { type: statusMap[r.status]?.type || 'default', size: 'small' as const }, { default: () => statusMap[r.status]?.label || r.status }),
  },
  { title: '签订日期', key: 'sign_date', width: 100 },
  {
    title: '操作', key: 'actions', width: 200, fixed: 'right' as const,
    render(row) {
      const btns: any[] = [
        h(NButton, { size: 'small', onClick: () => openDetail(row.id) }, { default: () => '详情' }),
      ];
      if (row.status === 'draft') {
        btns.push(h(NButton, { size: 'small', type: 'primary', style: { marginLeft: '4px' }, onClick: () => changeStatus(row.id, 'active') }, { default: () => '生效' }));
        btns.push(h(NButton, { size: 'small', style: { marginLeft: '4px' }, onClick: () => openEdit(row) }, { default: () => '编辑' }));
        btns.push(
          h(NPopconfirm, { onPositiveClick: () => deleteContract(row.id) }, {
            trigger: () => h(NButton, { size: 'small', type: 'error', style: { marginLeft: '4px' } }, { default: () => '删除' }),
            default: () => '确认删除此合同？',
          }),
        );
      } else if (row.status === 'active') {
        btns.push(h(NButton, { size: 'small', type: 'warning', style: { marginLeft: '4px' }, onClick: () => changeStatus(row.id, 'fulfilling') }, { default: () => '开始履行' }));
      } else if (row.status === 'fulfilling') {
        btns.push(h(NButton, { size: 'small', type: 'success', style: { marginLeft: '4px' }, onClick: () => changeStatus(row.id, 'completed') }, { default: () => '完成' }));
      }
      return h(NSpace, { size: 4 }, { default: () => btns });
    },
  },
];

// ── 数据加载 ──────────────────────────────────────────

async function loadOptions() {
  try {
    const [s, c] = await Promise.all([
      invoke<any[]>('list_suppliers'),
      invoke<any[]>('list_customers'),
    ]);
    suppliers.value = s.map(x => ({ id: x.id, name: x.name }));
    customers.value = c.map(x => ({ id: x.id, name: x.name }));
  } catch (_) {}
}

async function loadContracts() {
  loading.value = true;
  try {
    const res = await invoke<{ items: Contract[]; total: number }>('list_contracts', {
      direction: directionFilter.value || null,
      status: statusFilter.value || null,
      search: searchKw.value || null,
      page: page.value, pageSize: 20,
    });
    contracts.value = res.items;
    total.value = res.total;
  } catch (e: any) { message.error(typeof e === 'string' ? e : '加载失败'); }
  finally { loading.value = false; }
}

// ── 详情 ──────────────────────────────────────────────

async function openDetail(id: number) {
  try {
    detail.value = await invoke<Contract>('get_contract', { id });
    showDrawer.value = true;
  } catch (e: any) { message.error(typeof e === 'string' ? e : '获取详情失败'); }
}

// ── 创建/编辑 ─────────────────────────────────────────

function openCreate() {
  editingId.value = null;
  form.value = {
    name: '', direction: 'purchase', supplier_id: null, customer_id: null,
    total_amount: 0, sign_date: new Date().toISOString().slice(0, 10),
    start_date: '', end_date: '', payment_method: '', payment_terms: '',
    brand_ids: '', file_path: '', notes: '',
  };
  showModal.value = true;
}

function openEdit(row: Contract) {
  editingId.value = row.id;
  form.value = {
    name: row.name, direction: row.direction,
    supplier_id: row.supplier_id, customer_id: row.customer_id,
    total_amount: row.total_amount, sign_date: row.sign_date,
    start_date: row.start_date, end_date: row.end_date,
    payment_method: row.payment_method, payment_terms: row.payment_terms,
    brand_ids: row.brand_ids, file_path: row.file_path, notes: row.notes,
  };
  showModal.value = true;
}

async function saveContract() {
  if (!form.value.name) { message.warning('请输入合同名称'); return; }
  if (form.value.direction === 'purchase' && !form.value.supplier_id) { message.warning('采购合同必须选择供应商'); return; }
  if (form.value.direction === 'sales' && !form.value.customer_id) { message.warning('销售合同必须选择客户'); return; }
  try {
    if (editingId.value) {
      await invoke('update_contract', { id: editingId.value, input: form.value });
      message.success('合同已更新');
    } else {
      await invoke('create_contract', { input: { ...form.value, handled_by: appStore.userId } });
      message.success('合同已创建');
    }
    showModal.value = false; loadContracts();
  } catch (e: any) { message.error(typeof e === 'string' ? e : '保存失败'); }
}

// ── 状态变更 ──────────────────────────────────────────

async function changeStatus(id: number, newStatus: string) {
  try {
    await invoke('change_contract_status', { id, newStatus });
    message.success('状态已更新');
    loadContracts();
    if (showDrawer.value) openDetail(id);
  } catch (e: any) { message.error(typeof e === 'string' ? e : '操作失败'); }
}

// ── 软删除 ────────────────────────────────────────────

async function deleteContract(id: number) {
  try { await invoke('delete_contract', { id }); message.success('已删除'); loadContracts(); }
  catch (e: any) { message.error(typeof e === 'string' ? e : '删除失败'); }
}

// ── 收付款 ────────────────────────────────────────────

function openPayModal() { payForm.value = { amount: 0, payment_date: new Date().toISOString().slice(0, 10), payment_method: '', notes: '' }; showPayModal.value = true; }

async function addPayment() {
  if (payForm.value.amount <= 0) { message.warning('金额必须大于0'); return; }
  if (!payForm.value.payment_date) { message.warning('请选择日期'); return; }
  try {
    await invoke('add_payment_record', { input: { ...payForm.value, contract_id: detail.value!.id } });
    message.success('付款记录已添加');
    showPayModal.value = false;
    openDetail(detail.value!.id);
  } catch (e: any) { message.error(typeof e === 'string' ? e : '添加失败'); }
}

async function deletePayment(pid: number) {
  try {
    await invoke('delete_payment_record', { id: pid });
    message.success('已删除');
    openDetail(detail.value!.id);
  } catch (e: any) { message.error(typeof e === 'string' ? e : '删除失败'); }
}

onMounted(() => { loadOptions(); loadContracts(); });
</script>

<template>
  <div>
    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:16px">
      <NSpace>
        <NSelect v-model:value="directionFilter" :options="[{label:'全部类型',value:'all'},{label:'采购合同',value:'purchase'},{label:'销售合同',value:'sales'}]" placeholder="合同类型" clearable style="width:130px" @update:value="page=1;loadContracts()" />
        <NSelect v-model:value="statusFilter" :options="[{label:'全部状态',value:'all'},{label:'草稿',value:'draft'},{label:'生效中',value:'active'},{label:'履行中',value:'fulfilling'},{label:'已完成',value:'completed'},{label:'已终止',value:'terminated'},{label:'已过期',value:'expired'}]" placeholder="状态" clearable style="width:120px" @update:value="page=1;loadContracts()" />
        <NInput v-model:value="searchKw" placeholder="搜索编号/名称" clearable style="width:180px" @keyup.enter="page=1;loadContracts()" />
        <NButton @click="page=1;loadContracts()">刷新</NButton>
      </NSpace>
      <NButton type="primary" @click="openCreate">新建合同</NButton>
    </div>

    <NDataTable
      :columns="columns" :data="contracts" :loading="loading"
      :pagination="{ page, pageSize: 20, itemCount: total, onChange: (p: number) => { page = p; loadContracts(); } }"
      :row-key="(r: Contract) => r.id" scroll-x="xMax"
    />

    <!-- 创建/编辑合同 Modal -->
    <NModal v-model:show="showModal" :title="editingId ? '编辑合同' : '新建合同'" style="width:720px">
      <NForm style="padding:24px" label-placement="left" label-width="80">
        <NSpace>
          <NFormItem label="合同名称" required><NInput v-model:value="form.name" style="width:260px" placeholder="输入合同名称" /></NFormItem>
          <NFormItem label="合同类型" required>
            <NSelect v-model:value="form.direction" :options="[{label:'采购合同',value:'purchase'},{label:'销售合同',value:'sales'}]" style="width:140px" />
          </NFormItem>
        </NSpace>
        <NSpace>
          <NFormItem v-if="form.direction==='purchase'" label="供应商" required>
            <NSelect v-model:value="form.supplier_id" :options="suppliers.map(s=>({label:s.name,value:s.id}))" style="width:240px" filterable />
          </NFormItem>
          <NFormItem v-if="form.direction==='sales'" label="客户" required>
            <NSelect v-model:value="form.customer_id" :options="customers.map(c=>({label:c.name,value:c.id}))" style="width:240px" filterable />
          </NFormItem>
          <NFormItem label="合同金额" required><NInputNumber v-model:value="form.total_amount" :min="0" style="width:160px"><template #prefix>¥</template></NInputNumber></NFormItem>
        </NSpace>
        <NSpace>
          <NFormItem label="签订日期"><NInput v-model:value="form.sign_date" placeholder="YYYY-MM-DD" style="width:160px" /></NFormItem>
          <NFormItem label="生效日期"><NInput v-model:value="form.start_date" placeholder="YYYY-MM-DD" style="width:160px" /></NFormItem>
          <NFormItem label="到期日期"><NInput v-model:value="form.end_date" placeholder="YYYY-MM-DD" style="width:160px" /></NFormItem>
        </NSpace>
        <NSpace>
          <NFormItem label="付款方式"><NInput v-model:value="form.payment_method" placeholder="如：银行转账" style="width:200px" /></NFormItem>
          <NFormItem label="付款条款"><NInput v-model:value="form.payment_terms" placeholder="如：货到30天内付清" style="width:280px" /></NFormItem>
        </NSpace>
        <NFormItem label="关联品牌"><NInput v-model:value="form.brand_ids" placeholder='JSON数组，如 ["品牌A","品牌B"]' /></NFormItem>
        <NFormItem label="附件路径"><NInput v-model:value="form.file_path" placeholder="合同文件路径" /></NFormItem>
        <NFormItem label="备注"><NInput v-model:value="form.notes" type="textarea" :rows="2" /></NFormItem>
        <NSpace justify="end" style="width:100%">
          <NButton @click="showModal=false">取消</NButton>
          <NButton type="primary" @click="saveContract">{{ editingId ? '保存' : '创建' }}</NButton>
        </NSpace>
      </NForm>
    </NModal>

    <!-- 详情 Drawer -->
    <NDrawer v-model:show="showDrawer" :width="560">
      <NDrawerContent v-if="detail" title="合同详情" closable>
        <template #header>
          <span style="font-size:16px;font-weight:bold">{{ detail.contract_code }}</span>
          <NTag :type="statusMap[detail.status]?.type || 'default'" size="small" style="margin-left:12px">{{ statusMap[detail.status]?.label }}</NTag>
        </template>

        <NDescriptions label-placement="left" :column="2" bordered size="small">
          <NDescriptionsItem label="合同名称" :span="2">{{ detail.name }}</NDescriptionsItem>
          <NDescriptionsItem label="合同类型">{{ directionLabel(detail.direction) }}</NDescriptionsItem>
          <NDescriptionsItem label="金额">¥{{ detail.total_amount.toLocaleString() }}</NDescriptionsItem>
          <NDescriptionsItem :label="detail.direction==='purchase'?'供应商':'客户'">
            {{ detail.direction === 'purchase' ? detail.supplier_name : detail.customer_name }}
          </NDescriptionsItem>
          <NDescriptionsItem label="履行状态">
            <NTag :type="detail.fulfillment_status==='completed'?'success':detail.fulfillment_status==='partial'?'warning':'default'" size="small">{{ fulfillMap[detail.fulfillment_status] }}</NTag>
          </NDescriptionsItem>
          <NDescriptionsItem label="签订日期">{{ detail.sign_date || '-' }}</NDescriptionsItem>
          <NDescriptionsItem label="生效日期">{{ detail.start_date || '-' }}</NDescriptionsItem>
          <NDescriptionsItem label="到期日期">{{ detail.end_date || '-' }}</NDescriptionsItem>
          <NDescriptionsItem label="已付金额"><span style="color:#18a058;font-weight:bold">¥{{ detail.paid_amount.toLocaleString() }}</span></NDescriptionsItem>
          <NDescriptionsItem label="未付金额"><span :style="{color:detail.unpaid_amount>0?'#d03050':'#999',fontWeight:'bold'}">¥{{ detail.unpaid_amount.toLocaleString() }}</span></NDescriptionsItem>
          <NDescriptionsItem label="付款方式">{{ detail.payment_method || '-' }}</NDescriptionsItem>
          <NDescriptionsItem label="付款条款">{{ detail.payment_terms || '-' }}</NDescriptionsItem>
          <NDescriptionsItem label="经手人">{{ detail.handler_name || '-' }}</NDescriptionsItem>
          <NDescriptionsItem label="备注" :span="2">{{ detail.notes || '-' }}</NDescriptionsItem>
        </NDescriptions>

        <!-- 状态操作 -->
        <NDivider />
        <div style="display:flex; gap:8px; flex-wrap:wrap">
          <NButton v-if="detail.status==='draft'" size="small" type="primary" @click="changeStatus(detail.id,'active')">生效</NButton>
          <NButton v-if="detail.status==='active'" size="small" type="warning" @click="changeStatus(detail.id,'fulfilling')">开始履行</NButton>
          <NButton v-if="detail.status==='fulfilling'" size="small" type="success" @click="changeStatus(detail.id,'completed')">标记完成</NButton>
          <NButton v-if="['active','fulfilling'].includes(detail.status)" size="small" type="error" @click="changeStatus(detail.id,'terminated')">终止合同</NButton>
        </div>

        <!-- 收付款记录 -->
        <NDivider>收付款记录</NDivider>
        <div style="margin-bottom:8px">
          <NButton size="small" type="primary" @click="openPayModal">+ 添加收付款</NButton>
        </div>
        <div v-if="detail.payments.length === 0" style="color:#999; text-align:center; padding:20px">暂无收付款记录</div>
        <div v-for="p in detail.payments" :key="p.id" style="padding:8px 0; border-bottom:1px solid #f0f0f0; display:flex; justify-content:space-between; align-items:center">
          <div>
            <span style="color:#18a058;font-weight:bold;font-size:15px">+¥{{ p.amount.toLocaleString() }}</span>
            <span style="margin-left:8px;color:#666">{{ p.payment_date }}</span>
            <span v-if="p.payment_method" style="margin-left:8px;color:#999">{{ p.payment_method }}</span>
            <div v-if="p.notes" style="color:#999;font-size:12px;margin-top:2px">{{ p.notes }}</div>
          </div>
          <NPopconfirm @positive-click="deletePayment(p.id)">
            <template #trigger><NButton size="tiny" type="error" text>删除</NButton></template>
            确认删除此付款记录？
          </NPopconfirm>
        </div>

        <template #footer>
          <NButton @click="showDrawer=false">关闭</NButton>
        </template>
      </NDrawerContent>
    </NDrawer>

    <!-- 添加付款 Modal -->
    <NModal v-model:show="showPayModal" title="添加收付款记录" style="width:420px">
      <NForm style="padding:24px" label-placement="left" label-width="80">
        <NFormItem label="金额" required><NInputNumber v-model:value="payForm.amount" :min="0.01" style="width:100%"><template #prefix>¥</template></NInputNumber></NFormItem>
        <NFormItem label="日期" required><NInput v-model:value="payForm.payment_date" placeholder="YYYY-MM-DD" /></NFormItem>
        <NFormItem label="付款方式"><NInput v-model:value="payForm.payment_method" placeholder="如：银行转账" /></NFormItem>
        <NFormItem label="备注"><NInput v-model:value="payForm.notes" type="textarea" /></NFormItem>
        <NSpace justify="end" style="width:100%">
          <NButton @click="showPayModal=false">取消</NButton>
          <NButton type="primary" @click="addPayment">确认</NButton>
        </NSpace>
      </NForm>
    </NModal>
  </div>
</template>
