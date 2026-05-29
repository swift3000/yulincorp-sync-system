import { createRouter, createWebHistory } from 'vue-router';
import { useAppStore } from '@/stores';

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'login',
      component: () => import('@/views/Login.vue'),
    },
    {
      path: '/dashboard',
      name: 'dashboard',
      component: () => import('@/views/Dashboard.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/suppliers',
      name: 'suppliers',
      component: () => import('@/views/Suppliers.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/customers',
      name: 'customers',
      component: () => import('@/views/Customers.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/brands',
      name: 'brands',
      component: () => import('@/views/Brands.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/categories',
      name: 'categories',
      component: () => import('@/views/Categories.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/products',
      name: 'products',
      component: () => import('@/views/Products.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/inventory',
      name: 'inventory',
      component: () => import('@/views/Inventory.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/purchase',
      name: 'purchase',
      component: () => import('@/views/Purchase.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/sales',
      name: 'sales',
      component: () => import('@/views/Sales.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/contracts',
      name: 'contracts',
      component: () => import('@/views/Contracts.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/projects',
      name: 'projects',
      component: () => import('@/views/Projects.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/finance',
      name: 'finance',
      component: () => import('@/views/Finance.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/settings',
      name: 'settings',
      component: () => import('@/views/Settings.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/:pathMatch(.*)*',
      name: 'not-found',
      component: () => import('@/views/NotFound.vue'),
    },
  ],
});

// 路由守卫：未登录重定向到登录页
router.beforeEach(async (to, _from, next) => {
  if (to.meta.requiresAuth) {
    const store = useAppStore();
    if (!store.isLoggedIn) {
      next({ name: 'login' });
      return;
    }
    next();
  } else {
    next();
  }
});

export default router;
