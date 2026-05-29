<script setup lang="ts">
import { ref, onMounted, h, computed } from 'vue';
import { NDataTable, NButton, NModal, NForm, NFormItem, NInput, NSelect, NSpace, NPopconfirm, useMessage } from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';

interface Category {
  id: number; name: string; parent_id: number | null; sort_order: number; created_at: string;
}

const message = useMessage();
const loading = ref(false);
const items = ref<Category[]>([]);
const showModal = ref(false);
const editing = ref<Category | null>(null);
const form = ref({ name: '', parent_id: null as number | null });

const parentOptions = computed(() =>
  items.value
    .filter(c => c.id !== editing.value?.id)
    .map(c => ({ label: c.name, value: c.id }))
);

const columns: DataTableColumns<Category> = [
  { title: 'ID', key: 'id', width: 60 },
  { title: '名称', key: 'name' },
  {
    title: '上级分类', key: 'parent_id',
    render(row) {
      const parent = items.value.find(c => c.id === row.parent_id);
      return parent?.name || '—';
    },
  },
  {
    title: '操作', key: 'actions', width: 160,
    render(row) {
      return h(NSpace, {}, {
        default: () => [
          h(NButton, { size: 'small', onClick: () => editItem(row) }, { default: () => '编辑' }),
          h(NPopconfirm, { onPositiveClick: () => removeItem(row.id) }, {
            trigger: () => h(NButton, { size: 'small', type: 'error' }, { default: () => '删除' }),
            default: () => '确定删除该分类？',
          }),
        ],
      });
    },
  },
];

async function loadData() {
  loading.value = true;
  try { items.value = await invoke<Category[]>('list_categories'); }
  catch (e: any) { message.error(typeof e === 'string' ? e : '加载失败'); }
  finally { loading.value = false; }
}

function openCreate() {
  editing.value = null; form.value = { name: '', parent_id: null }; showModal.value = true;
}
function editItem(row: Category) { editing.value = row; form.value = { name: row.name, parent_id: row.parent_id }; showModal.value = true; }

async function saveItem() {
  if (!form.value.name.trim()) { message.warning('请输入分类名'); return; }
  try {
    if (editing.value) {
      await invoke('update_category', { id: editing.value.id, name: form.value.name, parentId: form.value.parent_id });
      message.success('更新成功');
    } else {
      await invoke('create_category', { name: form.value.name, parentId: form.value.parent_id });
      message.success('创建成功');
    }
    showModal.value = false; loadData();
  } catch (e: any) { message.error(typeof e === 'string' ? e : '操作失败'); }
}

async function removeItem(id: number) {
  try { await invoke('delete_category', { id }); message.success('已删除'); loadData(); }
  catch (e: any) { message.error(typeof e === 'string' ? e : '删除失败'); }
}

onMounted(loadData);
</script>

<template>
  <div>
    <div style="display:flex; justify-content:flex-end; margin-bottom:16px">
      <NButton type="primary" @click="openCreate">新增分类</NButton>
    </div>
    <NDataTable :columns="columns" :data="items" :loading="loading" />
    <NModal v-model:show="showModal" :title="editing ? '编辑分类' : '新增分类'">
      <NForm style="padding:24px">
        <NFormItem label="名称" required><NInput v-model:value="form.name" /></NFormItem>
        <NFormItem label="上级分类">
          <NSelect v-model:value="form.parent_id" :options="parentOptions" clearable placeholder="无（顶级分类）" />
        </NFormItem>
        <NSpace justify="end" style="width:100%">
          <NButton @click="showModal=false">取消</NButton>
          <NButton type="primary" @click="saveItem">保存</NButton>
        </NSpace>
      </NForm>
    </NModal>
  </div>
</template>
