use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, TILE_SIZE};

use super::extract::{BoundaryEdge, BoundaryKind};
use super::types::BoundaryPolyline;

/// 抽出済み境界エッジ全体で、**3 本以上**の辺が接するグリッドコーナー（多地形の三叉路など）。
///
/// 各ポリラインが別シードの法線ノイズを受けると、同一点が幾何的にずれて継ぎ目が空く。
/// これらのコーナーでは変位を 0 にし、全種別で同一座標に固定する。
pub(crate) fn boundary_junction_corner_keys(edges: &[BoundaryEdge]) -> HashSet<(i32, i32)> {
    let mut deg: HashMap<(i32, i32), u32> = HashMap::new();
    for e in edges {
        *deg.entry(world_to_corner_key(e.a)).or_insert(0) += 1;
        *deg.entry(world_to_corner_key(e.b)).or_insert(0) += 1;
    }
    deg.into_iter()
        .filter(|&(_, c)| c >= 3)
        .map(|(k, _)| k)
        .collect()
}

/// ワールド座標 Vec2 をグリッドコーナーインデックス (i32, i32) に変換する。
///
/// すべての境界エッジ端点は TILE_SIZE の倍数のグリッドコーナーに位置するため、
/// round() で一意な整数キーが得られる（浮動小数点等値比較を回避）。
pub(crate) fn world_to_corner_key(p: Vec2) -> (i32, i32) {
    let cx = (p.x / TILE_SIZE + MAP_WIDTH as f32 / 2.0).round() as i32;
    let cy = (p.y / TILE_SIZE + MAP_HEIGHT as f32 / 2.0).round() as i32;
    (cx, cy)
}
/// BoundaryEdge のリストを連続ポリライン群（開チェーンと閉ループ）に変換する。
pub fn chain_edges_to_polylines(edges: Vec<BoundaryEdge>) -> Vec<BoundaryPolyline> {
    // 種別ごとにエッジをグループ化
    let mut by_kind: HashMap<BoundaryKind, Vec<BoundaryEdge>> = HashMap::new();
    for e in edges {
        by_kind.entry(e.kind).or_default().push(e);
    }

    let mut result = Vec::new();
    for (kind, kind_edges) in by_kind {
        let n = kind_edges.len();
        let corner_keys: Vec<[(i32, i32); 2]> = kind_edges
            .iter()
            .map(|e| [world_to_corner_key(e.a), world_to_corner_key(e.b)])
            .collect();

        // コーナー → [エッジインデックス] の隣接マップ
        let mut adj: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, keys) in corner_keys.iter().enumerate() {
            adj.entry(keys[0]).or_default().push(i);
            adj.entry(keys[1]).or_default().push(i);
        }

        let mut visited = vec![false; n];

        // degree-1 コーナー（開チェーンの端点）から処理
        let chain_starts: Vec<(i32, i32)> = adj
            .iter()
            .filter(|(_, es)| es.len() == 1)
            .map(|(k, _)| *k)
            .collect();

        for start_key in chain_starts {
            let first = match adj[&start_key].iter().find(|&&i| !visited[i]) {
                Some(&i) => i,
                None => continue,
            };
            let (points, _first_forward) = follow_chain(
                start_key,
                first,
                &kind_edges,
                &corner_keys,
                &adj,
                &mut visited,
            );
            if points.len() >= 2 {
                let arc_lengths = parameterize_arc_length(&points);
                result.push(BoundaryPolyline {
                    points,
                    arc_lengths,
                    is_closed: false,
                    kind,
                });
            }
        }

        // 残る未訪問エッジ → 閉ループ
        for start_idx in 0..n {
            if visited[start_idx] {
                continue;
            }
            let start_key = corner_keys[start_idx][0];
            let (mut points, _first_forward) = follow_chain(
                start_key,
                start_idx,
                &kind_edges,
                &corner_keys,
                &adj,
                &mut visited,
            );
            trim_closed_polyline_duplicate_end(&mut points);
            // 閉じた単純ループは少なくとも 3 頂点（重複除去後）。
            if points.len() >= 3 {
                let arc_lengths = parameterize_arc_length(&points);
                result.push(BoundaryPolyline {
                    points,
                    arc_lengths,
                    is_closed: true,
                    kind,
                });
            }
        }
    }

    result
}

/// 閉ループ走査では始点コーナーが **先頭と末尾の両方** に入る（`follow_chain` が一周して戻るため）。
/// `p[0] == p[n-1]` のままだと、メッシュ側のセグメント `p[n-1] → p[0]` が長さ 0 になり
/// 閉じるクワッドが落ち、継ぎ目だけ鋭角に見える。末尾の重複を除いて真の環状点列にする。
fn trim_closed_polyline_duplicate_end(points: &mut Vec<Vec2>) {
    if points.len() < 2 {
        return;
    }
    let last = points.len() - 1;
    if points[0].distance_squared(points[last]) < 1e-10 {
        points.pop();
    }
}

/// 指定コーナーから始まる連続チェーンを辿り、点列と「最初のエッジを順方向（a→b）で辿ったか」を返す。
fn follow_chain(
    start_key: (i32, i32),
    first_edge_idx: usize,
    edges: &[BoundaryEdge],
    corner_keys: &[[(i32, i32); 2]],
    adj: &HashMap<(i32, i32), Vec<usize>>,
    visited: &mut [bool],
) -> (Vec<Vec2>, bool) {
    let mut points = Vec::new();
    let mut cur_key = start_key;
    let mut cur_edge_idx = first_edge_idx;
    let mut first_forward = true;

    loop {
        visited[cur_edge_idx] = true;
        let [ka, kb] = corner_keys[cur_edge_idx];
        let edge = &edges[cur_edge_idx];

        if points.is_empty() {
            if ka == cur_key {
                first_forward = true;
                points.push(edge.a);
                points.push(edge.b);
                cur_key = kb;
            } else {
                first_forward = false;
                points.push(edge.b);
                points.push(edge.a);
                cur_key = ka;
            }
        } else if ka == cur_key {
            points.push(edge.b);
            cur_key = kb;
        } else {
            points.push(edge.a);
            cur_key = ka;
        }

        match adj
            .get(&cur_key)
            .and_then(|es| es.iter().find(|&&i| !visited[i]))
            .copied()
        {
            Some(next_idx) => cur_edge_idx = next_idx,
            None => break,
        }
    }

    (points, first_forward)
}

/// 点列の累積弧長テーブルを構築する（先頭は 0.0、points と同じ長さ）。
pub fn parameterize_arc_length(points: &[Vec2]) -> Vec<f32> {
    let mut arc = vec![0.0f32; points.len()];
    for i in 1..points.len() {
        arc[i] = arc[i - 1] + points[i].distance(points[i - 1]);
    }
    arc
}
