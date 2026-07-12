use std::collections::HashSet;

use bevy::prelude::*;

use super::polyline::world_to_corner_key;
use super::types::BoundaryPolyline;

/// ハッシュベースの 1D 値ノイズ（[-1.0, 1.0]）。
fn value_noise_1d(t: f32, seed: u32) -> f32 {
    let i = t.floor() as i32;
    let f = t.fract();
    let f = f * f * (3.0 - 2.0 * f); // smoothstep
    let v0 = hash_to_f32(i, seed);
    let v1 = hash_to_f32(i + 1, seed);
    v0 + (v1 - v0) * f
}

fn hash_to_f32(i: i32, seed: u32) -> f32 {
    let h = (i as u32).wrapping_mul(2_654_435_761).wrapping_add(seed);
    let h = h ^ (h >> 16);
    let h = h.wrapping_mul(0x45d9f3b);
    let h = h ^ (h >> 16);
    (h as f32 / u32::MAX as f32) * 2.0 - 1.0
}

#[inline]
fn mix64(z: u64) -> u64 {
    let mut x = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

#[inline]
fn u64_to_unit_f32(x: u64) -> f32 {
    (((x >> 8) & 0xFF_FFFF) as f32) * (1.0 / 16_777_216.0)
}

/// ポリラインごとのノイズパラメータ（`master_seed` と幾何から決定論的に導出）。
///
/// 種別だけ XOR していた頃と違い、**同じ `BoundaryKind` の複数線**でも別シード・別位相になる。
#[derive(Clone, Copy, Debug)]
pub struct PolylineNoiseParams {
    /// `value_noise_1d` のシード。
    seed: u32,
    /// 弧長座標への位相オフセット（同じ全長でも波形をずらす）。
    arc_phase: f32,
    /// 基準周波数に掛る倍率。
    freq_scale: f32,
}

pub fn boundary_polyline_noise_params(
    master_seed: u64,
    polyline: &BoundaryPolyline,
) -> PolylineNoiseParams {
    let mut h = mix64(master_seed);
    h ^= mix64(polyline.kind.index() as u64);
    h ^= mix64(polyline.points.len() as u64);
    h ^= mix64(if polyline.is_closed {
        0xC001_D00D_C0DE_u64
    } else {
        0x5EED_FACE_u64
    });

    if let Some(p0) = polyline.points.first() {
        let k = world_to_corner_key(*p0);
        h ^= mix64((k.0 as u64).wrapping_shl(32) ^ (k.1 as u32 as u64));
    }
    if polyline.points.len() > 1
        && let Some(pl) = polyline.points.last()
    {
        let k = world_to_corner_key(*pl);
        h ^= mix64((k.0 as u64).wrapping_shl(16) ^ (k.1 as u32 as u64).wrapping_shl(48));
    }
    if let Some(pm) = polyline.points.get(polyline.points.len() / 2) {
        let k = world_to_corner_key(*pm);
        h ^= mix64(k.0 as u64 ^ (k.1 as u64).wrapping_shl(32));
    }
    if let Some(total) = polyline.arc_lengths.last() {
        let q = (*total * 1000.0) as u64;
        h ^= mix64(q);
    }

    let h1 = mix64(h);
    let h2 = mix64(h.wrapping_add(0x9e37_79b9_7f4a_7c15));
    let h3 = mix64(h.wrapping_add(0xc6bc_2796_92b5_c323));

    PolylineNoiseParams {
        seed: (h1 ^ (h1 >> 32)) as u32,
        arc_phase: u64_to_unit_f32(h2) * 800.0,
        freq_scale: 0.82 + u64_to_unit_f32(h3) * 0.36,
    }
}

/// ポリラインの各制御点を法線方向にノイズ変位した点列を返す。
///
/// `junctions` に含まれるコーナー（全境界グラフで次数 ≥ 3）は変位 0 とし、三叉路で帯が割れないようにする。
pub fn displace_polyline(
    polyline: &BoundaryPolyline,
    noise: &PolylineNoiseParams,
    base_freq: f32,
    amplitude: f32,
    junctions: &HashSet<(i32, i32)>,
) -> Vec<Vec2> {
    let freq = base_freq * noise.freq_scale;
    let points = &polyline.points;
    let arcs = &polyline.arc_lengths;
    let n = points.len();
    let mut result = Vec::with_capacity(n);

    for i in 0..n {
        let key = world_to_corner_key(points[i]);
        let tangent = compute_tangent(points, i, polyline.is_closed);
        let normal = Vec2::new(-tangent.y, tangent.x);
        let displacement = if junctions.contains(&key) {
            0.0
        } else {
            let t = arcs[i] * freq + noise.arc_phase;
            value_noise_1d(t, noise.seed) * amplitude
        };
        result.push(points[i] + normal * displacement);
    }

    result
}

/// ノイズ変位済み点列の鋭角コーナーを面取り（Chamfer）し、
/// Catmull-Rom スプラインのオーバーシュートを抑制した新しい点列を返す。
///
/// 各コーナーを 2 つのベベル点で置換する：
/// - `bevel1 = p - t * d_in`  （コーナー手前）
/// - `bevel2 = p + t * d_out` （コーナー直後）
///
/// 以下の頂点は変更しない：
/// - 開ポリラインの端点
/// - `junctions` に含まれるコーナー（三叉路点: 変位 0 で元座標にある）
/// - 内角が `cos_threshold` 以上の緩やかな曲がり（面取り不要）
pub(crate) fn chamfer_polyline_points(
    points: &[Vec2],
    is_closed: bool,
    junctions: &HashSet<(i32, i32)>,
    t: f32,
    cos_threshold: f32,
) -> Vec<Vec2> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }

    let mut result = Vec::with_capacity(n + n / 3);

    for i in 0..n {
        let p = points[i];

        // 開ポリラインの端点は変更しない
        if !is_closed && (i == 0 || i == n - 1) {
            result.push(p);
            continue;
        }

        // ジャンクション頂点は変更しない（displace_polyline で変位=0 なので元の grid 座標にある）
        if junctions.contains(&world_to_corner_key(p)) {
            result.push(p);
            continue;
        }

        let prev_i = if i == 0 { n - 1 } else { i - 1 };
        let next_i = if i == n - 1 { 0 } else { i + 1 };

        let d_in = (p - points[prev_i]).normalize_or_zero();
        let d_out = (points[next_i] - p).normalize_or_zero();

        // 内角コサインが cos_threshold より小さい（より鋭い）コーナーのみ面取り
        if d_in.dot(d_out) < cos_threshold {
            result.push(p - t * d_in); // コーナー手前
            result.push(p + t * d_out); // コーナー直後
        } else {
            result.push(p);
        }
    }

    result
}

