# hw_ai Boundary Cleanup Plan

`hw_ai` と root `src/systems/` の境界が docs と実装でズレている箇所を整理するための計画。
対象は主に `soul_ai::helpers::work::unassign_task` と `task_execution` 周辺の責務分離。

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `hw-ai-boundary-cleanup-plan-2026-03-12` |
| ステータス | `Draft` |
| 作成日 | `2026-03-12` |
| 最終更新日 | `2026-03-12` |
| 作成者 | `AI (Codex)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

> コードサーベイ基準日: `2026-03-12`

## 1. Problem Description

- 解決したい課題:
  - `hw_ai` は docs 上「純粋 AI core」とされているが、実装では `Commands` / `WorldMap` / `Visibility` / `Transform` / `hw_visual` に直接依存している。
  - 特に `crates/hw_ai/src/soul_ai/helpers/work.rs` の `unassign_task` が root-only 副作用を持ち、docs の「root shell として残す」説明と食い違っている。
  - `task_execution` も docs では「root wrapper + thin shell」と説明される一方、`hw_ai` 側の trait / helper が visual handle や app-side side effect を受けており、どこまでを crate core とみなすかが曖昧。
- 到達したい状態:
  - `hw_ai` の責務を「純粋 core に戻す」か「現実の依存を正式仕様として認める」かを明文化し、コードと docs を一致させる。
  - root `src/` と `hw_ai` の間で「何が adapter で、何が core か」を再定義し、今後の移設判断基準をぶれなくする。
- 成功指標:
  - `unassign_task` の所有先が docs とコードで一致する
  - `hw_ai` の依存関係説明が `Cargo.toml` と一致する
  - `task_execution` の root wrapper / crate core の境界が `docs/cargo_workspace.md`, `docs/soul_ai.md`, `src/README.md`, `src/systems/soul_ai/README.md`, `crates/hw_ai/README.md` で矛盾しない
  - `cargo check --workspace` が成功する

## 2. Current State And Gap

### 2.1 観測された不整合

- `hw_ai` は docs では `hw_core + hw_jobs + hw_logistics + hw_world + hw_spatial` 依存とされているが、実際には `hw_visual` に依存している。
- `crates/hw_ai/src/soul_ai/helpers/work.rs` の `unassign_task` は以下を直接行う:
  - `Commands` による entity 変更
  - `WorldMap` を使った drop 位置再計算
  - `Visibility` / `Transform` 更新
  - `hw_visual::haul::WheelbarrowMovement` の remove
  - **`commands.entity(soul_entity).remove::<WorkingOn>()`（⚠️ tasks.md §5 と矛盾）**
    - `tasks.md §5` の契約では「`WorkingOn` 削除は `task_execution_system` の責務、`unassign_task` はしない」とある
    - 実装はこの契約に違反しており、どちらが正とするかを M1 で明確化する必要がある
- root `src/systems/soul_ai/helpers/work.rs` は docs の説明とは異なり、実装本体を持たず re-export のみ。
- `task_execution` の handler trait は `hw_visual::SoulTaskHandles` を受けるため、docs 上の「pure AI core」説明と一致しない。

### 2.2 問題の本質

- 現状は「クレート化そのものが壊れている」のではなく、途中で責務線引きが変わったのに docs と判断基準が追従していない状態。
- このままだと、今後の移設で以下が再発する:
  - root-only 副作用を `hw_ai` に追加しても違和感に気づけない
  - 逆に本当に pure に戻したいのか、現行方針で良いのか判断できない
  - docs を参照して作業したときに誤った配置判断をする

## 3. Solution Approach

本件は先に「境界方針」を固定してからコードを寄せるべきで、いきなり移設だけ進めると再度ずれる。
そのため、以下の 2 案を比較し、**A を推奨**する。

### 案A: `hw_ai` を pure core に戻す（推奨）

- `unassign_task` の実装本体を root `src/systems/soul_ai/helpers/work.rs` へ戻す
- `hw_ai` 側には `is_soul_available_for_work` や pure helper のみ残す
- `task_execution` で visual handle や app-side cleanup を伴う部分は root wrapper / adapter に寄せる
- `hw_ai` から `hw_visual` 依存を外す、または少なくとも task execution core と visual effect を分離する

利点:

- docs の既存方針と整合する
- crate 境界の説明が単純になる
- 将来の `hw_ai` テスト性・再利用性が上がる

コスト:

- `task_execution` 周辺で引数設計や helper 分割が必要
- visual feedback と cleanup を root 側へ戻す調整が発生する

### 案B: 現行実装を正式仕様として追認する

- `hw_ai` は「AI core + shared execute implementation」まで担当すると定義を変更する
- `unassign_task` の root 残留説明を削除し、`hw_ai` が side-effectful execute 実装も持つ前提に docs を更新する
- `hw_ai` の依存関係に `hw_visual` を正式に追加し、pure という表現をやめる

利点:

- コード移動量が小さい
- 短期的には最速で整合する

欠点:

- crate の意味が広がりすぎる
- 今後も root-only 副作用が `hw_ai` に流入しやすい
- `hw_ai` が事実上「AI 名義の app 実装箱」になりやすい

### 推奨判断

- 現行 docs 群が一貫して「root-only 副作用は root に残す」と説明しているため、まずは **案A** でコードを戻す方が筋が良い。
- ただし `task_execution` の実体が大きく、短期で全部戻すのが重い場合は、M1 で最小不整合の `unassign_task` を先に戻し、M2 以降で handler / visual 依存を段階分離する。

## 4. Expected Performance Impact

- 直接的なランタイム性能改善はほぼ見込まない。
- 主効果は保守性改善であり、期待値は以下:
  - crate 境界判断の誤り減少
  - docs と実装の往復コスト削減
  - `hw_ai` 単体チェック・レビュー時の責務把握が容易になる
- 間接的には `task_execution` の引数整理により compile error の局所化が進み、変更時のデバッグコストを下げられる可能性がある。

## 5. Implementation Steps

### M1: 境界方針の固定と `unassign_task` の所有者確定

- 変更内容:
  - `unassign_task` をどちらが所有するかを案Aで確定する
  - `src/systems/soul_ai/helpers/work.rs` に実装本体を戻し、`hw_ai` 側は pure helper のみ残す
  - `src/README.md`, `src/systems/soul_ai/README.md`, `docs/soul_ai.md`, `docs/cargo_workspace.md`, `crates/hw_ai/README.md` の境界説明を現実に合わせて修正する
- 完了条件:
  - `src/systems/soul_ai/helpers/work.rs` が re-export だけでなく root 実体を持つ
  - `crates/hw_ai/src/soul_ai/helpers/work.rs` から `unassign_task` が消える、または pure helper に分解される
  - `crates/hw_ai/Cargo.toml` の `hw_visual` 依存の要否が説明可能になる

### M2: `task_execution` の visual/app 副作用を仕分ける

- 変更内容:
  - `hw_ai::soul_ai::execute::task_execution` のうち pure に残せる部分と root adapter に戻す部分を棚卸しする
  - 特に以下を分離候補として整理する:
    - `hw_visual::SoulTaskHandles` を受ける handler 層
    - `Commands` を直接使う visual / drop / cleanup helper
    - `WorldMap` と entity side effect を伴う cancel / unassign 系
  - 必要に応じて「pure decision / phase transition」と「app-side effect executor」を分割する
- 完了条件:
  - `task_execution` 周辺で root-only 依存のあるレイヤが module 単位で見える
  - `handler/task_handler.rs` の責務が docs 上で説明できる形になる

### M3: `hw_ai` 依存グラフの再整理

- 変更内容:
  - 案Aで進む場合は `crates/hw_ai/Cargo.toml` から `hw_visual` を外せるか検証する
  - 外せない場合は、何が最後の blocker かを docs に明記する
  - `docs/cargo_workspace.md` の依存図と各 crate README を実コードに合わせる
- 完了条件:
  - `docs/cargo_workspace.md` の依存関係記述が `Cargo.toml` と一致する
  - `hw_ai` README の「pure」表現が実コードと矛盾しない

### M4: root shell / thin adapter の表現統一

- 変更内容:
  - `src/systems/soul_ai/README.md` と `docs/soul_ai.md` の「thin shell」「root wrapper」「root-only 契約」を現コード構造に合わせて書き直す
  - `task_execution_system` だけが wrapper なのか、`unassign_task`・transport helper・visual handler も root 側責務なのかを明文化する
- 完了条件:
  - README と docs の両方で同じ境界説明になっている
  - 新規作業者が docs だけ見て配置判断を誤らない

## 6. Files To Modify

### 実装候補

- `crates/hw_ai/src/soul_ai/helpers/work.rs`
- `src/systems/soul_ai/helpers/work.rs`
- `crates/hw_ai/src/soul_ai/execute/task_execution/handler/task_handler.rs`
- `crates/hw_ai/src/soul_ai/execute/task_execution/handler/impls.rs`
- `crates/hw_ai/src/soul_ai/execute/task_execution/context/*`
- `src/systems/soul_ai/execute/task_execution/mod.rs`
- `src/systems/soul_ai/mod.rs`
- `crates/hw_ai/Cargo.toml`

### ドキュメント候補

- `docs/cargo_workspace.md`
- `docs/soul_ai.md`
- `src/README.md`
- `src/systems/soul_ai/README.md`
- `crates/hw_ai/README.md`
- 必要なら `docs/architecture.md`

## 7. Verification Methods

- コンパイル確認:
  - `cargo check -p hw_ai`
  - `cargo check --workspace`
- 差分確認:
  - `rg -n "unassign_task" src crates docs`
  - `rg -n "hw_visual" crates/hw_ai`
  - `rg -n "pure|root shell|root-only|thin shell" docs src/README.md crates/hw_ai/README.md src/systems/soul_ai/README.md`
- 設計確認:
  - `docs/cargo_workspace.md` の依存図と `Cargo.toml` の依存が一致していること
  - `src/systems/soul_ai/helpers/work.rs` の責務説明と実装が一致していること

## 8. Risks And Open Questions

- `task_execution` は既に `hw_ai` 側へかなり移っているため、`unassign_task` だけを root に戻すと再び相互依存が増える可能性がある。
- `hw_visual::SoulTaskHandles` を完全に外すには、task execution の visual feedback を別層へ分ける必要があるかもしれない。
- 案Aを採るなら「どこまで pure を厳守するか」を明文化しないと、別の helper が同じように `hw_ai` へ流入する。
- 逆に案Bへ切り替えるなら、`hw_ai` の名称と責務説明をかなり書き換える必要がある。

## 9. Recommended Execution Order

1. M1 で `unassign_task` の所有者を確定し、最小不整合を解消する
2. M2 で `task_execution` の visual/app 依存を棚卸しし、分離方針を固める
3. M3 で `hw_ai` の依存グラフを実コードどおりに整理する
4. M4 で docs / README を同期し、境界説明を固定する

## 10. Definition Of Done

- `hw_ai` と root の責務境界が、コード・Cargo 依存・docs の 3 点で一致している
- `unassign_task` の所有先が明確で、README 群に矛盾がない
- `task_execution` の core / adapter 分離が説明可能で、新規変更時の配置判断基準が明文化されている
- `cargo check --workspace` が成功している
