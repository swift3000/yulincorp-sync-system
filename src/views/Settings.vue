<script setup lang="ts">
import { ref, onMounted, h, computed } from 'vue';
import {
  NDataTable, NButton, NModal, NForm, NFormItem, NInput,
  NSelect, NSpace, NTag, NPopconfirm, useMessage, NTabs, NTabPane,
  NCard, NDescriptions, NDescriptionsItem,
} from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '@/stores';
import { APP_NAME, APP_VERSION } from '@/constants';

interface UserRecord {
  id: number; username: string; display_name: string; role: string;
  is_active: boolean; created_at: string;
}

const message = useMessage();
const appStore = useAppStore();
const isAdmin = computed(() => appStore.userRole === 'admin' || appStore.userRole === 'boss');

// 用户管理
const users = ref<UserRecord[]>([]);
const userLoading = ref(false);
const showUserModal = ref(false);
const editingUserId = ref<number | null>(null);
const userForm = ref({
  username: '', password: '', display_name: '', role: 'employee',
});
const editingPassword = ref(false);

// 系统信息
const sysInfo = ref({ version: '', db_size_bytes: 0, user_count: 0, platform: '' });

const roleOptions = [
  { label: '管理员', value: 'admin' },
  { label: '老板', value: 'boss' },
  { label: '员工', value: 'employee' },
];
const roleMap: Record<string, string> = { admin: '管理员', boss: '老板', employee: '员工' };
const roleTagType: Record<string, any> = { admin: 'error', boss: 'warning', employee: 'info' };

const userColumns: DataTableColumns<UserRecord> = [
  { title: 'ID', key: 'id', width: 50 },
  { title: '用户名', key: 'username', width: 120 },
  { title: '显示名', key: 'display_name', width: 120 },
  {
    title: '角色', key: 'role', width: 80,
    render: r => h(NTag, { type: roleTagType[r.role] || 'default', size: 'small' as const }, { default: () => roleMap[r.role] || r.role }),
  },
  {
    title: '状态', key: 'is_active', width: 80,
    render: r => h(NTag, { type: r.is_active ? 'success' : 'error', size: 'small' as const }, { default: () => r.is_active ? '启用' : '禁用' }),
  },
  { title: '创建时间', key: 'created_at', width: 150 },
  {
    title: '操作', key: 'actions', width: 140,
    render(row) {
      return h(NSpace, { size: 4 }, { default: () => [
        h(NButton, { size: 'small', onClick: () => openEditUser(row) }, { default: () => '编辑' }),
        row.id !== 1
          ? h(NPopconfirm, { onPositiveClick: () => toggleUser(row) }, {
            trigger: () => h(NButton, { size: 'small', type: row.is_active ? 'warning' : 'success' }, { default: () => row.is_active ? '禁用' : '启用' }),
            default: () => `确认${row.is_active ? '禁用' : '启用'}用户 ${row.username}？`,
          })
          : null,
      ]});
    },
  },
];

// ── 加载 ──────────────────────────────────────────────

async function loadUsers() {
  if (!isAdmin.value) return;
  userLoading.value = true;
  try { users.value = await invoke<UserRecord[]>('list_users'); }
  catch (e: any) { message.error(e); }
  finally { userLoading.value = false; }
}
async function loadSysInfo() {
  try { sysInfo.value = await invoke<any>('get_system_info'); }
  catch (_) {}
}

// ── 用户CRUD ──────────────────────────────────────────

function openCreateUser() {
  editingUserId.value = null; editingPassword.value = true;
  userForm.value = { username: '', password: '', display_name: '', role: 'employee' };
  showUserModal.value = true;
}
function openEditUser(row: UserRecord) {
  editingUserId.value = row.id; editingPassword.value = false;
  userForm.value = { username: row.username, password: '', display_name: row.display_name, role: row.role };
  showUserModal.value = true;
}
async function saveUser() {
  if (!userForm.value.username && !editingUserId.value) { message.warning('请输入用户名'); return; }
  if (editingPassword.value && !userForm.value.password) { message.warning('请输入密码'); return; }
  try {
    if (editingUserId.value) {
      await invoke('update_user', {
        id: editingUserId.value,
        input: { displayName: userForm.value.display_name, role: userForm.value.role, ...(editingPassword.value ? { password: userForm.value.password } : {}) },
      });
      message.success('用户已更新');
    } else {
      await invoke('create_user', { input: userForm.value });
      message.success('用户已创建');
    }
    showUserModal.value = false; loadUsers();
  } catch (e: any) { message.error(typeof e === 'string' ? e : '操作失败'); }
}
async function toggleUser(row: UserRecord) {
  try {
    await invoke('update_user', { id: row.id, input: { isActive: !row.is_active } });
    message.success(row.is_active ? '已禁用' : '已启用');
    loadUsers();
  } catch (e: any) { message.error(e); }
}

