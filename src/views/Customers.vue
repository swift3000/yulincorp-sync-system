<script setup lang="ts">
import { ref, onMounted, h } from 'vue';
import { NDataTable, NButton, NModal, NForm, NFormItem, NInput, NSpace, NPopconfirm, useMessage } from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';

interface Customer {
  id: number; name: string; contact_person: string; phone: string;
  email: string; address: string; tax_id: string; notes: string;
  is_active: boolean; created_at: string;
}

const message = useMessage();
const loading = ref(false);
const items = ref<Customer[]>([]);
const total = ref(0);
const page = ref(1);
const keyword = ref('');

const showModal = ref(false);
const editing = ref<Customer | null>(null);
const form = ref({ name: '', contact_person: '', phone: '', email: '', address: '', tax_id: '', notes: '' });

const columns: DataTableColumns<Customer> = [
  { title: 'ID', key: 'id', width: 60 },
  { title: '名称', key: 'name', ellipsis: { tooltip: true } },
  { title: '联系人', key: 'contact_person', width: 100 },
  { title: '电话', key: 'phone', width: 120 },
  { title: '地址', key: 'address', ellipsis: { tooltip: true } },
  {
    title: '操作', key: 'actions', width: 160,
    render(row) {
      return h(NSpace, {}, {
        default: () => [
          h(NButton, { size: 'small', onClick: () => editItem(row) }, { default: () => '编辑' }),
          h(NPopconfirm, { onPositiveClick: () => removeItem(row.id) }, {
            trigger: () => h(NButton, { size: 'small', type: 'error' }, { default: () => '删除' }),
            default: () => '确定删除该客户？',
          }),
        ],
      });
    },
  },
];

async function loadData() {
  loading.value = true;
  try {
    const res = await invoke<{ items: Customer[]; total: number }>('list_customers', {
      keyword: keyword.value || null, page: page.value, pageSize: 20,
    });
    items.value = res.items;
    total.value = res.total;
  } catch (e: any) {
    message.error(typeof e === 'string' ? e : '加载失败');
  } finally { loading.value = false; }
}

function openCreate() {
  editing.value = null;
  form.value = { name: '', contact_person: '', phone: '', email: '', address: '', tax_id: '', notes: '' };
  showModal.value = true;
}

function editItem(row: Customer) { editing.value = row; form.value = { ...row }; showModal.value = true; }

async function saveItem() {
  if (!form.value.name.trim()) { message.warning('请输入客户名称'); return; }
  try {
    if (editing.value) {
      await invoke('update_customer', { id: editing.value.id, data: form.value });
      message.success('更新成功');
    } else {
      await invoke('create_customer', { data: form.value });
      message.success('创建成功');
    }
    showModal.value = false; loadData();
  } catch (e: any) { message.error(typeof e === 'string' ? e : '操作失败'); }
}

async function removeItem(id: number) {
  try { await invoke('delete_customer', { id }); message.success('已删除'); loadData(); }
  catch (e: any) { message.error(typeof e === 'string' ? e : '删除失败'); }
}

onMounted(loadData);
</script>

<template>
  <div>
    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:16px">
      <NSpace>
        <NInput v-model:value="keyword" placeholder="搜索客户..." style="width:240px" clearable @keyup.enter="page=1;loadData()" />
        <NButton @click="page=1;loadData()">搜索</NButton>
      </NSpace>
      <NButton type="primary" @click="openCreate">新增客户</NButton>
    </div>
    <NDataTable :columns="columns" :data="items" :loading="loading"
      :pagination="{ page, pageSize: 20, itemCount: total, onChange: (p:number) => { page=p; loadData(); } }" />
    <NModal v-model:show="showModal" :title="editing ? '编辑客户' : '新增客户'">
      <NForm style="padding:24px">
        <NFormItem label="名称" required><NInput v-model:value="form.name" /></NFormItem>
        <NFormItem label="联系人"><NInput v-model:value="form.contact_person" /></NFormItem>
        <NFormItem label="电话"><NInput v-model:value="form.phone" /></NFormItem>
        <NFormItem label="邮箱"><NInput v-model:value="form.email" /></NFormItem>
        <NFormItem label="地址"><NInput v-model:value="form.address" /></NFormItem>
        <NFormItem label="税号"><NInput v-model:value="form.tax_id" /></NFormItem>
        <NFormItem label="备注"><NInput v-model:value="form.notes" type="textarea" /></NFormItem>
        <NSpace justify="end" style="width:100%">
          <NButton @click="showModal=false">取消</NButton>
          <NButton type="primary" @click="saveItem">保存</NButton>
        </NSpace>
      </NForm>
    </NModal>
  </div>
</template>
