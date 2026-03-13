# bevy_app/familiar_ai ルート薄型化計画（adapter 維持版）

## メタ情報

| 項目 | 値 |
|---|---|
| 計画ID | `familiar-ai-root-slim` |
| ステータス | `Implementation Ready` |
| 作成日 | `2026-03-13` |
| 更新日 | `2026-03-13` |
| 作成者 | `AI` |

---

## 1. 問題

`bevy_app/src/systems/familiar_ai/` には、すでに `hw_familiar_ai` へ移せる pure core と、
root 側に残すべき adapter / facade / orchestration が混在している。

現状の課題は次の 2 点:

1. root 側に「thin shell 以上の意味を持たない pure helper / pure planning」がまだ残っている可能性がある
2. 一方で、`WorldMapRead` / concrete `SpatialGrid` / `PathfindingContext` / `Commands` /
   root message writer / relationship 最終確定まで crate 側へ寄せると、既存の
   root shell 方針と矛盾する

この計画では、**root を「plugin wiring + root adapter/facade + visual execute」まで縮退**させる。
ただし、**ゲーム整合性の最終責務を持つ層は root に残す**。

---

## 2. 方針

### 2-1. root に残すもの

以下は Bevy API 依存だからではなく、**root がゲームの最終責務を確定する層だから**残す。

| 区分 | 具体例 | root 残留理由 |
|---|---|---|
| plugin wiring / schedule ownership | `familiar_ai/mod.rs` | crate plugin 登録、system ordering、resource 初期化は app shell の責務 |
| root adapter | `decide/state_decision.rs` | full-fat query から narrow view を構築し、root `MessageWriter` へ request/event を書き込む |
| root wrapper / orchestration | `decide/task_delegation.rs` | concrete `DesignationSpatialGrid` / `TransportRequestSpatialGrid` / `ResourceSpatialGrid` / `WorldMapRead` / `PathfindingContext` / `ConstructionSiteAccess` を束ねる |
| root orchestration | `decide/auto_gather_for_blueprint.rs` | `Commands` / entity query / pathfinding / designation 付与と cleanup を担う |
| root helper / adapter | `decide/auto_gather_for_blueprint/actions.rs`, `helpers.rs` | `is_reachable`、designation marker 付与・回収など world 反映責務を持つ |
| root full-fat query bridge | `helpers/query_types.rs` | root ECS query と crate 側 narrow query を橋渡しする |
| root facade | `soul_ai/helpers/work.rs::unassign_task` | `OnTaskAbandoned` と `WorkingOn` の責務を root で最終確定する |
| visual / relationship apply | `execute/*_apply.rs` | visual 依存または root 側 Relationship 更新を持つ |

### 2-2. `hw_familiar_ai` に置くもの

以下は shared crate 型と Bevy 汎用 API だけで閉じる pure core として `hw_familiar_ai` に寄せる。

| 区分 | 具体例 |
|---|---|
| pure state machine / branch logic | `decide/state_decision` の pure dispatch, result 型 |
| pure planning / scoring | `decide/task_management/*`, `auto_gather_for_blueprint/{planning,demand,supply}` |
| pure helper | `decide/helpers.rs`, `decide/recruitment.rs`, `decide/scouting.rs`, `decide/supervising.rs` |
| thin execute core | `execute/state_apply.rs`, `execute/state_log.rs` |
| change detection | `perceive/state_detection.rs` |

### 2-3. 今回のゴール

`bevy_app/src/systems/familiar_ai/` を次の状態まで整理する:

- `hw_familiar_ai` に置ける pure core を移す
- root には adapter / facade / orchestration / visual execute のみ残す
- `unassign_task` は root facade のまま維持する
- `auto_gather_for_blueprint` は pure planning 層と orchestration 層を分離したまま保つ

---

## 3. 現状整理

### 3-1. すでに `hw_familiar_ai` 側に実装済みで、bevy_app は thin re-export のもの

コードを実際に確認した結果、以下のファイルは **bevy_app 側が1〜数行の re-export / root adapter のみ**となっており、
pure core は `hw_familiar_ai` への移設が完了している。