onMounted(() => { loadSysInfo(); });
</script>

<template>
  <div>
    <h2 style="margin-bottom:16px">系统设置</h2>

    <NTabs type="line" animated @update:value="(v: string) => { if (v==='users' && isAdmin) loadUsers(); }">
      <!-- 用户管理 -->
      <NTabPane v-if="isAdmin" name="users" tab="用户管理">
        <div style="margin-bottom:12px">
          <NButton type="primary" @click="openCreateUser">+ 创建用户</NButton>
        </div>
        <NDataTable :columns="userColumns" :data="users" :loading="userLoading" :row-key="(r: UserRecord) => r.id" />
      </NTabPane>

      <!-- 系统信息 -->
      <NTabPane name="info" tab="系统信息">
        <NCard>
          <NDescriptions label-placement="left" :column="2" bordered>
            <NDescriptionsItem label="系统版本">v{{ APP_VERSION }}</NDescriptionsItem>
            <NDescriptionsItem label="运行平台">{{ sysInfo.platform }}</NDescriptionsItem>
            <NDescriptionsItem label="数据库大小">{{ (sysInfo.db_size_bytes / 1024).toFixed(1) }} KB</NDescriptionsItem>
            <NDescriptionsItem label="活跃用户数">{{ sysInfo.user_count }}</NDescriptionsItem>
            <NDescriptionsItem label="后端框架">Tauri 2.x + Rust</NDescriptionsItem>
            <NDescriptionsItem label="前端框架">Vue 3 + Naive UI</NDescriptionsItem>
            <NDescriptionsItem label="数据库引擎">SQLite 3</NDescriptionsItem>
            <NDescriptionsItem label="认证方式">PBKDF2-SHA256</NDescriptionsItem>
            <NDescriptionsItem label="当前用户">{{ appStore.displayName }}</NDescriptionsItem>
            <NDescriptionsItem label="当前角色">{{ roleMap[appStore.userRole] || appStore.userRole }}</NDescriptionsItem>
          </NDescriptions>
        </NCard>
        <div style="margin-top:16px; color:#999; font-size:13px; text-align:center">
          {{ APP_NAME }} · 采购销售协同管理系统 · 模块总数: 10 · IPC命令: 60+
        </div>
      </NTabPane>
    </NTabs>

    <!-- 用户创建/编辑 Modal -->
    <NModal v-model:show="showUserModal" :title="editingUserId ? '编辑用户' : '创建用户'" style="width:440px">
      <NForm style="padding:24px" label-placement="left" label-width="80">
        <NFormItem v-if="!editingUserId" label="用户名" required><NInput v-model:value="userForm.username" /></NFormItem>
        <NFormItem label="显示名称"><NInput v-model:value="userForm.display_name" /></NFormItem>
        <NFormItem label="角色" required>
          <NSelect v-model:value="userForm.role" :options="roleOptions" />
        </NFormItem>
        <NFormItem :label="editingUserId ? '新密码' : '密码'" :required="!editingUserId">
          <div style="display:flex; align-items:center; gap:8px; width:100%">
            <NInput v-if="editingPassword" v-model:value="userForm.password" type="password" :placeholder="editingUserId ? '留空则不修改' : '请输入密码'" style="flex:1" />
            <span v-else style="color:#999; flex:1">不修改密码</span>
            <NButton v-if="editingUserId" size="small" @click="editingPassword=!editingPassword">{{ editingPassword ? '取消' : '修改密码' }}</NButton>
          </div>
        </NFormItem>
        <NSpace justify="end" style="width:100%">
          <NButton @click="showUserModal=false">取消</NButton>
          <NButton type="primary" @click="saveUser">保存</NButton>
        </NSpace>
      </NForm>
    </NModal>
  </div>
</template>
