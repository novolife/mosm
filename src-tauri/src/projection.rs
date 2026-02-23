//! Web 墨卡托投影 (EPSG:3857)
//!
//! 将 WGS84 经纬度坐标转换为 Web 墨卡托投影坐标（单位：米）。
//! 这是 OpenStreetMap、Google Maps 等主流地图服务使用的投影标准。

use std::f64::consts::PI;

/// 地球赤道半周长（米）
/// 计算方式：地球半径 6378137m × π
const EARTH_HALF_CIRCUMFERENCE: f64 = 20037508.342789244;

/// 将 WGS84 经纬度转换为 Web 墨卡托坐标
///
/// # 参数
/// - `lon`: 经度（度，-180 到 180）
/// - `lat`: 纬度（度，-85.051129 到 85.051129）
///
/// # 返回
/// - `(x, y)`: 墨卡托坐标（米）
///
/// # 公式
/// - x = lon × (半周长 / 180)
/// - y = ln(tan((90 + lat) × π / 360)) × (半周长 / π)
#[inline]
pub fn lonlat_to_mercator(lon: f64, lat: f64) -> (f64, f64) {
    let x = lon * EARTH_HALF_CIRCUMFERENCE / 180.0;
    
    // 限制纬度范围，避免 tan 函数在极点附近产生无穷大
    let lat_clamped = lat.clamp(-85.051129, 85.051129);
    let lat_rad = (90.0 + lat_clamped) * PI / 360.0;
    let y = lat_rad.tan().ln() * EARTH_HALF_CIRCUMFERENCE / PI;
    
    (x, y)
}

/// 将 Web 墨卡托坐标转换回 WGS84 经纬度
///
/// # 参数
/// - `x`: 墨卡托 X 坐标（米）
/// - `y`: 墨卡托 Y 坐标（米）
///
/// # 返回
/// - `(lon, lat)`: 经纬度（度）
#[inline]
pub fn mercator_to_lonlat(x: f64, y: f64) -> (f64, f64) {
    let lon = x * 180.0 / EARTH_HALF_CIRCUMFERENCE;
    let lat = (2.0 * (y * PI / EARTH_HALF_CIRCUMFERENCE).exp().atan() - PI / 2.0) * 180.0 / PI;
    (lon, lat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_origin() {
        let (x, y) = lonlat_to_mercator(0.0, 0.0);
        assert!((x - 0.0).abs() < 1e-6);
        assert!((y - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_monaco() {
        // Monaco: 7.42°E, 43.74°N
        let (x, y) = lonlat_to_mercator(7.42, 43.74);
        // 预期值（使用 EPSG.io 或其他工具验证）
        // x ≈ 826,000 米
        // y ≈ 5,430,000 米
        assert!(x > 800_000.0 && x < 900_000.0);
        assert!(y > 5_400_000.0 && y < 5_500_000.0);
    }

    #[test]
    fn test_round_trip() {
        let lon = 7.42;
        let lat = 43.74;
        let (x, y) = lonlat_to_mercator(lon, lat);
        let (lon2, lat2) = mercator_to_lonlat(x, y);
        assert!((lon - lon2).abs() < 1e-10);
        assert!((lat - lat2).abs() < 1e-10);
    }

    #[test]
    fn test_aspect_ratio() {
        // 在同一纬度，0.01° 经度和 0.01° 纬度在墨卡托投影下
        // 应该产生相同的像素距离（因为墨卡托投影会放大高纬度）
        let lat = 43.74;
        let (x1, y1) = lonlat_to_mercator(7.42, lat);
        let (x2, _) = lonlat_to_mercator(7.43, lat);      // +0.01° 经度
        let (_, y2) = lonlat_to_mercator(7.42, lat + 0.01); // +0.01° 纬度
        
        let dx = (x2 - x1).abs();
        let dy = (y2 - y1).abs();
        
        // 在墨卡托投影中，dx 应该等于 dy（等角投影）
        // 实际上 dx/dy ≈ 1（考虑浮点误差）
        let ratio = dx / dy;
        assert!((ratio - 1.0).abs() < 0.01, "ratio = {}, expected ≈ 1.0", ratio);
    }
}