| bevy_app ファイル | bevy_app 側の内容 | hw_familiar_ai 側 |
|---|---|---|
| `perceive/state_detection.rs` | `pub use hw_familiar_ai::...` のみ（純 re-export） | `perceive/state_detection.rs` に実装済み |
| `decide/recruitment.rs` | `hw_familiar_ai` からの re-export ＋ `squad` / `scouting` / `supervising` のまとめ re-export | `decide/recruitment.rs` ほか |
| `decide/encouragement.rs` | `hw_familiar_ai` の core 関数を re-export + root system 関数のみ記述 | `decide/encouragement.rs` に実装済み |
| `decide/squad.rs` | `hw_familiar_ai::...::SquadManager` の re-export のみ | `decide/squad.rs` に実装済み |
| `decide/scouting.rs` | `hw_familiar_ai` への re-export のみ | `decide/scouting.rs` に実装済み |
| `decide/supervising.rs` | `hw_familiar_ai` への re-export のみ | `decide/supervising.rs` に実装済み |
| `decide/state_decision.rs` (pure core 部) | `hw_familiar_ai::...::determine_decision_path` 等を呼び出すのみ | `decide/state_decision.rs` に実装済み |
| `decide/state_handlers/*` | `hw_familiar_ai` の各ハンドラへの委譲のみ | `decide/state_handlers/*` に実装済み |
| `decide/task_management/*` | `hw_familiar_ai` の `TaskManager::delegate_task` を呼び出す | `decide/task_management/*` に実装済み |
| `decide/auto_gather_for_blueprint/demand.rs` | `pub(super) use hw_familiar_ai::...::collect_raw_demand_by_owner;` 1行 | `auto_gather_for_blueprint/demand.rs` に実装済み |
| `decide/auto_gather_for_blueprint/planning.rs` | `pub(super) use hw_familiar_ai::...::build_auto_gather_targets;` 1行 | `auto_gather_for_blueprint/planning.rs` に実装済み |
| `decide/auto_gather_for_blueprint/supply.rs` | `pub(super) use hw_familiar_ai::...::collect_supply_state;` 1行 | `auto_gather_for_blueprint/supply.rs` に実装済み |
| `execute/state_apply.rs` 等 (pure core) | `hw_familiar_ai` 側の thin execute core を呼び出す | `execute/state_apply.rs`, `state_log.rs` に実装済み |

### 3-2. root 側に残す前提で扱うもの（意図的な残留）

以下は現行アーキテクチャ文書どおり、削除対象ではなく**意図的な残留**として扱う。

| ファイル | 扱い | root に残す根拠 |
|---|---|---|
| `decide/state_decision.rs` | root adapter として維持 | `FamiliarDecideOutput` (`MessageWriter` 群) への request/event 書き込みは root の責務 |
| `decide/task_delegation.rs` | root wrapper / orchestration として維持 | `WorldMapRead` / concrete SpatialGrid / `PathfindingContext` / `ConstructionSiteAccess` / perf metrics を束ねる |
| `decide/familiar_processor.rs` | root adapter として維持 | `FamiliarDelegationContext` が `WorldMap` / `PathfindingContext` / `transmute_lens_filtered` を直接保持する |
| `decide/auto_gather_for_blueprint.rs` | root orchestration として維持 | `Commands` / entity query / pathfinding / designation 付与と cleanup を担う |
| `decide/auto_gather_for_blueprint/actions.rs` | root helper / adapter として維持 | designation marker 付与・回収など world 反映責務を持つ |
| `decide/auto_gather_for_blueprint/helpers.rs` | root helper として維持 | `is_reachable` が `WorldMap` / `PathfindingContext` を直接使用（pure helper は hw_familiar_ai 側に抽出済み） |
| `helpers/query_types.rs` | root full-fat query bridge として維持 | narrow query は hw_familiar_ai 側に既存、root 側は full-fat query 3型のみ |
| `perceive/resource_sync.rs` | root system として維持 | `SharedResourceCache` 再構築・`AssignedTask`/`Designation`/`TransportRequest`/relationship の実ワールド再構築 |
| `soul_ai/helpers/work.rs::unassign_task` | root facade のまま維持 | `OnTaskAbandoned` と `WorkingOn` の責務を root で最終確定する |

### 3-3. 精査完了：実装不要と確定したもの

下表は「精査が必要」としていたが、コード確認の結果**追加移設不要**と確定した。

