import { createApp } from 'vue'
import App from './App.vue'

// 全局拦截右键菜单，消除浏览器默认行为
document.addEventListener('contextmenu', (e) => {
  e.preventDefault()
})

// 禁用默认拖拽行为
document.addEventListener('dragstart', (e) => {
  e.preventDefault()
})

// 禁用某些快捷键（如 Ctrl+S 保存页面）
document.addEventListener('keydown', (e) => {
  // 禁用 Ctrl+S (保存页面)
  if (e.ctrlKey && e.key === 's') {
    e.preventDefault()
  }
  // 禁用 Ctrl+P (打印)
  if (e.ctrlKey && e.key === 'p') {
    e.preventDefault()
  }
  // 禁用 F5 (刷新) - 可选，根据需要启用
  // if (e.key === 'F5') {
  //   e.preventDefault()
  // }
})

createApp(App).mount('#app')
