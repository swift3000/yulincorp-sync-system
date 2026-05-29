<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { NDataTable, NButton, NModal, NForm, NFormItem, NInput, NSpace, NPopconfirm, useMessage } from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';

interface Supplier {
  id: number; name: string; contact_person: string; phone: string;
  email: string; address: string; bank_account: string; tax_id: string;
  notes: string; is_active: boolean; created_at: string;
}

const message = useMessage();
const loading = ref(false);
const items = ref<Supplier[]>([]);
const total = ref(0);
const page = ref(1);
const keyword = ref('');

const showModal = ref(false);
const editing = ref<Supplier | null>(null);
const form = ref({
  name: '', contact_person: '', phone: '', email: '',
  address: '', bank_account: '', tax_id: '', notes: '',
});

const columns: DataTableColumns<Supplier> = [
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
            default: () => '确定删除该供应商？',
          }),
        ],
      });
    },
  },
];

import { h } from 'vue';

async function loadData() {
  loading.value = true;
  try {
    const res = await invoke<{ items: Supplier[]; total: number }>('list_suppliers', {
      keyword: keyword.value || null, page: page.value, pageSize: 20,
    });
    items.value = res.items;
    total.value = res.total;
  } catch (e: any) {
    message.error(typeof e === 'string' ? e : '加载失败');
  } finally {
    loading.value = false;
  }
}

function openCreate() {
  editing.value = null;
  form.value = { name: '', contact_person: '', phone: '', email: '', address: '', bank_account: '', tax_id: '', notes: '' };
  showModal.value = true;
}

function editItem(row: Supplier) {
  editing.value = row;
  form.value = { ...row };
  showModal.value = true;
}

async function saveItem() {
  if (!form.value.name.trim()) {
    message.warning('请输入供应商名称');
    return;
  }
  try {
    if (editing.value) {
      await invoke('update_supplier', { id: editing.value.id, data: form.value });
      message.success('更新成功');
    } else {
      await invoke('create_supplier', { data: form.value });
      message.success('创建成功');
    }
    showModal.value = false;
    loadData();
  } catch (e: any) {
    message.error(typeof e === 'string' ? e : '操作失败');
  }
}

async function removeItem(id: number) {
  try {
    await invoke('delete_supplier', { id });
    message.success('已删除');
    loadData();
  } catch (e: any) {
    message.error(typeof e === 'string' ? e : '删除失败');
  }
}

onMounted(loadData);
</script>

<template>
  <div>
    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:16px">
      <NSpace>
        <NInput v-model:value="keyword" placeholder="搜索供应商..." style="width:240px" clearable @keyup.enter="page=1;loadData()" />
        <NButton @click="page=1;loadData()">搜索</NButton>
      </NSpace>
      <NButton type="primary" @click="openCreate">新增供应商</NButton>
    </div>
    <NDataTable :columns="columns" :data="items" :loading="loading" :pagination="{ page, pageSize: 20, itemCount: total, onChange: (p:number) => { page=p; loadData(); } }" />

    <NModal v-model:show="showModal" :title="editing ? '编辑供应商' : '新增供应商'">
      <NForm style="padding:24px">
        <NFormItem label="名称" required><NInput v-model:value="form.name" /></NFormItem>
        <NFormItem label="联系人"><NInput v-model:value="form.contact_person" /></NFormItem>
        <NFormItem label="电话"><NInput v-model:value="form.phone" /></NFormItem>
        <NFormItem label="邮箱"><NInput v-model:value="form.email" /></NFormItem>
        <NFormItem label="地址"><NInput v-model:value="form.address" /></NFormItem>
        <NFormItem label="银行账号"><NInput v-model:value="form.bank_account" /></NFormItem>
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
