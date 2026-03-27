<!-- Brand Guidelines - FoxNIO Logo -->

# FoxNIO Logo 使用规范

## 🦊 Logo 设计

### 设计理念
- **极简几何狐狸** - 三角形组合构成的抽象狐狸头像
- **T字镂空** - 象征技术(Technology)和传输(Transfer)
- **黑白配色** - 经典、专业、永恒

### 几何构成
```
耳朵：两个向上的三角形
脸部：倒三角形
镂空：横向钝角三角形 + 竖向锐角三角形（T字型）
```

---

## 🎨 颜色规范

### 亮色模式（Light Mode）
- **狐狸主体**：`#1a1a1a` (深灰黑)
- **镂空部分**：`#ffffff` (纯白)
- **背景**：浅色背景

### 暗色模式（Dark Mode）
- **狐狸主体**：`#ffffff` (纯白)
- **镂空部分**：`#1a1a1a` (深灰黑)
- **背景**：深色背景

### 品牌色
- **主色**：`#1a1a1a` (深灰黑)
- **辅助色**：`#ffffff` (纯白)
- **强调色**：`#3b82f6` (蓝色 - 用于交互元素)

---

## 🦊 FoxNIO Logo 集成完成！

### 📁 Logo 文件

```
frontend/static/
├── logo.svg          # 自适应版本（推荐）
├── logo-light.svg    # 亮色版本
└── logo-dark.svg     # 暗色版本
```

### 🎨 使用位置

1. **Sidebar.svelte**
   - 侧边栏顶部 Logo
   - 底部版本信息 Logo
   - 自动深浅色切换 ✅

2. **+layout.svelte**
   - 移动端导航栏 Logo
   - 页脚 Logo
   - 自动深浅色切换 ✅

---

## ✅ 深浅色自动切换

### 实现方式

```svelte
<script>
  let isDark = $state(false);
  
  onMount(() => {
    // 检测系统主题
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    isDark = mediaQuery.matches;
    
    // 监听主题变化
    mediaQuery.addEventListener('change', (e) => {
      isDark = e.matches;
    });
    
    // 监听文档主题变化
    const observer = new MutationObserver(() => {
      isDark = document.documentElement.classList.contains('dark');
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class']
    });
  });
</script>

{#if isDark}
  <!-- 暗色 Logo -->
  <svg viewBox="0 0 100 100" fill="none">
    <path fill="#ffffff"/> <!-- 白色狐狸 -->
    <path fill="#1a1a1a"/> <!-- 黑色镂空 -->
  </svg>
{:else}
  <!-- 亮色 Logo -->
  <svg viewBox="0 0 100 100" fill="none">
    <path fill="#1a1a1a"/> <!-- 黑色狐狸 -->
    <path fill="#ffffff"/> <!-- 白色镂空 -->
  </svg>
{/if}
```

---

## 🎯 品牌定位

### 品牌关键词
- **优雅** - 极简设计，去除多余装饰
- **专业** - 黑白经典，永不过时
- **克制** - 几何构成，理性表达

### 设计语言
- 极简主义
- 几何图形
- 黑白对比
- 负空间运用

---

**FoxNIO - AI API Gateway**
**优雅 · 专业 · 克制**
