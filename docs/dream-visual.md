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

### 描画方式

`Mesh2d` + `DreamBubbleMaterial`（カスタム `Material2d`）によるシェーダー描画。
シェーダー（`assets/shaders/dream_bubble.wgsl`）はソフトグロー・虹色屈折・スペキュラハイライト・リム発光・ノイズ変形を実装しており、質量（mass）に応じて変形の強さが変わる。

### 睡眠中の Soul からの発生

- 睡眠中かつ `DreamQuality != Awake` の Soul に発生
- 品質ごとに間隔・寿命・色・揺れ量が変化
- Soul ごとの同時粒子数は `DREAM_PARTICLE_MAX_PER_SOUL` で制限
- `NightTerror` でも粒子は発生（赤系）

### 休憩所 (RestArea) からの一括発生

- 休憩中の Soul 個別の状態によらず、休憩所エンティティから一括でパーティクルが発生
- パーティクルの大きさ、動きの激しさは**「現在何人休憩しているか（Occupants）」**に比例してスケールアップする
- **泡の発生間隔はSoul睡眠時と同じ (`DREAM_POPUP_INTERVAL`: 0.5秒) のまま**、人数が増えるほど泡1つあたりの「質量（Mass）」が大きくなる
- `VividDream` 品質（青色系）として描画され、活発に湧き出る視覚効果となる

## 3. Dream 獲得 UI パーティクル（`dream_popup_spawn_system`）

> 今後拡張予定のため [`gain_visual.rs`](../src/systems/visual/dream/gain_visual.rs) に独立モジュールとして配置しています。

### 描画方式

`MaterialNode<DreamBubbleUiMaterial>`（カスタム `UiMaterial`）によるシェーダー描画。
シェーダー（`assets/shaders/dream_bubble_ui.wgsl`）はWorld空間用と同様のエフェクトに加え、質量に応じたバブルクラスター表現を持つ：
- `mass < 3.0`: 1泡（ノイズ変形のみ）
- `mass < 6.0`: 2泡クラスター
- `mass >= 6.0`: 3泡クラスター（三角形配置）

各サブ泡は独立した輪郭線（リム発光）を持つ。マテリアルの uniform（`color`, `alpha`, `time`, `mass`, `velocity_dir`）は毎フレーム物理演算の結果から更新される。

### 生成条件

- 睡眠中 Soul が一定間隔 (`DREAM_POPUP_INTERVAL`) ごとに獲得したDream量をチェックし、その蓄積が `DREAM_POPUP_THRESHOLD` を超えていた場合に:
  1. 到達しなかった蓄積値は次回の判定へ持ち越される
  2. World 座標から `camera.world_to_viewport` で画面座標へ変換
  3. UI ルートノードの子として `DreamGainUiParticle` を生成。生成時の質量(Mass)は獲得Dream量と厳密に一致する
  4. **万有引力のような逆二乗則ベースの物理演算** によって、右上の `DreamPoolIcon` へ向かって吸い込まれる

### 軌道・物理アルゴリズム (Physics V2)

以前のベジェ曲線による軌道計算から、純粋な物理・流体シミュレーションへと移行しました。

- **浮力と初速**: 発生直後は上方向の**浮力 (`DREAM_UI_BUOYANCY`)** とランダムな初速によって上へ飛びますが、浮力は発生から1.5秒で徐々に減衰してゼロになり、上辺に張り付くのを防ぎます。
- **空気抵抗**: 常に **Drag (`DREAM_UI_DRAG`)** が時間係数で掛かっており、極端な加速を防いで泡らしいもっさりとした動きを作ります。
- **引力（対数スケールと質量）**: 各泡は「**質量（Mass）**」を持っており、アイコンからの距離に対する対数関数カーブ（近接時に急加速するが上限がある）に従って引力 (`DREAM_UI_BASE_ATTRACTION`) が強まります。質量が大きい（合体して大きくなった）泡ほど優先的に強い力で吸い込まれます。
- **渦 (Vortex)**: 直線的に引っ張るだけでなく、接線方向の**渦の力 (`DREAM_UI_VORTEX_STRENGTH`)** が働きます。Y軸が下向きのUI座標に合わせて内側にカーブする螺旋状の軌道を描きます。近づくほど直進性が高まり、**また泡が成長して巨大になるほど自重で軌道が安定（渦の影響が減少）し、円軌道に入ってスタックするのを防ぎます**。
- **ノイズ**: ランダムな角度への微細なノイズ力 (`DREAM_UI_NOISE_STRENGTH`) により、ふらふらとした揺らぎを表現しています。
- **画面端ストッパー (Clamp & Damping)**: 泡が画面外へ吹き飛んだり通り過ぎたりしないよう、画面端へ到達した際は速度の飛び出し成分のみを強烈に減衰させ、枠内に留まるようにクランプされます。
- **異常値救済措置 (Failsafe Rescue)**: 計算の際どいタイミングですり抜けて画面外に大きく吹っ飛んでしまった非常事態（100px以上画面外に出た場合）には、異常値となった反対側の座標（例: 左に飛び出たら右端、下に飛び出たら上端）へワープさせるフェイルセーフを実装しています。
- **スタック防止 (Minimum velocity)**: 画面左端などで引力と壁の斥力が釣り合って極端に減速・停止してしまうのを防ぐため、常にアイコン方向へ向かう一定の最低速度（`min_speed`）ベクトルを保証し、最終的に必ず到達するようにしています。
- **視覚的変化**:
  - **サイズ縮小**: 発生時間の長さによらず、ターゲットに近づくにつれて対数関数的に収縮します。ベースサイズは「質量（Mass）+ 基本値 (`DREAM_UI_BASE_MASS_OFFSET`)」の平方根に比例して大きくなります。
  - **Squash & Stretch**: 移動速度に応じて進行方向に伸び、垂直方向に縮みます。
  - **色と透明度**: 生成直後のアルファ値フェードインを除き、アイコンに近づくほど白く発光するように色が変化します。
  - **合体 (Merge)**: 泡同士が近づいた場合、バネ的な引力で互いに引き寄せ合って合体し、質量が増加します。半径は質量の平方根に比例するため、**合体前後で描画される泡の面積の総和は完全に保たれます**。また、極端な巨大化を防ぐため合体回数や「質量の絶対値（Mass > 12.0）」のハードリミットが設けられています。

