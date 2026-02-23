# 开发问题与解决方案

本文档记录了 MOSM 开发过程中遇到的主要问题及其解决方案。

---

## 1. 地图渲染比例失真（水平拉伸）

### 问题描述

加载 OSM PBF 数据后，地图显示出现严重的水平方向拉伸。例如，摩纳哥港口（Port Hercule）的防波堤角度与 OSM 官方网站显示不一致，水平方向明显被拉伸了 1.5-2 倍。

### 根本原因

**问题 1: 缺少地图投影**

最初的实现直接使用 WGS84 经纬度坐标作为 Canvas 的 X/Y 坐标，没有进行 Web Mercator 投影转换。

在 WGS84 坐标系中：
- 1° 经度在不同纬度对应的实际距离不同（赤道约 111km，高纬度地区更短）
- 1° 纬度始终约 111km

直接使用经纬度会导致高纬度地区的地图水平方向被压缩。

**问题 2: Canvas 初始化时机错误**

即使投影正确后，重启应用时仍出现变形。原因是在 Vue 的 `onMounted` 钩子中直接初始化渲染器，此时 DOM 虽已挂载，但 CSS 布局可能还未完全计算完成。`getBoundingClientRect()` 获取的尺寸不准确，导致 Canvas 物理分辨率与 CSS 显示尺寸不匹配。

### 解决方案

**1. 在 Rust 端实现 Web Mercator 投影**

创建 `src-tauri/src/projection.rs`：

```rust
use std::f64::consts::PI;

const EARTH_HALF_CIRCUMFERENCE: f64 = 20037508.342789244;

pub fn lonlat_to_mercator(lon: f64, lat: f64) -> (f64, f64) {
    let x = lon * EARTH_HALF_CIRCUMFERENCE / 180.0;
    let lat_clamped = lat.clamp(-85.051129, 85.051129);
    let lat_rad = (90.0 + lat_clamped) * PI / 360.0;
    let y = lat_rad.tan().ln() * EARTH_HALF_CIRCUMFERENCE / PI;
    (x, y)
}
```

在 `binary_protocol.rs` 的 `encode_ways_geometry` 和 `encode_priority_nodes` 中应用投影，将经纬度转换为墨卡托坐标后再发送给前端。

**2. 延迟初始化渲染器**

在 `src/composables/useMapRenderer.ts` 中使用双重 `requestAnimationFrame` 确保布局完成：

```typescript
onMounted(() => {
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      initialize()
    })
  })
  window.addEventListener('resize', resize)
})
```

### 验证方法

1. 绘制一个固定像素大小的正方形（如 100×100），确认显示为正方形
2. 绘制一个固定米数的墨卡托正方形（如 1000m×1000m），确认显示为正方形且 Ratio = 1.000
3. 与 OSM 官方网站对比相同区域的地图形状

---

## 2. 节点不显示

### 问题描述

实现节点 LOD（细节层次）策略后，节点完全不显示，无论缩放级别如何。

### 根本原因

`MapRenderer` 的 `pan()` 和 `zoomAt()` 方法更新了内部相机状态，但 `useMapRenderer.ts` 中的 Vue `watch` 只监听直接的属性变化，无法检测到内部状态变化，导致 `debouncedFetchData()` 没有被触发。

### 解决方案

在 `MapRenderer` 中添加相机变化回调机制：

```typescript
// MapRenderer 中
private onCameraChange: (() => void) | null = null

setOnCameraChange(callback: () => void): void {
  this.onCameraChange = callback
}

private notifyCameraChange(): void {
  if (this.onCameraChange) {
    this.onCameraChange()
  }
}

// 在 pan() 和 zoomAt() 末尾调用
this.notifyCameraChange()
```

```typescript
// useMapRenderer.ts 中
renderer.value.setOnCameraChange(() => {
  debouncedFetchData()
})
```

---

## 3. PBF 加载时间过长

### 问题描述

加载 Andorra PBF 文件（约 3MB）需要约 1 分钟。

### 解决方案

使用并行解析策略：

1. 使用 `rayon` 并行处理 PBF 数据块
2. 使用 `DashMap` 替代 `HashMap` 实现并发安全的数据存储
3. 优化 R-Tree 索引重建，避免频繁重建

---

## 4. 渲染帧率低

### 问题描述

缩放和拖动时 FPS 下降至约 5，渲染时间约 300ms。

### 解决方案

1. **零拷贝二进制协议**: 使用 `ArrayBuffer` 和 `DataView` 直接操作二进制数据，避免 JSON 序列化开销
2. **LOD 策略**: 根据缩放级别过滤节点数量
3. **批量渲染**: 使用 `ctx.beginPath()` 和 `ctx.stroke()` 批量绘制路径
4. **防抖数据请求**: 使用 300ms 防抖避免频繁的 IPC 调用

---

## 调试技巧

### 1. Canvas 尺寸验证

绘制固定像素大小的正方形验证 Canvas 渲染是否正确：

```typescript
ctx.strokeRect(-50, -50, 100, 100) // 应显示为正方形
```

### 2. 投影验证

绘制固定米数的墨卡托正方形验证投影是否正确：

```typescript
const testSize = 500 // 500米
const pt1 = this.mercatorToScreen(centerX - testSize, centerY - testSize)
const pt2 = this.mercatorToScreen(centerX + testSize, centerY + testSize)
// pt2.x - pt1.x 应等于 pt1.y - pt2.y
```

### 3. 初始化时机验证

在 `resize()` 中记录 Canvas 尺寸，确保初始化时获取的尺寸正确：

```typescript
console.log(`CSS: ${rect.width}x${rect.height}, Canvas: ${canvas.width}x${canvas.height}`)
```