/// 点列の i 番目における接線方向を返す（中央差分、端点は前後向き差分）。
fn compute_tangent(points: &[Vec2], i: usize, is_closed: bool) -> Vec2 {
    let n = points.len();
    if n < 2 {
        return Vec2::X;
    }
    if i == 0 {
        if is_closed {
            (points[1] - points[n - 1]).normalize_or_zero()
        } else {
            (points[1] - points[0]).normalize_or_zero()
        }
    } else if i == n - 1 {
        if is_closed {
            // 中央差分の wrap: 次は points[0]（閉曲線の先頭へ戻る）
            (points[0] - points[n - 2]).normalize_or_zero()
        } else {
            (points[n - 1] - points[n - 2]).normalize_or_zero()
        }
    } else {
        (points[i + 1] - points[i - 1]).normalize_or_zero()
    }
}

/// Catmull-Rom スプライン補間で密な点列を生成する。
///
/// - 開チェーン: 両端に外挿ゴースト点を付加して全セグメントを補間する
/// - 閉ループ: 末尾/先頭の点を折り返してゴーストとし、接続を滑らかにする
pub fn sample_catmull_rom(points: &[Vec2], is_closed: bool, steps_per_segment: u32) -> Vec<Vec2> {
    let n = points.len();
    if n < 2 || steps_per_segment == 0 {
        return points.to_vec();
    }

    // ゴースト点を含む拡張制御点列を構築（長さ = n + 3）
    let extended: Vec<Vec2> = if is_closed {
        // 先頭に p_{n-1}、末尾に p0, p1 を追加
        let mut v = Vec::with_capacity(n + 3);
        v.push(points[n - 1]);
        v.extend_from_slice(points);
        v.push(points[0]);
        v.push(points[1]);
        v
    } else {
        // 先頭に外挿ゴースト、末尾に外挿ゴーストを追加
        let ghost_start = 2.0 * points[0] - points[1];
        let ghost_end = 2.0 * points[n - 1] - points[n - 2];
        let mut v = Vec::with_capacity(n + 2);
        v.push(ghost_start);
        v.extend_from_slice(points);
        v.push(ghost_end);
        v
    };

    let num_segments = if is_closed { n } else { n - 1 };
    let mut result = Vec::with_capacity(num_segments * steps_per_segment as usize + 1);

    for seg in 0..num_segments {
        let (p0, p1, p2, p3) = (
            extended[seg],
            extended[seg + 1],
            extended[seg + 2],
            extended[seg + 3],
        );
        for step in 0..steps_per_segment {
            let t = step as f32 / steps_per_segment as f32;
            result.push(catmull_rom_point(p0, p1, p2, p3, t));
        }
    }

    // 開チェーン: 末尾制御点を追加
    // 閉ループ: 先頭サンプル点を複製して完全に閉じる
    if is_closed {
        if let Some(&first) = result.first() {
            result.push(first);
        }
    } else {
        result.push(*points.last().expect("points is non-empty"));
    }

    result
}

fn catmull_rom_point(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use bevy::prelude::Vec2;

    use super::super::extract::BoundaryKind;
    use super::super::polyline::{parameterize_arc_length, world_to_corner_key};
    use super::super::types::BoundaryPolyline;
    use super::{boundary_polyline_noise_params, displace_polyline, value_noise_1d};

    #[test]
    fn value_noise_is_deterministic_for_seed() {
        let a = value_noise_1d(12.5, 42);
        let b = value_noise_1d(12.5, 42);
        assert_eq!(a, b);
        assert_ne!(value_noise_1d(12.5, 43), a);
    }

    #[test]
    fn displace_polyline_zero_at_junction() {
        let points = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(32.0, 0.0),
            Vec2::new(64.0, 0.0),
        ];
        let junction_key = world_to_corner_key(points[1]);
        let polyline = BoundaryPolyline {
            points: points.clone(),
            arc_lengths: parameterize_arc_length(&points),
            is_closed: false,
            kind: BoundaryKind::GrassDirt,
        };
        let noise = boundary_polyline_noise_params(123, &polyline);
        let junctions = HashSet::from([junction_key]);
        let displaced = displace_polyline(&polyline, &noise, 1.0, 12.0, &junctions);
        assert_eq!(displaced[1], points[1]);
    }
}
