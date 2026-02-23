# 测试数据准备指南

## 推荐测试文件

从 Geofabrik 下载小型 PBF 文件进行测试：

### 小型文件 (< 10 MB) - 快速验证

| 地区 | 大小 | 下载链接 |
|------|------|----------|
| 梵蒂冈 | ~0.3 MB | https://download.geofabrik.de/europe/vatican-city-latest.osm.pbf |
| 摩纳哥 | ~0.5 MB | https://download.geofabrik.de/europe/monaco-latest.osm.pbf |
| 列支敦士登 | ~2 MB | https://download.geofabrik.de/europe/liechtenstein-latest.osm.pbf |
| 安道尔 | ~3 MB | https://download.geofabrik.de/europe/andorra-latest.osm.pbf |

### 中型文件 (10-50 MB) - 性能测试

| 地区 | 大小 | 下载链接 |
|------|------|----------|
| 卢森堡 | ~25 MB | https://download.geofabrik.de/europe/luxembourg-latest.osm.pbf |
| 塞浦路斯 | ~30 MB | https://download.geofabrik.de/europe/cyprus-latest.osm.pbf |

## 快速下载命令

```powershell
# 在项目根目录创建 testdata 文件夹
mkdir testdata

# 下载摩纳哥 (最小，适合首次测试)
Invoke-WebRequest -Uri "https://download.geofabrik.de/europe/monaco-latest.osm.pbf" -OutFile "testdata/monaco.osm.pbf"

# 或使用 curl
curl -o testdata/monaco.osm.pbf https://download.geofabrik.de/europe/monaco-latest.osm.pbf
```

## 测试步骤

1. 启动开发服务器:
   ```bash
   pnpm tauri dev
   ```

2. 点击侧边栏的 "打开 PBF 文件" 按钮

3. 选择下载的 `.osm.pbf` 文件

4. 观察:
   - 侧边栏显示加载的节点/路径/关系数量
   - 地图画布显示渲染的街道线条
   - 左上角显示 FPS 和渲染时间

## 摩纳哥测试数据预期

加载 `monaco.osm.pbf` 后预期看到:
- 约 25,000-30,000 节点
- 约 3,000-5,000 路径
- 约 200-400 关系

相机默认位置在北京 (116.4°E, 39.9°N)，需要手动调整视口到摩纳哥区域:
- 经度: 7.42° E
- 纬度: 43.73° N
- 缩放: 14-16

## 调试技巧

打开浏览器开发者工具 (F12)，在 Console 中可以看到:
- "PBF 数据加载完成，触发视口查询"
- "视口数据: X 节点, Y 路径"