| ファイル | 精査結果 | 根拠 |
|---|---|---|
| `decide/familiar_processor.rs` | **実装不要・現状維持** | `FamiliarDelegationContext` は `WorldMap` / `PathfindingContext` / `transmute_lens_filtered` を保持しており root adapter として正当。先頭の `pub use` はすでに hw_familiar_ai へ委譲済み（`FamiliarSquadContext` / `process_squad_management` / `FamiliarRecruitmentContext` / `process_recruitment`）。さらなる関数抽出の余地なし |
| `decide/auto_gather_for_blueprint/helpers.rs` | **実装不要・現状維持** | pure helper（`OwnerInfo`, `SourceCandidate`, `resource_rank` 等）は `hw_familiar_ai` 側 `helpers.rs` へ移設済みで re-export 済み。残る `is_reachable` は `WorldMap` + `PathfindingContext` 依存のため root に留まる必然性がある |
| `helpers/query_types.rs` | **実装不要・現状維持** | narrow query 5型（`SoulSquadQuery`, `SoulSupervisingQuery`, `SoulScoutingQuery`, `SoulRecruitmentQuery`, `SoulEncouragementQuery`）は hw_familiar_ai 側に定義済み・re-export 済み。root 側に残る `FamiliarSoulQuery` / `FamiliarStateQuery` / `FamiliarTaskQuery` は root 固有の型（`DamnedSoul`, `AssignedTask`, `Inventory`, `ManagedTasks` 等）を束ねる full-fat query であり分離不可 |

### 3-4. 逆輸入を避けるための API 仕分け

ここで避けたいのは、`bevy_app` 固有の wrapper / final side effect 契約を
`hw_familiar_ai` 側 API に持ち込んでから、再び root で使い戻す構成である。

#### A. `familiar_processor.rs`

| 項目 | 判定 | 理由 |
|---|---|---|
| `FamiliarDelegationContext` | root 残留 | `FamiliarSoulQuery`, `FamiliarTaskAssignmentQueries`, `ConstructionSiteAccess`, concrete grid, `WorldMap`, `PathfindingContext` を同時に束ねる root wrapper 文脈だから |
| `process_task_delegation_and_movement(...)` | root 残留 | `transmute_lens_filtered` による query bridge、`Destination` / `Path` 更新、root `state_handlers` 呼び出しまで担う adapter だから |
| `FamiliarSquadContext`, `SquadManagementOutcome`, `finalize_state_transitions`, `process_squad_management` | crate 側維持 | すでに `hw_familiar_ai::decide::helpers` にあり、root 固有 resource を要求しない pure helper / outcome だから |
| `FamiliarRecruitmentContext`, `RecruitmentOutcome`, `process_recruitment` | 現状維持 | 既存境界のまま。追加で crate 側へ押し込むと root query / path 更新を含む adapter ごと移すことになりやすい |

#### B. `task_delegation.rs`

| 項目 | 判定 | 理由 |
|---|---|---|
| `ReachabilityCacheKey` | root 残留 | root wrapper 内の到達判定キャッシュキーであり、crate 公開 API にする意味が薄い |
| `ReachabilityFrameCache` | root 残留 | `WorldMap` change detection と一体で運用する app resource だから |
| `FamiliarAiTaskDelegationParams` | root 残留 | `Time`, concrete grid, `TileSiteIndex`, `WorldMapRead`, `PathfindingContext`, perf metrics を束ねる root `SystemParam` だから |
| `familiar_task_delegation_system(...)` | root 残留 | timer / perf / cache / snapshot 構築の後で crate core を呼ぶ wrapper system だから |

#### C. `helpers/query_types.rs`

| 項目 | 判定 | 理由 |
|---|---|---|
| `SoulEncouragementQuery`, `SoulRecruitmentQuery`, `SoulScoutingQuery`, `SoulSquadQuery`, `SoulSupervisingQuery` | crate 側維持 | narrow query として `hw_familiar_ai` に定義済みで、root は re-export だけでよい |
| `FamiliarSoulQuery` | root 残留 | `AssignedTask`, `Destination`, `Path`, `Inventory`, relationship をまとめた full-fat query で、root adapter 専用だから |
| `FamiliarStateQuery` | root 残留 | root `state_decision.rs` が `FamiliarDecideOutput` へ反映するための full-fat query だから |
| `FamiliarTaskQuery` | root 残留 | root `task_delegation.rs` が wrapper として扱う familiar query だから |

#### D. `auto_gather_for_blueprint/helpers.rs`

| 項目 | 判定 | 理由 |
|---|---|---|
| `OwnerInfo`, `STAGE_COUNT`, `SourceCandidate`, `SupplyBucket`, `compare_auto_idle_for_cleanup`, `resource_rank`, `work_type_for_resource` | crate 側維持 | すでに pure helper として `hw_familiar_ai` に実装済み |
| `is_reachable(...)` | root 残留 | `WorldMap` と `PathfindingContext` を受ける pathfinding adapter であり、crate 側へ動かすと root wrapper を逆輸入する形になる |
| `pub(super) use ...helpers::*` による再公開 | root 維持可 | call site の互換パス維持に有効な thin bridge で、crate 側を root 依存にしない |

#### E. 判定ルール（今後の追加抽出用）

