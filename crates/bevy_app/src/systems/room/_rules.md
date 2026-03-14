# bevy_app/systems/room — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このディレクトリがやること）

**ECS 接続層（アダプタ層）のみ**：`hw_world` の部屋検出ロジックを Bevy ECS へ接続する配線

具体的には：
- 部屋検出結果（`RoomBounds`）の ECS への反映
- 部屋 overlay ビジュアルの適用
- 部屋関連 Designation / タスクとの接続

## 禁止事項（AI がやってはいけないこと）

- **このディレクトリに部屋検出の純粋ロジックを書かない**（`hw_world` / `hw_world::room_detection` に書く）
- **Bevy 0.14 以前の API を推測で使わない**

## crate 境界ルール

- `bevy_app` は **App Shell / Adapter**：部屋の検出ロジックは `hw_world` に置く
- 詳細: [docs/crate-boundaries.md](../../../../../docs/crate-boundaries.md)

## ECS 契約

- `RoomBounds` Component は `hw_world` 側が所有・定義
- 部屋検出結果は非同期で次フレーム以降に反映される（Change Detection ベース）
- 詳細: [docs/room_detection.md](../../../../../docs/room_detection.md)

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/room_detection.md](../../../../../docs/room_detection.md)
- `crates/bevy_app/src/systems/room/_rules.md`（このファイル）

## 検証方法

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/room_detection.md](../../../../../docs/room_detection.md): 部屋検出システム仕様
- [docs/crate-boundaries.md](../../../../../docs/crate-boundaries.md): leaf/root 境界ルール
- [crates/hw_world/_rules.md](../../../../hw_world/_rules.md): leaf crate ルール
