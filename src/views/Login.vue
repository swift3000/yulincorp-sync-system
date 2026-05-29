<script setup lang="ts">
import { ref } from 'vue';
import { useRouter } from 'vue-router';
import { useAppStore } from '@/stores';
import { NForm, NFormItem, NInput, NButton, NCard, NText, NAlert } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { APP_NAME, APP_VERSION } from '@/constants';

const router = useRouter();
const store = useAppStore();

const username = ref('');
const password = ref('');
const loading = ref(false);
const error = ref('');

async function handleLogin() {
  if (!username.value.trim() || !password.value) {
    error.value = '请输入用户名和密码';
    return;
  }
  loading.value = true;
  error.value = '';

  try {
    const res = await invoke<{
      success: boolean;
      message: string;
      user: { id: number; username: string; display_name: string; role: string } | null;
      token: string | null;
    }>('login', { req: { username: username.value.trim(), password: password.value } });

    if (res.success && res.user && res.token) {
      store.login({
        id: res.user.id,
        username: res.user.username,
        display_name: res.user.display_name,
        role: res.user.role,
        token: res.token,
      });
      router.push({ name: 'dashboard' });
    } else {
      error.value = res.message || '登录失败';
    }
  } catch (e: any) {
    error.value = typeof e === 'string' ? e : '登录失败，请检查系统状态';
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div style="display: flex; align-items: center; justify-content: center; height: 100vh; background: #f5f7fa">
    <NCard :title="`${APP_NAME} · 协同管理系统`" style="width: 400px">
      <template #header-extra>
        <NText depth="3" style="font-size: 12px">v{{ APP_VERSION }}</NText>
      </template>

      <NForm>
        <NFormItem label="用户名">
          <NInput
            v-model:value="username"
            placeholder="请输入用户名"
            :disabled="loading"
            @keyup.enter="handleLogin"
          />
        </NFormItem>
        <NFormItem label="密码">
          <NInput
            v-model:value="password"
            type="password"
            placeholder="请输入密码"
            :disabled="loading"
            @keyup.enter="handleLogin"
            show-password-on="click"
          />
        </NFormItem>

        <NAlert v-if="error" type="error" :title="error" style="margin-bottom: 16px" />

        <NButton type="primary" block :loading="loading" @click="handleLogin">
          登录
        </NButton>
      </NForm>

      <template #footer>
        <NText depth="3" style="font-size: 12px; text-align: center; width: 100%; display: block">
          陕西昱霖科技有限公司 © 2026
        </NText>
      </template>
    </NCard>
  </div>
</template>