今後 `familiar_ai` から何かを切り出すときは、次のどちらかで判断する。

- `hw_familiar_ai` に置いてよい:
  - shared crate 型と Bevy 汎用 API だけで閉じる
  - root `WorldMapRead` / concrete `SpatialGrid` / `PathfindingContext` / root `MessageWriter` を要求しない
  - relationship / event / designation cleanup の最終確定を行わない
- root に残す:
  - root full-fat query / lens bridge を持つ
  - concrete world wrapper / pathfinding / app shell resource を束ねる
  - request / relationship / event / designation cleanup の最終適用を行う

---

## 4. 残存する実装タスク

主要な pure core 移設は**完了済み**。残作業は以下の**ドキュメント整備と契約の明文化**に絞られる。

### T1: アーキテクチャ文書の更新

**対象ファイル:** `docs/familiar_ai.md`, `docs/cargo_workspace.md`

**内容:**

- `familiar_ai/` root 側ファイルの責務区分（adapter / orchestration / facade）を明記
- `hw_familiar_ai` に移設済みの pure core 一覧を boundary として記載
- `task_delegation.rs` = root wrapper、`state_decision.rs` = root adapter である旨を明記
- `helpers/query_types.rs` は narrow query の re-export ＋ root full-fat query 集約の bridge であることを明記

**完了条件:**

- `cargo check --workspace` が green を維持したまま docs が更新される
- docs の boundary 記述が現在のコード構造と矛盾しない

### T2: root 残留ファイルへのコメント追記

**対象ファイル（6点）:**

| ファイル | 追記内容 |
|---|---|
| `decide/familiar_processor.rs` | `//! root adapter: FamiliarDelegationContext は WorldMap / PathfindingContext を直接保持するため root に残留` を先頭コメントへ追加 |
| `decide/task_delegation.rs` | `//! root wrapper / orchestration` を先頭コメントへ追加または既存コメントを更新 |
| `decide/auto_gather_for_blueprint.rs` | `//! root orchestration: Commands / pathfinding / designation 付与を担う` を先頭コメントへ確認・追加 |
| `decide/auto_gather_for_blueprint/helpers.rs` | `//! root helper: is_reachable は WorldMap + PathfindingContext 依存のため root に残留` を追記 |
| `helpers/query_types.rs` | `//! root full-fat query bridge: narrow query は hw_familiar_ai 側に定義済み` を先頭に追記 |
| `perceive/resource_sync.rs` | `//! root perceive system: SharedResourceCache 再構築は root の責務` を確認・追記 |

**完了条件:**

- 各ファイル先頭コメントが 2-1 節の「root 残留理由」と一致する
- コメントを追記後、`cargo check --workspace` が green を維持する

### T3: bevy_app thin re-export ファイルのコメント統一

thin re-export ファイルのうち、コメントが不統一または欠如しているものを確認・統一する。

**対象（確認のみ、必要であれば追記）:**

- `decide/demand.rs`, `planning.rs`, `supply.rs`（各1行）: `//! pure core は hw_familiar_ai 側に実装済み。本ファイルは re-export のみ。`
- `decide/recruitment.rs`: 既存コメントあり（「ロジックは hw_familiar_ai へ移設済み」）、維持

**完了条件:**

- re-export ファイルがコメントにより「意図的な thin shell」と識別できる

---

## 5. 変更対象ファイル

| 操作 | ファイル | 備考 |
|---|---|---|
| 更新 | `docs/familiar_ai.md` | T1 対応 |
| 更新 | `docs/cargo_workspace.md` | T1 対応 |
| 更新（コメント追記のみ） | `src/systems/familiar_ai/decide/familiar_processor.rs` | T2 対応 |
| 更新（コメント追記のみ） | `src/systems/familiar_ai/decide/task_delegation.rs` | T2 対応 |
| 更新（コメント確認・追記） | `src/systems/familiar_ai/decide/auto_gather_for_blueprint.rs` | T2 対応 |
| 更新（コメント追記のみ） | `src/systems/familiar_ai/decide/auto_gather_for_blueprint/helpers.rs` | T2 対応 |
| 更新（コメント追記のみ） | `src/systems/familiar_ai/helpers/query_types.rs` | T2 対応 |
| 更新（コメント確認） | `src/systems/familiar_ai/perceive/resource_sync.rs` | T2 対応 |
| 更新（コメント確認・統一） | `src/systems/familiar_ai/decide/auto_gather_for_blueprint/demand.rs` 等 | T3 対応 |

**コード実装変更は発生しない**（T1〜T3 はすべてコメント追記と docs 更新のみ）