**表示順**: UI文字やパネルよりも確実に手前に表示されるよう、パーティクルは `ZIndex(100)`、軌跡は `ZIndex(99)` に設定されています。

実装: [`ui_particle.rs`](../src/systems/visual/dream/ui_particle.rs) 

## 4. `+Dream` ポップアップ (`dream_popup_*`)

- 睡眠中 Soul が 0.5秒おきに判定され、累積が `DREAM_POPUP_THRESHOLD` を超えるたびに持っているすべての値を消費して `+Dream` 浮遊テキストを生成
- 十分な閾値に満たない場合でも、睡眠状態から別のアクションに移った際（起きるなど）に、蓄積しているDreamがあれば残さず全て消費して生成される
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
| `src/systems/visual/dream/dream_bubble_material.rs` | `DreamBubbleMaterial`（World用 Material2d）・`DreamBubbleUiMaterial`（UI用 UiMaterial）定義 |
| `assets/shaders/dream_bubble.wgsl` | World 空間用フラグメントシェーダー |
| `assets/shaders/dream_bubble_ui.wgsl` | UI 空間用フラグメントシェーダー（バブルクラスター対応） |
| `src/interface/ui/setup/time_control.rs` | Dream テキストノード生成 |
| `src/interface/ui/interaction/status_display.rs` | Dream 表示更新とパルス演出 |
| `src/interface/ui/presentation/builders.rs` | RestArea ツールチップの Dream/s 表示 |

## 7. 定数（ビジュアル関連）

| 定数 | 値 | 用途 |
| :--- | :--- | :--- |
| `DREAM_PARTICLE_MAX_PER_SOUL` | 5 | Soul ごとの通常粒子上限 |
| `DREAM_POPUP_INTERVAL` | 0.5 | 泡生成の判定を行う間隔（秒） |
| `DREAM_POPUP_THRESHOLD` | 0.5 | `+Dream` UI発生および泡生成の閾値 |
| `DREAM_UI_PARTICLE_SIZE` | 14.14 | 吸い込まれる泡の基本サイズ（質量1.0のときのサイズ。面積ベースで調整） |
| `DREAM_UI_BUOYANCY` | 110.0 | 上方向への浮力（最大値。発生から1.5秒でゼロへ減衰） |
| `DREAM_UI_BASE_ATTRACTION` | 50.0 | アイコンへの基本引力（距離の対数カーブと質量で増幅される） |
| `DREAM_UI_BASE_MASS_OFFSET` | 1.0 | 質量に加算する基本値（少量のDream獲得時でも必要な移動速度と大きさを担保・下駄） |
| `DREAM_UI_VORTEX_STRENGTH` | 3.0 | 横向きにそれる渦巻き力の強さ |
| `DREAM_UI_DRAG` | 0.85 | 空気抵抗（1.0未満。小さいほどすぐ減速してもっさりする） |
| `DREAM_UI_NOISE_STRENGTH` | 120.0 | 揺らぎ・ふらつきの強さ |
| `DREAM_UI_NOISE_INTERVAL` | 0.3 | ふらつきの方向が変わる間隔（秒） |
| `DREAM_UI_BOUNDARY_MARGIN` | 30.0 | 画面端からの距離（これ以上近づくと斥力が働く） |
| `DREAM_UI_BOUNDARY_PUSH` | 300.0 | 画面端の斥力の強さ |
| `DREAM_UI_ARRIVAL_RADIUS` | 40.0 | アイコンに吸い込まれたと判定する距離半径 |
| `DREAM_UI_TRAIL_INTERVAL` | 0.12 | 残像（TrailGhost）を生成する間隔（秒） |
| `DREAM_UI_TRAIL_LIFETIME` | 0.15 | 残像が消えるまでの時間（秒） |
| `DREAM_UI_PULSE_DURATION` | 0.35 | アイコンが脈打つ演出の時間 |
| `DREAM_UI_MERGE_RADIUS` | 40.0 | 泡同士が吸い寄り合体する距離 |
| `DREAM_UI_MERGE_MAX_COUNT` | 8 | 1つの泡が合体できる回数の上限 |
| `DREAM_UI_MERGE_DURATION` | 0.25 | 合体にかかる時間（秒） |
