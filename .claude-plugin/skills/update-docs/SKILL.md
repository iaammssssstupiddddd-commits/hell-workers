---
name: update-docs
description: Use this skill after implementing code changes (crate migrations, type moves, milestone completions) in the hell-workers project. Trigger when the user says "ドキュメントを更新", "READMEを更新", "docs更新", "実装ドキュメントを更新", or after completing a milestone like M1/M2/M3. Updates all affected docs (implementation specs, README files in docs/, crates/, and src/) to reflect what changed.
---

# update-docs — ドキュメント更新スキル

コード変更（型の crate 移動、マイルストーン完了等）の後に、影響するすべてのドキュメントを一括更新する。

## 更新対象ファイルの全体像

```
docs/
  README.md                    ← 削除済みファイル参照の除去・同期メモ更新
  plans/README.md              ← 計画書のステータス更新（完了/In Progress/削除）
  proposals/README.md          ← 削除済み提案書の除去
  cargo_workspace.md           ← 各 crate の責務表に新型を追記
  logistics.md                 ← ECS 接続マップ・型の所在変更を反映
  tasks.md                     ← AssignedTask 等の ECS 接続マップを更新
  building.md                  ← Blueprint/Site コンポーネントの所在変更を反映
  architecture.md              ← crate 構造・SystemSet 所在の変更を反映
  soul_ai.md / familiar_ai.md  ← hw_ai との境界変更があれば更新
  invariants.md                ← ゲーム不変条件の変更（新しいサイレント失敗・契約変更）
  events.md                    ← イベント追加・削除・Producer/Consumer 変更

crates/
  hw_*/README.md               ← 追加された型・モジュールを主要モジュール表と境界表に追記
  hw_*/_rules.md               ← crate 境界・禁止事項・依存制約の変更を反映

src/
  systems/*/README.md          ← シェル化されたファイルの説明更新・re-export 注記追加
  systems/familiar_ai/README.md ← hw_* から re-export している型の出所を注記
  systems/logistics/README.md  ← hw_logistics 境界表を更新
  systems/world/README.md      ← シェル化されたファイルの説明更新
  systems/*/_rules.md          ← ECS 接続層の禁止事項・システムセット順の変更を反映
```

## 手順

### 1. 変更内容を把握する

git status と変更ファイル一覧を確認して、何が変わったかを特定する。

```bash
git status --short
```

注目点：
- **新規 crate ファイル**（`crates/hw_*/src/*.rs`）: その crate の README に追記が必要
- **シェル化された root ファイル**（`pub use hw_xxx::*` に置き換えられたもの）: src/ の README を「シェル」と明記
- **削除されたドキュメント**（`D docs/plans/xxx.md`）: plans/README.md・proposals/README.md から行を除去
- **移動された型**（`cargo_workspace.md` の責務表に反映が必要）

### 2. 対象ファイルを読む（並列）

変更に関係するすべての README を並列で読み、現在の記述を把握してから編集する。

### 3. 各 README を更新する

#### docs/plans/README.md

- 削除済み計画書（git status で `D` のもの）を「現行計画書」テーブルから除去
- In Progress 計画書のステータス・Notes を最新の進捗に更新
  - 例: `Draft` → `In Progress (~40%)` + `M1/M2/M3 完了。残り M4/M5/M6` のような Notes

#### docs/proposals/README.md

- 削除済み提案書（git status で `D` のもの）を「現在の提案書」テーブルから除去

#### docs/README.md

- 削除済みファイルを参照している「同期済み」注記行を除去する

#### docs/cargo_workspace.md

各 crate の「代表例」セクションに移動してきた型を追記する。
「ここに置かないもの」にも必要に応じて注記する。

例（M1 完了時）:
```
### `hw_jobs`
代表例:
+ `FloorTileBlueprint`, `WallTileBlueprint`（タイル Blueprint）
+ `TargetFloorConstructionSite`, `WallConstructionCancelRequested` 等

ここに置かないもの:
+ `FloorConstructionSite`（`TaskArea` 依存のためまだ root 残留）
```

#### crates/hw_*/README.md

- **主要モジュール表**: 新規 `.rs` ファイルを行追加
- **src/ との境界表**: 移動してきた型を「hw_XXX に置くもの」列に追記。まだ root 残留中の型は「src/ に置くもの」列に理由付きで残す

#### src/systems/*/README.md（シェル化された場合）

ファイルが `pub use hw_xxx::yyy::*;` 1行に置き換えられたら:
```
- `zones.rs` | `Site`・`Yard` の定義     ← 変更前
+ `zones.rs` | `pub use hw_world::zones::*;` — 1行シェル  ← 変更後
```

