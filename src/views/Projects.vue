<script setup lang="ts">
import { ref, onMounted, h, computed } from 'vue';
import {
  NDataTable, NButton, NModal, NForm, NFormItem, NInput, NInputNumber,
  NSelect, NSpace, NTag, NDrawer, NDrawerContent, NPopconfirm, NDivider,
  NDescriptions, NDescriptionsItem, NProgress, useMessage, NTimeline, NTimelineItem,
} from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '@/stores';

interface ProjectPhase {
  id: number; project_id: number; phase_name: string; status: string;
  start_date: string; end_date: string; notes: string; sort_order: number;
}
interface ProjectDocument {
  id: number; project_id: number; phase_id: number | null;
  doc_name: string; doc_type: string; file_path: string; file_size: number;
}
interface Project {
  id: number; project_code: string; name: string;
  customer_id: number | null; customer_name: string;
  status: string; budget: number; actual_cost: number;
  start_date: string; end_date: string;
  handled_by: number | null; handler_name: string; notes: string;
  phases: ProjectPhase[]; documents: ProjectDocument[];
}

const message = useMessage();
const appStore = useAppStore();
const loading = ref(false);
const projects = ref<Project[]>([]);
const statusFilter = ref<string | null>(null);
const searchKw = ref('');
const page = ref(1); const total = ref(0);

const customers = ref<{ id: number; name: string }[]>([]);

// 表单
const showModal = ref(false);
const editingId = ref<number | null>(null);
const form = ref({
  name: '', customer_id: null as number | null, budget: 0,
  start_date: '', end_date: '', notes: '', handled_by: null as number | null,
});

// 详情
const showDrawer = ref(false);
const detail = ref<Project | null>(null);

const statusMap: Record<string, { label: string; type: 'default'|'info'|'success'|'warning'|'error' }> = {
  bidding: { label: '招投标', type: 'info' },
  design: { label: '方案设计', type: 'info' },
  execution: { label: '采购执行', type: 'warning' },
  delivery: { label: '验收交付', type: 'warning' },
  maintenance: { label: '维保服务', type: 'success' },
  completed: { label: '已完成', type: 'success' },
  cancelled: { label: '已取消', type: 'error' },
};
const phaseStatusMap: Record<string, { label: string; type: string }> = {
  pending: { label: '待开始', type: 'default' },
  in_progress: { label: '进行中', type: 'info' },
  completed: { label: '已完成', type: 'success' },
};

// 进度计算
const progressPercent = computed(() => {
  if (!detail.value || detail.value.phases.length === 0) return 0;
  const done = detail.value.phases.filter(p => p.status === 'completed').length;
  return Math.round((done / detail.value.phases.length) * 100);
});

const columns: DataTableColumns<Project> = [
  { title: '编号', key: 'project_code', width: 130 },
  { title: '项目名称', key: 'name', ellipsis: { tooltip: true } },
  { title: '客户', key: 'customer_name', width: 120 },
  {
    title: '状态', key: 'status', width: 90,
    render: r => h(NTag, { type: statusMap[r.status]?.type || 'default', size: 'small' as const }, { default: () => statusMap[r.status]?.label || r.status }),
  },
  {
    title: '预算', key: 'budget', width: 100,
    render: r => `¥${r.budget.toLocaleString()}`,
  },
  {
    title: '实际成本', key: 'actual_cost', width: 100,
    render: r => h('span', { style: { color: r.actual_cost > r.budget ? '#d03050' : '#18a058' } }, `¥${r.actual_cost.toLocaleString()}`),
  },
  { title: '开始', key: 'start_date', width: 100 },
  { title: '结束', key: 'end_date', width: 100 },
  {
    title: '操作', key: 'actions', width: 120, fixed: 'right' as const,
    render(row) {
      return h(NSpace, { size: 4 }, { default: () => [
        h(NButton, { size: 'small', onClick: () => openDetail(row.id) }, { default: () => '详情' }),
        row.status !== 'completed' && row.status !== 'cancelled'
          ? h(NButton, { size: 'small', type: 'primary', onClick: () => advanceStatus(row) }, { default: () => '推进' })
          : null,
        h(NPopconfirm, { onPositiveClick: () => deleteProject(row.id) }, {
          trigger: () => h(NButton, { size: 'small', type: 'error' }, { default: () => '删除' }),
          default: () => '确认删除此项目？',
        }),
      ]});
    },
  },
];

// ── 加载 ──────────────────────────────────────────────

async function loadOptions() {
  try { customers.value = (await invoke<any[]>('list_customers')).map(c => ({ id: c.id, name: c.name })); } catch (_) {}
}
async function loadProjects() {
  loading.value = true;
  try {
    const res = await invoke<{ items: Project[]; total: number }>('list_projects', {
      status: statusFilter.value || null, search: searchKw.value || null,
      page: page.value, pageSize: 20,
    });
    projects.value = res.items; total.value = res.total;
  } catch (e: any) { message.error(typeof e === 'string' ? e : '加载失败'); }
  finally { loading.value = false; }
}

