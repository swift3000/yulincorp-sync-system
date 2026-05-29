<script setup lang="ts">
import { ref, onMounted, h } from 'vue';
import { NDataTable, NButton, NModal, NForm, NFormItem, NInput, NSpace, NPopconfirm, useMessage } from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';

interface Brand {
  id: number; name: string; logo_path: string; website: string;
  description: string; is_active: boolean; created_at: string;
}

const message = useMessage();
const loading = ref(false);
const items = ref<Brand[]>([]);
const keyword = ref('');

const showModal = ref(false);
const editing = ref<Brand | null>(null);
const form = ref({ name: '', website: '', description: '' });

const columns: DataTableColumns<Brand> = [
  { title: 'ID', key: 'id', width: 60 },
  { title: '品牌名', key: 'name' },
  { title: '网站', key: 'website', ellipsis: { tooltip: true } },
  { title: '描述', key: 'description', ellipsis: { tooltip: true } },
  {
    title: '操作', key: 'actions', width: 160,
    render(row) {
      return h(NSpace, {}, {
        default: () => [
          h(NButton, { size: 'small', onClick: () => editItem(row) }, { default: () => '编辑' }),
          h(NPopconfirm, { onPositiveClick: () => removeItem(row.id) }, {
            trigger: () => h(NButton, { size: 'small', type: 'error' }, { default: () => '删除' }),
            default: () => '确定删除该品牌？',
          }),
        ],
      });
    },
  },
];

async function loadData() {
  loading.value = true;
  try {
    items.value = await invoke<Brand[]>('list_brands', { keyword: keyword.value || null });
  } catch (e: any) { message.error(typeof e === 'string' ? e : '加载失败'); }
  finally { loading.value = false; }
}

function openCreate() {
  editing.value = null; form.value = { name: '', website: '', description: '' }; showModal.value = true;
}
function editItem(row: Brand) { editing.value = row; form.value = { name: row.name, website: row.website, description: row.description }; showModal.value = true; }

async function saveItem() {
  if (!form.value.name.trim()) { message.warning('请输入品牌名'); return; }
  try {
    if (editing.value) {
      await invoke('update_brand', { id: editing.value.id, name: form.value.name, website: form.value.website, description: form.value.description });
      message.success('更新成功');
    } else {
      await invoke('create_brand', { name: form.value.name });
      message.success('创建成功');
    }
    showModal.value = false; loadData();
  } catch (e: any) { message.error(typeof e === 'string' ? e : '操作失败'); }
}

async function removeItem(id: number) {
  try { await invoke('delete_brand', { id }); message.success('已删除'); loadData(); }
  catch (e: any) { message.error(typeof e === 'string' ? e : '删除失败'); }
}

onMounted(loadData);
</script>

<template>
  <div>
    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:16px">
      <NSpace>
        <NInput v-model:value="keyword" placeholder="搜索品牌..." style="width:240px" clearable @keyup.enter="loadData()" />
        <NButton @click="loadData()">搜索</NButton>
      </NSpace>
      <NButton type="primary" @click="openCreate">新增品牌</NButton>
    </div>
    <NDataTable :columns="columns" :data="items" :loading="loading" />
    <NModal v-model:show="showModal" :title="editing ? '编辑品牌' : '新增品牌'">
      <NForm style="padding:24px">
        <NFormItem label="品牌名" required><NInput v-model:value="form.name" /></NFormItem>
        <NFormItem label="网站"><NInput v-model:value="form.website" /></NFormItem>
        <NFormItem label="描述"><NInput v-model:value="form.description" type="textarea" /></NFormItem>
        <NSpace justify="end" style="width:100%">
          <NButton @click="showModal=false">取消</NButton>
          <NButton type="primary" @click="saveItem">保存</NButton>
        </NSpace>
      </NForm>
    </NModal>
  </div>
</template>
