use bevy::prelude::*;

use super::extract::BoundaryKind;

/// 連続した境界ポリライン。開チェーンと閉ループの両方を表現する。
#[derive(Debug, Clone)]
pub struct BoundaryPolyline {
    pub points: Vec<Vec2>,
    /// 累積弧長テーブル（points と同じ長さ、先頭は 0.0）。
    pub arc_lengths: Vec<f32>,
    pub is_closed: bool,
    pub kind: BoundaryKind,
}

/// 境界リボンが影響するグリッドセルのインデックス。
///
/// PostStartup で build し、将来の TerrainChangedEvent 対応の基盤として使用する。
#[derive(Resource, Default)]
pub struct BoundarySliceSpatialIndex;