// ── 详情 ──────────────────────────────────────────────

async function openDetail(id: number) {
  try { detail.value = await invoke<Project>('get_project', { id }); showDrawer.value = true; }
  catch (e: any) { message.error(typeof e === 'string' ? e : '获取详情失败'); }
}

// ── 创建/编辑 ─────────────────────────────────────────

function openCreate() {
  editingId.value = null;
  form.value = { name: '', customer_id: null, budget: 0,
    start_date: new Date().toISOString().slice(0, 10), end_date: '', notes: '', handled_by: appStore.userId };
  showModal.value = true;
}
async function saveProject() {
  if (!form.value.name) { message.warning('请输入项目名称'); return; }
  try {
    if (editingId.value) {
      await invoke('update_project', { id: editingId.value, input: form.value });
      message.success('项目已更新');
    } else {
      await invoke('create_project', { input: form.value });
      message.success('项目已创建');
    }
    showModal.value = false; loadProjects();
  } catch (e: any) { message.error(typeof e === 'string' ? e : '保存失败'); }
}

// ── 状态推进 ──────────────────────────────────────────

const nextStatus: Record<string, string> = {
  bidding: 'design', design: 'execution', execution: 'delivery',
  delivery: 'completed', maintenance: 'completed',
};

async function advanceStatus(row: Project) {
  const next = nextStatus[row.status];
  if (!next) { message.warning('当前状态无法推进'); return; }
  try {
    await invoke('change_project_status', { id: row.id, newStatus: next });
    message.success('状态已推进');
    loadProjects();
  } catch (e: any) { message.error(typeof e === 'string' ? e : '操作失败'); }
}

async function deleteProject(id: number) {
  try { await invoke('delete_project', { id }); message.success('已删除'); loadProjects(); }
  catch (e: any) { message.error(typeof e === 'string' ? e : '删除失败'); }
}

// ── 阶段更新 ──────────────────────────────────────────

const editingPhase = ref<{ id: number; status: string; start_date: string; end_date: string; notes: string } | null>(null);
const showPhaseModal = ref(false);

function editPhase(phase: ProjectPhase) {
  editingPhase.value = { id: phase.id, status: phase.status, start_date: phase.start_date, end_date: phase.end_date, notes: phase.notes };
  showPhaseModal.value = true;
}
async function savePhase() {
  if (!editingPhase.value) return;
  try {
    await invoke('update_project_phase', { phaseId: editingPhase.value.id, input: editingPhase.value });
    message.success('阶段已更新');
    showPhaseModal.value = false;
    openDetail(detail.value!.id);
  } catch (e: any) { message.error(typeof e === 'string' ? e : '更新失败'); }
}

onMounted(() => { loadOptions(); loadProjects(); });
</script>

