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

## 📐 尺寸规范

### 标准尺寸
- **导航栏 Logo**：32x32px
- **侧边栏 Logo**：32x32px
- **Favicon**：16x16px / 32x32px
- **大图标**：64x64px / 128x128px

### 最小尺寸
- **最小显示尺寸**：16x16px
- **安全边距**：Logo 四周保留至少 4px 空白

---

## 💡 使用场景

### 1. 导航栏
```html
<!-- 亮色背景 -->
<svg class="w-8 h-8">
  <path fill="#1a1a1a"/> <!-- 主体 -->
  <path fill="#ffffff"/> <!-- 镂空 -->
</svg>

<!-- 暗色背景 -->
<svg class="w-8 h-8">
  <path fill="#ffffff"/> <!-- 主体 -->
  <path fill="#1a1a1a"/> <!-- 镂空 -->
</svg>
```

### 2. 加载动画
```html
<!-- 旋转 Logo -->
<svg class="w-12 h-12 animate-spin">
  <!-- Logo SVG -->
</svg>
```

### 3. 品牌水印
```html
<!-- 半透明水印 -->
<svg class="opacity-10">
  <!-- Logo SVG -->
</svg>
```

---

## ✅ 使用规范

### ✅ 正确用法
- 保持 Logo 完整性，不拆分使用
- 保持颜色对比度，确保清晰可见
- 与品牌文字 "FoxNIO" 组合时，保持合理间距
- 响应式场景下自动切换深浅色

### ❌ 禁止用法
- 不要改变 Logo 颜色（仅限黑白两色）
- 不要拉伸或压缩 Logo
- 不要添加阴影、描边等效果
- 不要在复杂背景上使用，影响识别度
- 不要修改几何形状

---

## 🔄 自动切换实现

### CSS 方式
```css
.logo-dark { display: none; }

@media (prefers-color-scheme: dark) {
  .logo-light { display: none; }
  .logo-dark { display: block; }
}
```

### JavaScript 方式
```javascript
const isDark = document.documentElement.classList.contains('dark') ||
               window.matchMedia('(prefers-color-scheme: dark)').matches;

// 根据主题切换 SVG fill 颜色
```

### Svelte 组件
```svelte
{#if isDark}
  <!-- 暗色 Logo -->
{:else}
  <!-- 亮色 Logo -->
{/if}
```

---

## 📦 文件格式

### 提供格式
- `logo.svg` - 自适应版本（推荐）
- `logo-light.svg` - 亮色版本
- `logo-dark.svg` - 暗色版本
- `logo.png` - PNG 格式（用于不支持 SVG 的场景）

### SVG 优势
- 无限缩放不失真
- 文件体积小
- 支持颜色动态修改
- 适合 Web 使用

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
