# 3D-RtT フェーズ1: RtTインフラ実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `phase1-rtt-infrastructure-plan-2026-03-15` |
| ステータス | `Draft` |
| 作成日 | `2026-03-15` |
| 最終更新日 | `2026-03-15` |
| 作成者 | Gemini CLI |
| 関連提案 | `docs/plans/3d-rtt/milestone-roadmap.md` |
| 関連Issue/PR | N/A |

## 1. 目的

- **解決したい課題**: 最終的なフル3D-RtT化への第一歩として、ロジック・既存描画を破壊することなく、3Dパイプラインを裏で構築し、最終的に2D側へテクスチャとして流し込む基盤を整える。
- **到達したい状態**: 既存の2Dビューに変化がなく、その裏でCamera3dがRtT（Render-to-Texture）でオフスクリーンレンダリングを行い、その結果のテクスチャを2Dカメラ上で表示（合成）できている状態。
- **成功指標**:
  - コンパイルおよび `cargo check` が通ること。
  - テスト用の3D立方体が2Dカメラのビュー上に正しく表示されること。
  - パン・ズーム操作時に2D・3D両方のカメラが同期し、表示のズレがないこと。

## 2. スコープ

### 対象（In Scope）

- `Cargo.toml` への `"3d"` フィーチャー追加
- `LAYER_2D`, `LAYER_3D` 定数の定義
- オフスクリーンテクスチャ（`Handle<Image>`）の生成と管理
- `Camera3d` (正射影) + `RenderTarget::Image` の初期化
- パン・ズーム時の `Camera2d` と `Camera3d` のTransform/Scale同期システムの追加
- テスト用3Dオブジェクト（`Cuboid` 等）の配置と、RtTテクスチャのフルスクリーン表示設定

### 非対象（Out of Scope）

- 実際のゲームアセット（地形・建築物・キャラクター）の3D化（Phase 2以降のスコープ）
- Zソート問題の解決・検証（Phase 2のスコープ）
- 矢視モードの完全な実装（本フェーズではカメラ操作基盤のみ）

## 3. 現状とギャップ

- **現状**: `Camera2d` のみで画面全体の描画を行っている。3D関連の機能は無効化されており、レイヤーの使い分けも存在しない。
- **問題**: 3Dと2Dのハイブリッドレンダリングを行うためのインフラが全くない。
- **本計画で埋めるギャップ**: Bevyの3D描画パイプラインを有効化し、Camera2d/3Dの二重管理とテクスチャ合成パイプラインを繋ぐ。

## 4. 実装方針（高レベル）

- **方針**: メインの2Dゲーム画面はそのまま維持し、背後で3Dシーンを別レイヤー（`LAYER_3D`）にレンダリングする。その結果を画像化し、2Dシーンのレイヤー（`LAYER_2D`）にスプライトとして表示する。
- **設計上の前提**: Bevy 0.18 の `RenderTarget::Image` と `RenderLayers` を使用する。
- **Bevy 0.18 APIでの注意点**:
  - `RenderTarget::Image` は `ImageRenderTarget { handle, scale_factor }` の形式で利用する。
  - オフスクリーン画像には `TextureUsages::RENDER_ATTACHMENT` の指定が必須。
  - `PanCamera` (0.18) のズームは `transform.scale` を更新するため、Camera3dへの同期も Transform の `scale` と `translation` に着目する。

---

## 5. マイルストーン

## M1: Bevy `3d` フィーチャー有効化 (MS-1A)

- **変更内容**: `Cargo.toml` （ワークスペースルートおよび `crates/bevy_app/Cargo.toml` 等）の `bevy` 依存関係に `"3d"` フィーチャーを追加する。
- **変更ファイル**:
  - `Cargo.toml`
  - `crates/bevy_app/Cargo.toml`
- **完了条件**:
  - [ ] フィーチャー追加後、`cargo check` がコンパイルエラーゼロで通過する。
