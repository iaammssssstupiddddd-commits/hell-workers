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
  3. **万有引力のような逆二乗則ベースの物理演算** によって、右上の `DreamPoolIcon` へ向かって吸い込まれる

### 軌道・物理アルゴリズム (Physics V2)

以前のベジェ曲線による軌道計算から、純粋な物理・流体シミュレーションへと移行しました。

- **浮力と初速**: 発生直後は上方向の**浮力 (`DREAM_UI_BUOYANCY`)** とランダムな初速によって上へ飛びますが、浮力は発生から1.5秒で徐々に減衰してゼロになり、上辺に張り付くのを防ぎます。
- **空気抵抗**: 常に **Drag (`DREAM_UI_DRAG`)** が時間係数で掛かっており、極端な加速を防いで泡らしいもっさりとした動きを作ります。
- **引力（対数スケールと質量）**: 各泡は「**質量（Mass）**」を持っており、アイコンからの距離に対する対数関数カーブ（近接時に急加速するが上限がある）に従って引力 (`DREAM_UI_BASE_ATTRACTION`) が強まります。質量が大きい（合体して大きくなった）泡ほど優先的に強い力で吸い込まれます。
- **渦 (Vortex)**: 直線的に引っ張るだけでなく、接線方向の**渦の力 (`DREAM_UI_VORTEX_STRENGTH`)** が働きます。Y軸が下向きのUI座標に合わせて内側にカーブする螺旋状の軌道を描き、UI経由で吸い込まれる流体的な軌道を描きます。
- **ノイズ**: ランダムな角度への微細なノイズ力 (`DREAM_UI_NOISE_STRENGTH`) により、ふらふらとした揺らぎを表現しています。
- **画面端ストッパー (Clamp & Damping)**: 泡が画面外へ吹き飛んだり通り過ぎたりしないよう、画面端へ到達した際は速度の飛び出し成分のみを強烈に減衰させ、枠内に留まるようにクランプされます。
- **視覚的変化**:
  - **サイズ縮小**: 発生時間の長さによらず、ターゲットに近づくにつれて対数関数的に収縮します。ベースサイズは質量（Mass）の平方根に比例して大きくなります。
  - **Squash & Stretch**: 移動速度に応じて進行方向に伸び、垂直方向に縮みます。
  - **色と透明度**: 生成直後のアルファ値フェードインを除き、アイコンに近づくほど白く発光するように色が変化します。
  - **合体 (Merge)**: 泡同士が近づいた場合、バネ的な引力で互いに引き寄せ合って合体し、質量が増加します。

**表示順**: UI文字やパネルよりも確実に手前に表示されるよう、パーティクルは `ZIndex(100)`、軌跡は `ZIndex(99)` に設定されています。

実装: [`ui_particle.rs`](../src/systems/visual/dream/ui_particle.rs) 

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
| `DREAM_PARTICLE_MAX_PER_SOUL` | 5 | Soul ごとの通常粒子上限 |
| `DREAM_POPUP_THRESHOLD` | 0.08 | `+Dream` UI発生閾値 |
| `DREAM_UI_PARTICLE_SIZE` | 10.0 | 吸い込まれる泡の基本サイズ |
| `DREAM_UI_BUOYANCY` | 45.0 | 上方向への浮力（最大値。発生から1.5秒でゼロへ減衰） |
| `DREAM_UI_BASE_ATTRACTION` | 25.0 | アイコンへの基本引力（距離の対数カーブと質量で増幅される） |
| `DREAM_UI_VORTEX_STRENGTH` | 3.0 | 横向きにそれる渦巻き力の強さ |
| `DREAM_UI_DRAG` | 0.88 | 空気抵抗（1.0未満。小さいほどすぐ減速してもっさりする） |
| `DREAM_UI_NOISE_STRENGTH` | 60.0 | 揺らぎ・ふらつきの強さ |
| `DREAM_UI_NOISE_INTERVAL` | 0.3 | ふらつきの方向が変わる間隔（秒） |
| `DREAM_UI_BOUNDARY_MARGIN` | 30.0 | 画面端からの距離（これ以上近づくと斥力が働く） |
| `DREAM_UI_BOUNDARY_PUSH` | 150.0 | 画面端の斥力の強さ |
| `DREAM_UI_ARRIVAL_RADIUS` | 40.0 | アイコンに吸い込まれたと判定する距離半径 |
| `DREAM_UI_TRAIL_INTERVAL` | 0.12 | 残像（TrailGhost）を生成する間隔（秒） |
| `DREAM_UI_TRAIL_LIFETIME` | 0.15 | 残像が消えるまでの時間（秒） |
| `DREAM_UI_PULSE_DURATION` | 0.35 | アイコンが脈打つ演出の時間 |
| `DREAM_UI_MERGE_RADIUS` | 20.0 | 泡同士が吸い寄り合体する距離 |
