import { installFrontendLogBridge } from '$lib/api';
import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';

// 最早期的 console.log — 跑通即说明 webview JS 启动成功
// 装前端 log 桥 (把 console.log/error 转发到 backend 日志)
installFrontendLogBridge();

console.log('[main] script start, mounting App at', new Date().toISOString());

const target = document.getElementById('app');
if (!target) {
  console.error('[main] #app root not found');
  throw new Error('#app root not found');
}

const app = mount(App, { target });
console.log('[main] App mounted');

export default app;