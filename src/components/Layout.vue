<script setup lang="ts">
import { useRouter, useRoute } from 'vue-router';
import { useAppStore } from '@/stores';
import { NLayout, NLayoutSider, NLayoutHeader, NLayoutContent, NMenu, NButton, NIcon, NSpace, NText } from 'naive-ui';
import { h, computed } from 'vue';
import type { MenuOption } from 'naive-ui';
import { APP_NAME } from '@/constants';
import {
  HomeOutline, CubeOutline, ArchiveOutline, CartOutline, CashOutline,
  DocumentTextOutline, BriefcaseOutline, BarChartOutline, SettingsOutline, LogOutOutline,
  PeopleOutline, BusinessOutline, PricetagsOutline, LayersOutline,
} from '@vicons/ionicons5';

const router = useRouter();
const route = useRoute();
const appStore = useAppStore();

const isLoggedIn = computed(() => appStore.isLoggedIn);

const menuOptions: MenuOption[] = [
  { label: '工作台', key: 'dashboard', icon: () => h(HomeOutline) },
  {
    label: '基础数据', key: 'basic-data', type: 'group',
    children: [
      { label: '供应商', key: 'suppliers', icon: () => h(BusinessOutline) },
      { label: '客户', key: 'customers', icon: () => h(PeopleOutline) },
      { label: '品牌', key: 'brands', icon: () => h(PricetagsOutline) },
      { label: '分类', key: 'categories', icon: () => h(LayersOutline) },
    ],
  },
  {
    label: '业务管理', key: 'biz', type: 'group',
    children: [
      { label: '产品管理', key: 'products', icon: () => h(CubeOutline) },
      { label: '库存管理', key: 'inventory', icon: () => h(ArchiveOutline) },
      { label: '采购管理', key: 'purchase', icon: () => h(CartOutline) },
      { label: '销售管理', key: 'sales', icon: () => h(CashOutline) },
      { label: '合同管理', key: 'contracts', icon: () => h(DocumentTextOutline) },
      { label: '项目管理', key: 'projects', icon: () => h(BriefcaseOutline) },
    ],
  },
  { label: '财务报表', key: 'finance', icon: () => h(BarChartOutline) },
  { label: '系统设置', key: 'settings', icon: () => h(SettingsOutline) },
];

const activeKey = computed(() => {
  const name = route.name as string;
  return name || 'dashboard';
});

function handleMenuUpdate(key: string) {
  router.push({ name: key });
}

function handleLogout() {
  appStore.logout();
  router.push({ name: 'login' });
}
</script>

<template>
  <!-- 未登录：直接显示登录页 -->
  <template v-if="!isLoggedIn">
    <router-view />
  </template>

  <!-- 已登录：显示布局 -->
  <NLayout v-else has-sider position="absolute">
    <NLayoutSider
      bordered
      collapse-mode="width"
      :collapsed-width="64"
      :width="220"
      :native-scrollbar="false"
      style="background: var(--n-color)"
    >
      <!-- Logo -->
      <div style="height: 56px; display: flex; align-items: center; justify-content: center; border-bottom: 1px solid var(--n-border-color)">
        <NText strong style="font-size: 16px; color: var(--primary-color)">
          {{ APP_NAME }}
        </NText>
      </div>

      <NMenu
        :value="activeKey"
        :options="menuOptions"
        @update:value="handleMenuUpdate"
      />
    </NLayoutSider>

    <NLayout>
      <NLayoutHeader bordered style="height: 56px; display: flex; align-items: center; justify-content: flex-end; padding: 0 24px">
        <NSpace align="center">
          <NText depth="3">{{ appStore.username }}</NText>
          <NButton text @click="handleLogout">
            <template #icon>
              <NIcon><LogOutOutline /></NIcon>
            </template>
            退出
          </NButton>
        </NSpace>
      </NLayoutHeader>

      <NLayoutContent
        :native-scrollbar="false"
        content-style="padding: 24px;"
      >
        <router-view />
      </NLayoutContent>
    </NLayout>
  </NLayout>
</template>
