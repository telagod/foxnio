// Svelte stores

import { writable } from 'svelte/store';

// 主题
export const theme = writable<'light' | 'dark'>('light');

// 用户信息
export const user = writable<{
  id: string;
  email: string;
  role: string;
} | null>(null);

// 通知
export interface Notification {
  id: string;
  type: 'success' | 'error' | 'info';
  message: string;
}

export const notifications = writable<Notification[]>([]);

export function addNotification(type: Notification['type'], message: string) {
  const id = Date.now().toString();
  notifications.update(n => [...n, { id, type, message }]);
  
  // 3秒后自动移除
  setTimeout(() => {
    notifications.update(n => n.filter(item => item.id !== id));
  }, 3000);
}

export function removeNotification(id: string) {
  notifications.update(n => n.filter(item => item.id !== id));
}
