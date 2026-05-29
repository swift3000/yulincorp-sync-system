<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useAppStore } from '@/stores';
import { NCard, NGrid, NGi, NStatistic, NSpace, NText, NSpin, useMessage } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';

interface Stats {
  today_sales: number;
  pending_purchase: number;
  stock_alerts: number;
  project_count: number;
  total_customers: number;
  total_suppliers: number;
  monthly_revenue: number;
  monthly_profit: number;
}

const store = useAppStore();
const message = useMessage();
const stats = ref<Stats>({
  today_sales: 0, pending_purchase: 0, stock_alerts: 0, project_count: 0,
  total_customers: 0, total_suppliers: 0, monthly_revenue: 0, monthly_profit: 0,
});
const loading = ref(true);

onMounted(async () => {
  try {
    stats.value = await invoke<Stats>('get_stats');
  } catch (e) {
    message.error(typeof e === 'string' ? e : '获取统计数据失败');
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <NSpin :show="loading">
    <div>
      <NText tag="h2" depth="1" style="margin-bottom: 8px">
        欢迎回来，{{ store.displayName }}
      </NText>
      <NText depth="3" style="display: block; margin-bottom: 24px">
        角色：{{ store.userRole === 'admin' ? '管理员' : store.userRole === 'boss' ? '老板' : '员工' }}
      </NText>

      <NGrid :cols="4" :x-gap="16" :y-gap="16">
        <NGi>
          <NCard>
            <NStatistic label="今日销售额" :value="stats.today_sales">
              <template #prefix>¥</template>
            </NStatistic>
          </NCard>
        </NGi>
        <NGi>
          <NCard>
            <NStatistic label="本月营收" :value="stats.monthly_revenue">
              <template #prefix>¥</template>
            </NStatistic>
          </NCard>
        </NGi>
        <NGi>
          <NCard>
            <NStatistic label="待处理采购单" :value="stats.pending_purchase" />
          </NCard>
        </NGi>
        <NGi>
          <NCard>
            <NStatistic label="库存预警" :value="stats.stock_alerts" />
          </NCard>
        </NGi>
        <NGi>
          <NCard>
            <NStatistic label="进行中项目" :value="stats.project_count" />
          </NCard>
        </NGi>
        <NGi>
          <NCard>
            <NStatistic label="客户总数" :value="stats.total_customers" />
          </NCard>
        </NGi>
        <NGi>
          <NCard>
            <NStatistic label="供应商总数" :value="stats.total_suppliers" />
          </NCard>
        </NGi>
        <NGi>
          <NCard>
            <NStatistic label="本月毛利" :value="stats.monthly_profit">
              <template #prefix>¥</template>
            </NStatistic>
          </NCard>
        </NGi>
      </NGrid>

      <div style="margin-top: 24px">
        <NCard title="快速入口">
          <NSpace>
            <NText depth="3">功能模块持续开发中，敬请期待...</NText>
          </NSpace>
        </NCard>
      </div>
    </div>
  </NSpin>
</template>
