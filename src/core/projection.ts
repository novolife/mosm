/**
 * 投影转换
 *
 * Web Mercator (EPSG:3857) 坐标转换工具
 */

/** 投影转换：WGS84 经纬度 -> Web Mercator 像素坐标 */
export function lonLatToMercator(
  lon: number,
  lat: number,
  zoom: number,
): { x: number; y: number } {
  const scale = 256 * Math.pow(2, zoom)
  const x = ((lon + 180) / 360) * scale
  const latRad = (lat * Math.PI) / 180
  const y = ((1 - Math.log(Math.tan(latRad) + 1 / Math.cos(latRad)) / Math.PI) / 2) * scale
  return { x, y }
}

/** 批量投影转换 (适合渲染层) */
export function projectCoordinates(
  coords: Float64Array,
  zoom: number,
  centerX: number,
  centerY: number,
): Float32Array {
  const count = coords.length / 2
  const projected = new Float32Array(count * 2)
  const scale = 256 * Math.pow(2, zoom)

  for (let i = 0; i < count; i++) {
    const lon = coords[i * 2]
    const lat = coords[i * 2 + 1]
    const latRad = (lat * Math.PI) / 180

    projected[i * 2] = ((lon + 180) / 360) * scale - centerX
    projected[i * 2 + 1] =
      ((1 - Math.log(Math.tan(latRad) + 1 / Math.cos(latRad)) / Math.PI) / 2) * scale - centerY
  }

  return projected
}