---

## 6. リスク

| リスク | 影響 | 対策 |
|---|---|---|
| pure helper と root adapter の境界を曖昧にしたまま再移設する | 高 | 「system 単位移設」ではなく「責務単位移設」に切り替える。3-3 節の精査結果を参照し不要な移設を抑止する |
| `unassign_task` を crate 側へ寄せて root 契約が崩れる | 高 | root facade として固定し、cleanup 本体だけ crate 側に委譲する現状を維持 |
| `auto_gather_for_blueprint` の orchestration まで crate 側へ寄せて pathfinding / designation cleanup の責務が混ざる | 高 | pure planning（hw_familiar_ai）と root orchestration（bevy_app）の二層を維持。thin re-export ファイルのみ crate 側に触れる |
| `helpers/query_types.rs` を削除して root query lens 構築が各所へ分散する | 中 | bridge ファイルとして残し、full-fat query 集約の単一入口にする（3-3 節で移設不要と確定済み） |
| `task_delegation.rs` の wrapper 責務を削りすぎて root resource 依存が crate 側へ漏れる | 中 | `WorldMapRead` / grid / pathfinding / site access を root 固定とし、crate 側は pure algorithm のみとする |

---

## 7. 検証方法

実装（T1〜T3）完了後、次の順で検証する。

1. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` → エラーなし
2. `docs/familiar_ai.md` と `docs/cargo_workspace.md` の boundary 記述と現コード構造が矛盾しないことを目視確認
3. `crates/bevy_app/src/systems/familiar_ai/` に残るファイルが、adapter / facade / orchestration / visual execute / thin re-export のいずれかで説明できることを確認
4. `hw_familiar_ai` に実装済みの関数が root `WorldMapRead` / concrete `SpatialGrid` / `Commands` / root `MessageWriter` を直接要求していないことを確認

---

## 8. 目標状態

最終的な整理後の `bevy_app/src/systems/familiar_ai/` は、次のような性格を持つ:

```
familiar_ai/
├── mod.rs                              # plugin wiring / resource 初期化 / ordering
├── README.md
├── decide/
│   ├── mod.rs
│   ├── state_decision.rs               # root adapter（MessageWriter への request/event 書き込み）
│   ├── task_delegation.rs              # root wrapper / orchestration（WorldMap / grid / pathfinding）
│   ├── familiar_processor.rs           # root adapter（FamiliarDelegationContext、WorldMap 直接保持）
│   ├── auto_gather_for_blueprint.rs    # root orchestration（Commands / designation 付与）
│   ├── auto_gather_for_blueprint/
│   │   ├── actions.rs                  # root helper（designation marker 付与・回収）
│   │   ├── helpers.rs                  # root helper（is_reachable: WorldMap + pathfinding）
│   │   ├── demand.rs                   # thin re-export → hw_familiar_ai
│   │   ├── planning.rs                 # thin re-export → hw_familiar_ai
│   │   └── supply.rs                   # thin re-export → hw_familiar_ai
│   ├── encouragement.rs                # root adapter + thin re-export（root system 関数のみ追加）
│   ├── recruitment.rs                  # thin re-export（ロジックは hw_familiar_ai 側に移設済み）
│   ├── squad.rs                        # thin re-export → hw_familiar_ai
│   ├── scouting.rs                     # thin re-export → hw_familiar_ai
│   ├── supervising.rs                  # thin re-export → hw_familiar_ai
│   ├── state_handlers/mod.rs           # thin re-export → hw_familiar_ai
│   └── task_management/mod.rs          # hw_familiar_ai の TaskManager を呼び出す adapter
├── helpers/
│   ├── mod.rs
│   └── query_types.rs                  # root full-fat query bridge（narrow query は hw_familiar_ai 側）
├── perceive/
│   ├── mod.rs
│   ├── resource_sync.rs                # root perceive system（SharedResourceCache 再構築）
│   └── state_detection.rs              # thin re-export → hw_familiar_ai
├── execute/
│   ├── mod.rs
│   ├── encouragement_apply.rs          # visual / relationship apply
│   ├── idle_visual_apply.rs            # visual apply
│   ├── max_soul_apply.rs               # visual apply
│   └── squad_apply.rs                  # relationship apply
└── update/
    └── mod.rs
```

root は「薄いが意味のある shell」とし、**ゲーム整合性の最終責務を持たない pure core だけを crate 側へ寄せる**。

> **注意:** 上記ファイルツリーは **現在すでに達成済み**の構造を示す。  
> T1〜T3 はコメント・docs の整合性担保のみであり、ファイル移動・削除は発生しない。