<template>
  <div>
    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:16px">
      <NSpace>
        <NSelect v-model:value="statusFilter" :options="[{label:'全部',value:'all'},{label:'招投标',value:'bidding'},{label:'方案设计',value:'design'},{label:'采购执行',value:'execution'},{label:'验收交付',value:'delivery'},{label:'维保',value:'maintenance'},{label:'已完成',value:'completed'}]" placeholder="状态" clearable style="width:120px" @update:value="page=1;loadProjects()" />
        <NInput v-model:value="searchKw" placeholder="搜索编号/名称" clearable style="width:180px" @keyup.enter="page=1;loadProjects()" />
        <NButton @click="page=1;loadProjects()">刷新</NButton>
      </NSpace>
      <NButton type="primary" @click="openCreate">新建项目</NButton>
    </div>

    <NDataTable
      :columns="columns" :data="projects" :loading="loading"
      :pagination="{ page, pageSize: 20, itemCount: total, onChange: (p: number) => { page = p; loadProjects(); } }"
      :row-key="(r: Project) => r.id" scroll-x="xMax"
    />

    <!-- 创建/编辑 Modal -->
    <NModal v-model:show="showModal" :title="editingId ? '编辑项目' : '新建项目'" style="width:600px">
      <NForm style="padding:24px" label-placement="left" label-width="80">
        <NFormItem label="项目名称" required><NInput v-model:value="form.name" style="width:100%" /></NFormItem>
        <NSpace>
          <NFormItem label="客户">
            <NSelect v-model:value="form.customer_id" :options="customers.map(c=>({label:c.name,value:c.id}))" style="width:220px" filterable clearable />
          </NFormItem>
          <NFormItem label="预算"><NInputNumber v-model:value="form.budget" :min="0" style="width:160px"><template #prefix>¥</template></NInputNumber></NFormItem>
        </NSpace>
        <NSpace>
          <NFormItem label="开始日期"><NInput v-model:value="form.start_date" placeholder="YYYY-MM-DD" style="width:180px" /></NFormItem>
          <NFormItem label="结束日期"><NInput v-model:value="form.end_date" placeholder="YYYY-MM-DD" style="width:180px" /></NFormItem>
        </NSpace>
        <NFormItem label="备注"><NInput v-model:value="form.notes" type="textarea" :rows="2" /></NFormItem>
        <NSpace justify="end" style="width:100%">
          <NButton @click="showModal=false">取消</NButton>
          <NButton type="primary" @click="saveProject">{{ editingId ? '保存' : '创建' }}</NButton>
        </NSpace>
      </NForm>
    </NModal>

    <!-- 详情 Drawer -->
    <NDrawer v-model:show="showDrawer" :width="600">
      <NDrawerContent v-if="detail" title="项目详情" closable>
        <template #header>
          <span style="font-size:16px;font-weight:bold">{{ detail.project_code }}</span>
          <NTag :type="statusMap[detail.status]?.type||'default'" size="small" style="margin-left:12px">{{ statusMap[detail.status]?.label }}</NTag>
        </template>

        <NDescriptions label-placement="left" :column="2" bordered size="small">
          <NDescriptionsItem label="项目名称" :span="2">{{ detail.name }}</NDescriptionsItem>
          <NDescriptionsItem label="客户">{{ detail.customer_name || '-' }}</NDescriptionsItem>
          <NDescriptionsItem label="负责人">{{ detail.handler_name || '-' }}</NDescriptionsItem>
          <NDescriptionsItem label="预算">¥{{ detail.budget.toLocaleString() }}</NDescriptionsItem>
          <NDescriptionsItem label="实际成本"><span :style="{color:detail.actual_cost>detail.budget?'#d03050':'#18a058',fontWeight:'bold'}">¥{{ detail.actual_cost.toLocaleString() }}</span></NDescriptionsItem>
          <NDescriptionsItem label="开始">{{ detail.start_date || '-' }}</NDescriptionsItem>
          <NDescriptionsItem label="结束">{{ detail.end_date || '-' }}</NDescriptionsItem>
          <NDescriptionsItem label="备注" :span="2">{{ detail.notes || '-' }}</NDescriptionsItem>
        </NDescriptions>

        <!-- 进度 -->
        <NDivider />
        <div style="margin-bottom:8px; font-weight:bold">总体进度</div>
        <NProgress :percentage="progressPercent" :height="20" :border-radius="4" :color="progressPercent===100?'#18a058':'#2080f0'" />

        <!-- 阶段时间线 -->
        <NDivider>项目阶段</NDivider>
        <NTimeline>
          <NTimelineItem v-for="phase in detail.phases" :key="phase.id" :type="phase.status==='completed'?'success':phase.status==='in_progress'?'info':'default'">
            <template #header>
              <div style="display:flex; align-items:center; gap:8px">
                <span style="font-weight:bold">{{ phase.phase_name }}</span>
                <NTag :type="phaseStatusMap[phase.status]?.type as any" size="tiny">{{ phaseStatusMap[phase.status]?.label }}</NTag>
              </div>
              <div v-if="phase.start_date" style="font-size:12px;color:#999">{{ phase.start_date }} ~ {{ phase.end_date || '至今' }}</div>
            </template>
            <div v-if="phase.notes" style="font-size:13px;color:#666">{{ phase.notes }}</div>
            <NButton size="tiny" style="margin-top:4px" @click="editPhase(phase)">编辑阶段</NButton>
          </NTimelineItem>
        </NTimeline>

        <template #footer>
          <NButton @click="showDrawer=false">关闭</NButton>
        </template>
      </NDrawerContent>
    </NDrawer>

    <!-- 编辑阶段 Modal -->
    <NModal v-model:show="showPhaseModal" title="编辑阶段" style="width:420px">
      <NForm v-if="editingPhase" style="padding:24px" label-placement="left" label-width="80">
        <NFormItem label="状态">
          <NSelect v-model:value="editingPhase.status" :options="[{label:'待开始',value:'pending'},{label:'进行中',value:'in_progress'},{label:'已完成',value:'completed'}]" />
        </NFormItem>
        <NFormItem label="开始日期"><NInput v-model:value="editingPhase.start_date" placeholder="YYYY-MM-DD" /></NFormItem>
        <NFormItem label="结束日期"><NInput v-model:value="editingPhase.end_date" placeholder="YYYY-MM-DD" /></NFormItem>
        <NFormItem label="备注"><NInput v-model:value="editingPhase.notes" type="textarea" /></NFormItem>
        <NSpace justify="end" style="width:100%">
          <NButton @click="showPhaseModal=false">取消</NButton>
          <NButton type="primary" @click="savePhase">保存</NButton>
        </NSpace>
      </NForm>
    </NModal>
  </div>
</template>