re-export している型の出所が hw_* になった場合、説明文に `（hw_xxx から re-export）` を追記する。

#### src/systems/familiar_ai/README.md

`perceive/resource_sync.rs` のような「型は crate 側に移ったがシステム関数は root 残留」のファイルは、型の出所を括弧で注記する:
```
`SharedResourceCache`（`hw_logistics` から re-export）
```

#### src/systems/logistics/README.md

hw_logistics との境界表を更新:
- 型が hw_logistics に移ったら「hw_logistics に置かれているもの」列に移動
- システム関数が root 残留中なら「src/ に置かれているもの」列に理由付きで記載

### 4. docs/ 直下の実装ドキュメントを更新する

crate 移動や型の移動が発生した場合、対応する実装ドキュメントも更新する。
変更の規模に応じて影響するファイルを判断し、**必ず読んでから編集**すること。

#### docs/logistics.md

- 型が hw_logistics に移った場合: ECS 接続マップの「書き込み元」モジュールパスを更新
- `SharedResourceCache` のような型が crate 側に移ったら所在を注記
- hw_logistics との境界の変化（新たに移植されたシステム関数等）を反映

#### docs/tasks.md

- `AssignedTask` 等の ECS 接続マップで、書き込み元/削除元のモジュールパスが変わったら更新
- 型の所在（`hw_jobs::assigned_task` 等）が変わったセクションを修正

#### docs/building.md

- `FloorTileBlueprint` / `WallTileBlueprint` 等が hw_jobs に移動したなら所在を更新
- `FloorConstructionSite` / `WallConstructionSite` が root に残留中なら「root 残留」と明記
- Blueprint と ConstructionSite が**別型**であることが不明瞭なら補足を追加

#### docs/architecture.md

- crate の依存グラフや SystemSet 所在が変わった場合に更新
- hw_logistics に新たな依存（hw_world, hw_jobs, hw_spatial）が追加されたなら依存図を修正

#### docs/soul_ai.md / docs/familiar_ai.md

- hw_ai との境界が変わった場合のみ更新（通常の型移動では変更不要）

**判断基準**: 型の「どこに定義されているか」「どこから呼ばれるか」が変わったなら更新が必要。
コンポーネントフィールド変更や挙動変更がなければ省略してよい。

### 5. docs/plans/ 内の In Progress 計画書を更新する

完了したマイルストーンがある場合:
- `[ ]` → `[x]` に変更
- ステータス行を更新: `In Progress (~40%)`, 最終更新日に完了した M を追記
- 「現在地」セクションの進捗・完了済み・未着手リストを更新
- 更新履歴に今回の変更を追記

### 6. 確認

すべての更新が終わったら、変更内容の一覧をテーブルで提示する:

| ファイル | 変更内容 |
|:--|:--|
| `docs/plans/README.md` | ○○を更新 |
| `crates/hw_jobs/README.md` | ○○を追記 |
| ... | ... |

## よくあるパターン

| 変更の種類 | 更新が必要なファイル |
|:--|:--|
| 型を crate に移動 | 移動先 crate の README・cargo_workspace.md・移動元 src README・関連実装ドキュメント（logistics.md 等）・影響する `_rules.md` |
| logistics 系の型を hw_logistics に移動 | docs/logistics.md・crates/hw_logistics/README.md・src/systems/logistics/README.md・`hw_logistics/_rules.md` |
| jobs 系の型を hw_jobs に移動 | docs/building.md・docs/tasks.md・crates/hw_jobs/README.md・src/systems/jobs/README.md・`hw_jobs/_rules.md` |
| world 系の型を hw_world に移動 | crates/hw_world/README.md・src/systems/world/README.md・`hw_world/_rules.md` |
| root ファイルをシェル化 | 対応する src/systems/*/README.md |
| crate 間依存を追加 | docs/architecture.md・docs/cargo_workspace.md・影響する `_rules.md` の依存制約節 |
| 計画書マイルストーン完了 | docs/plans/README.md・docs/plans/[計画ファイル].md |
| 計画書ファイルを削除 | docs/plans/README.md |
| 提案書ファイルを削除 | docs/proposals/README.md |
| docs/README.md の同期メモが古くなった | docs/README.md |
| ゲームルール・不変条件の変更 | docs/invariants.md（該当する I-* 節）|
| イベント追加・削除・Producer/Consumer 変更 | docs/events.md（該当テーブル行）|
| `_rules.md` の新規作成 | docs/plans/README.md（multi-tool-ai-rules-plan の進捗更新）|