- **検証**:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`

## M2: Camera3d + RenderTarget セットアップ (MS-1B)

- **変更内容**:
  - 描画レイヤーの定数を追加（`LAYER_2D = 0`, `LAYER_3D = 1`）。
  - オフスクリーンレンダリング用のテクスチャ（`Image`）を生成・初期化する。
  - `Camera3d` (正射影: `OrthographicProjection`) を追加し、`RenderTarget` を上記テクスチャに設定する。
- **変更ファイル**:
  - `crates/hw_core/src/constants/render.rs`
  - `crates/bevy_app/src/assets.rs` (あるいは新規テクスチャ管理モジュール)
  - `crates/bevy_app/src/plugins/startup/mod.rs`
- **完了条件**:
  - [ ] レイヤー定数が定義されている。
  - [ ] Camera3dが `RenderLayers::layer(LAYER_3D)` でセットアップされている。
  - [ ] オフスクリーン画像がアセットシステムに登録されている。
- **検証**:
  - `cargo check` の通過。

## M3: Camera2d ↔ Camera3d 同期システム (MS-1C)

- **変更内容**:
  - 毎フレーム、`Camera2d` の Transform (XY移動, ズームスケール) を取得し、`Camera3d` に反映させる同期システムを実装する。
  - 移動(Pan)の同期: 2DのXYを3DのXZ（あるいは視点方向に応じた適切な軸）にマップする。
  - ズーム(Zoom)の同期: 2Dの `transform.scale` を3Dカメラのスケールに適用するか、Projection設定を合わせる。
- **変更ファイル**:
  - `crates/bevy_app/src/systems/visual/camera_sync.rs` (新規作成または既存システムに追加)
  - `crates/bevy_app/src/systems/visual/mod.rs` (システム登録)
- **完了条件**:
  - [ ] システムが正しく登録・実行されている。
  - [ ] 同期処理によってコンパイルエラーが発生しない。
- **検証**:
  - `cargo check`。可能であればゲームを起動し、既存の2D操作に悪影響がないか確認。

## M4: RtTテクスチャのCamera2d合成と検証 (MS-1D)

- **変更内容**:
  - M2で作成したテクスチャを利用し、フルスクリーン用 `Sprite` コンポーネントを直接 spawn して `Camera2d` ビュー内の適正なZ位置（背面や手前などテストしやすい位置）に表示する（Bevy 0.18 では `SpriteBundle` は廃止済み）。
  - `Camera3d` の `clear_color` を `ClearColorConfig::Custom(Color::srgba(0., 0., 0., 0.))`（透明）にする（Bevy 0.18 に `Color::NONE` は存在しない）。
  - テスト用3Dオブジェクト（`Mesh3d` + `Cuboid` メッシュ）を `(0,0,0)` 周辺に配置し、`RenderLayers::layer(LAYER_3D)` を割り当てる（Bevy 0.18 では `PbrBundle` は廃止済み）。
- **変更ファイル**:
  - `crates/bevy_app/src/plugins/startup/mod.rs` (あるいは専用のデバッグ・初期化コード)
- **完了条件（継続可否ゲート）**:
  - [ ] テスト立方体が画面上に正しく表示され、2Dのパン・ズーム操作に追従する。
  - [ ] テレインや既存要素が透過して見え、表示が破綻していない。
  - [ ] フルRtT計画の継続可否（パフォーマンス・品質評価）を判断可能な状態になる。
- **検証**:
  - ゲームを起動し、目視による描画・同期の正確性を確認。

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| Bevy 0.18のRtT APIの仕様変更 | 中 | 実装時に公式ドキュメント（`docs.rs/bevy/0.18.0`）を参照し、`ImageRenderTarget` の正確な使用法を担保する。 |
| Camera同期時の座標軸ズレ | 大 (操作不能) | XZ平面かXY平面か、正射影の向きを明確に設計し、必要であればスケールやオフセットの変換式を導入する。 |
| パフォーマンス低下 | 大 | `Camera3d` の設定においてシャドウや不要なポストプロセスが無効になっているか確認し、最小構成でスタートする。 |

## 7. 検証計画

- **必須**:
  - 各マイルストーン毎に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` を実行。
- **手動確認シナリオ**:
  - M4完了後、ゲームを起動して「テスト立方体が表示されるか」「カメラをドラッグして移動した際に立方体も一緒に追従して動くか」「ホイールスクロールで拡大縮小した際に立方体のサイズも同期して変わるか」をチェックする。

## 8. ロールバック方針

- **どの単位で戻せるか**: フェーズ全体、または各マイルストーン単位での `git revert` が可能。
- **戻す時の手順**: 描画処理のみの変更であるため、追加したフィーチャー（`Cargo.toml`）やシステム（`app.add_systems`等）の登録解除、および追加ファイルの削除のみで元に戻る。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1〜M4

### 次のAIが最初にやること

1. `Cargo.toml` の `bevy` 依存に `"3d"` フィーチャーを追加し、`cargo check` を通す (M1)。
2. M2以降のレイヤー定義とオフスクリーンテクスチャの準備へ進む。

### ブロッカー/注意点

- ロジック層（`hw_core` 等）への変更は最小限（`LAYER`定数追加程度）に留め、既存のゲーム挙動に影響を与えないこと。

### 参照必須ファイル

- `docs/plans/3d-rtt/milestone-roadmap.md`
- `crates/bevy_app/src/plugins/startup/mod.rs`
- `crates/hw_core/src/constants/render.rs`

### 最終確認ログ

- 最終 `cargo check`: N/A
- 未解決エラー: N/A

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-15` | Gemini CLI | 初版作成 |
