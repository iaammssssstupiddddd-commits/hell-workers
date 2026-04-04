# hw_core — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このクレートがやること）

- ゲーム全体で共有される**基盤型・定数・ECS Relationship・システムセット定義**の提供
- `DamnedSoul` / `Familiar` / `GatheringSpot` 等のコンポーネント型定義
- `GameSystemSet` / `FamiliarAiSystemSet` / `SoulAiSystemSet` の定義
- `events.rs` でのクレート間通信用メッセージ・Observer イベント型定義
- `visual_mirror/` での視覚状態ミラー型定義（実際の視覚演出は `hw_visual` が担う）
- `constants/` でのドメイン別定数集約

## 禁止事項（AI がやってはいけないこと）

- **他の hw_* クレートへの依存追加禁止**（hw_core は最下層。hw_* の中で唯一 bevy + rand のみに依存する）
- **システム実装・スポーン処理をこのクレートに書かない**（型定義・定数・定義のみ）
- **Bevy への登録（`app.register_type()` / `app.add_systems()`）をこのクレートに書かない**（登録は bevy_app 側）
- **`#[allow(dead_code)]` を使用しない**
- **Bevy 0.14 以前の API を推測で使わない**

## crate 境界ルール（docs/crate-boundaries.md に基づく）

- **最下層クレート**：hw_* の中で最初に依存される基盤
- 他の hw_* クレートへの逆依存は **完全禁止**
- 型定義・定数のみを持ち、ゲームロジックを持たない
- 詳細: [docs/crate-boundaries.md](../../docs/crate-boundaries.md)

## 依存制約（Cargo.toml 実体）

```
# 許可
bevy  ✓
rand  ✓

# 禁止（全 hw_* クレートへの依存禁止）
hw_energy      ✗
hw_jobs        ✗
hw_logistics   ✗
hw_soul_ai     ✗
hw_familiar_ai ✗
hw_spatial     ✗
hw_ui          ✗
hw_visual      ✗
hw_world       ✗
bevy_app       ✗
```

## hw_core に置くもの / 置かないもの

| hw_core に置くもの | src/ (bevy_app) に置くもの |
|---|---|
| コンポーネント型定義 | `#[reflect]` 登録・`init_resource` |
| `GameSystemSet` 等の定義 | `.configure_sets()` による配線 |
| 定数値 (`constants/`) | 定数を使うシステム実装 |
| `Message` 型定義 | `add_message::<T>()` 登録 |
| ECS Relationship 型定義 | Relationship を生成・削除するシステム |

## visual_mirror/ の追加ルール

`visual_mirror/` にミラー型を追加する場合:
- 型定義のみ（`#[derive(Component)]` + フィールド）
- Observer や sync system は `hw_jobs/visual_sync/` または `hw_visual/` に置く
- `mod.rs` の `pub mod` + `pub use` を必ず更新する

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/architecture.md](../../docs/architecture.md)（システムセット・GameTime・定数管理の変更時）
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md)（Cargo.toml 変更時）
- `crates/hw_core/_rules.md`（このファイル）

## 検証方法

```bash
# コンパイル確認（必須）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/architecture.md](../../docs/architecture.md): システム実行順序・GameTime・空間グリッド
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md): crate 責務一覧
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md): leaf/root 境界ルール
