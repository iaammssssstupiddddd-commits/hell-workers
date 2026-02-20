# Dream ビジュアルフィードバック

Dream システムの視覚的フィードバック実装についてのドキュメントです。  
コアロジック（蓄積・消費）については [dream.md](dream.md) を参照してください。

## 1. Soul スプライト色 (`idle_visual_system`)

### 通常睡眠 (`IdleBehavior::Sleeping`)

| 質 | 色 |
| :--- | :--- |
| `VividDream` | `(0.5, 0.6, 0.9, 1.0)` |
| `NightTerror` | `(0.8, 0.4, 0.4, 1.0)` |
| その他 | `(0.6, 0.6, 0.7, 1.0)` |

### 集会中睡眠 (`GatheringBehavior::Sleeping`)

| 質 | 色 |
| :--- | :--- |
| `VividDream` | `(0.5, 0.5, 0.9, 0.7)` |
| `NightTerror` | `(0.8, 0.4, 0.5, 0.6)` |
| その他 | `(0.6, 0.5, 0.8, 0.6)` |

## 2. Dream 粒子（World 空間）(`dream_particle_*`)

### 睡眠中の Soul からの発生

- 睡眠中かつ `DreamQuality != Awake` の Soul に発生
- 品質ごとに間隔・寿命・色・揺れ量が変化
- Soul ごとの同時粒子数は `DREAM_PARTICLE_MAX_PER_SOUL` で制限
- `NightTerror` でも粒子は発生（赤系）

### 休憩所 (RestArea) からの一括発生

- 休憩中の Soul 個別の状態によらず、休憩所エンティティから一括でパーティクルが発生
- パーティクルの大きさ、密度（生成間隔）、動きの激しさは**「現在何人休憩しているか（Occupants）」**に比例してスケールアップする
- `VividDream` 品質（青色系）として描画され、活発に湧き出る視覚効果となる

## 3. Dream 獲得 UI パーティクル（`dream_popup_spawn_system`）

> 今後拡張予定のため [`gain_visual.rs`](../src/systems/visual/dream/gain_visual.rs) に独立モジュールとして配置しています。

- 睡眠中 Soul が `DREAM_POPUP_THRESHOLD` を超えるたびに:
  1. World 座標から `camera.world_to_viewport` で画面座標へ変換
  2. UI ルートノードの子として `DreamGainUiParticle` を生成
  3. 4次ベジェ曲線（制御点 3つ）で右上の `DreamPoolIcon` へ向かって移動

### 軌道アルゴリズム

発生位置から最も近い画面端（上・右・下・左辺）を求め、その辺へ先に逃げ、辺に沿いながら目的地（`DreamPoolIcon`）まで到達する。

| 制御点 | 役割 |
| :--- | :--- |
| `c1` | 発生地点から最寄りの辺へ向かう |
| `c2` | 辺に貼り付いて目的地方向へ移動 |
| `c3` | 辺沿いでアイコン直前まで進む |

**表示順**: `ZIndex(-1)` により UI パネル・テキストより背面を通る。

実装: [`ui_particle.rs`](../src/systems/visual/dream/ui_particle.rs) の `calculate_control_points` 関数

### パターン選択ロジック

```
dist_up   = start_pos.y
dist_down = viewport_height - start_pos.y
dist_left = start_pos.x
dist_right = viewport_width - start_pos.x
→ 最小の dist に対応する辺へのパターンを選択
```

## 4. `+Dream` ポップアップ (`dream_popup_*`)

- `DreamVisualState.popup_accumulated += gain_rate * dt`
- 累積が `DREAM_POPUP_THRESHOLD` を超えるたびに `+Dream` 浮遊テキストを生成
- `NightTerror` は gain rate が 0 のため生成されない

## 5. Dream カウンター UI 表示

右上の時間コントロール領域（タスクサマリー下）に `Dream: X.X` を表示します。

- `UiSlot::DreamPoolText` でノード管理
- `update_dream_pool_display_system` が `DreamPool` 変更時に文言更新
- Dream 増加量が `DREAM_UI_PULSE_TRIGGER_DELTA` に達するごとにテキストを短時間発光
  - パルス時間: `DREAM_UI_PULSE_DURATION`
  - 明るさ係数: `DREAM_UI_PULSE_BRIGHTNESS`

補足:

- Building 情報パネルでも RestArea の現在 Dream 生成レートを  
  `Resting: current/capacity | Dream: x.xx/s` で表示

## 6. 関連ファイル

| ファイル | 内容 |
| :--- | :--- |
| `src/systems/soul_ai/visual/idle.rs` | 夢の質に応じた Soul 色変化 |
| `src/systems/visual/dream/gain_visual.rs` | **Dream 獲得 UI パーティクル・ポップアップ生成/更新（拡張予定）** |
| `src/systems/visual/dream/particle.rs` | Dream 粒子（World 空間）生成/更新 |
| `src/systems/visual/dream/ui_particle.rs` | UI パーティクル移動アニメーション・軌道計算 |
| `src/interface/ui/setup/time_control.rs` | Dream テキストノード生成 |
| `src/interface/ui/interaction/status_display.rs` | Dream 表示更新とパルス演出 |
| `src/interface/ui/presentation/builders.rs` | RestArea ツールチップの Dream/s 表示 |

## 7. 定数（ビジュアル関連）

| 定数 | 値 | 用途 |
| :--- | :--- | :--- |
| `DREAM_PARTICLE_MAX_PER_SOUL` | 5 | Soul ごとの同時粒子上限 |
| `DREAM_POPUP_THRESHOLD` | 0.08 | `+Dream` 表示の発生閾値 |
| `DREAM_UI_PULSE_TRIGGER_DELTA` | 0.05 | UI パルス発火に必要な増加量 |
| `DREAM_UI_PULSE_DURATION` | 0.35 | UI パルス時間（秒） |
