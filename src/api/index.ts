//! Tauri IPC 调用封装层
//!
//! 前端通过此模块调用 Rust 后端命令，自动处理序列化和错误。

import { invoke } from '@tauri-apps/api/core';

// ============================================================
// 认证
// ============================================================

export interface UserInfo {
  id: number;
  username: string;
  display_name: string;
  role: string;
}

export interface LoginResponse {
  success: boolean;
  message: string;
  user: UserInfo | null;
}

export async function login(username: string, password: string): Promise<LoginResponse> {
  return invoke<LoginResponse>('login', {
    req: { username, password },
  });
}

export async function getCurrentUser(token: string): Promise<UserInfo | null> {
  return invoke<UserInfo | null>('get_current_user', { token });
}

export async function logout(token: string): Promise<void> {
  return invoke<void>('logout', { token });
}

// ============================================================
// 仪表盘
// ============================================================

export interface DashboardStats {
  today_sales: number;
  pending_purchase: number;
  stock_alerts: number;
  project_count: number;
  total_customers: number;
  total_suppliers: number;
  monthly_revenue: number;
  monthly_profit: number;
}

export async function getStats(): Promise<DashboardStats> {
  return invoke<DashboardStats>('get_stats');
}

// ============================================================
// 产品管理
// ============================================================

export interface Product {
  id: number;
  name: string;
  sku: string;
  brand_id: number | null;
  brand_name: string | null;
  category_id: number | null;
  category_name: string | null;
  unit: string;
  spec: string;
  purchase_price: number;
  sale_price: number;
  min_stock: number;
  is_active: boolean;
  notes: string;
  created_at: string;
}

export interface ProductListResponse {
  products: Product[];
  total: number;
  page: number;
  page_size: number;
}

export async function listProducts(params: {
  page?: number;
  page_size?: number;
  keyword?: string;
  brand_id?: number;
  category_id?: number;
}): Promise<ProductListResponse> {
  return invoke<ProductListResponse>('list_products', params);
}

export async function getProduct(id: number): Promise<Product | null> {
  return invoke<Product | null>('get_product', { id });
}

export async function createProduct(data: {
  name: string;
  sku: string;
  brand_id?: number;
  category_id?: number;
  unit?: string;
  spec?: string;
  purchase_price?: number;
  sale_price?: number;
  min_stock?: number;
  notes?: string;
}): Promise<Product> {
  return invoke<Product>('create_product', { data });
}

export async function updateProduct(id: number, data: {
  name?: string;
  sku?: string;
  brand_id?: number;
  category_id?: number;
  unit?: string;
  spec?: string;
  purchase_price?: number;
  sale_price?: number;
  min_stock?: number;
  notes?: string;
}): Promise<Product> {
  return invoke<Product>('update_product', { id, data });
}

export async function deleteProduct(id: number): Promise<void> {
  return invoke<void>('delete_product', { id });
}

// ============================================================
// 库存管理
// ============================================================

export interface StockItem {
  product_id: number;
  product_name: string;
  sku: string;
  quantity: number;
  locked_quantity: number;
  available_quantity: number;
  min_stock: number;
  unit: string;
  updated_at: string;
}

export async function getStock(params: {
  keyword?: string;
  low_stock_only?: boolean;
}): Promise<StockItem[]> {
  return invoke<StockItem[]>('get_stock', params);
}

export async function stockIn(params: {
  product_id: number;
  quantity: number;
  warehouse_id?: number;
  notes?: string;
}): Promise<void> {
  return invoke<void>('stock_in', params);
}

export async function stockOut(params: {
  product_id: number;
  quantity: number;
  warehouse_id?: number;
  notes?: string;
}): Promise<void> {
  return invoke<void>('stock_out', params);
}

// ============================================================
// 采购管理
// ============================================================

export interface PurchaseOrder {
  id: number;
  order_no: string;
  supplier_name: string;
  total_amount: number;
  status: string;
  order_date: string;
  expected_date: string;
  items_count: number;
}

export async function listPurchaseOrders(params: {
  status?: string;
  page?: number;
  page_size?: number;
}): Promise<PurchaseOrder[]> {
  return invoke<PurchaseOrder[]>('list_purchase_orders', params);
}

// ============================================================
// 销售管理
// ============================================================

export interface SalesOrder {
  id: number;
  order_no: string;
  customer_name: string;
  total_amount: number;
  profit: number;
  status: string;
  order_date: string;
  delivery_date: string | null;
  items_count: number;
}

export async function listSalesOrders(params: {
  status?: string;
  page?: number;
  page_size?: number;
}): Promise<SalesOrder[]> {
  return invoke<SalesOrder[]>('list_sales_orders', params);
}
