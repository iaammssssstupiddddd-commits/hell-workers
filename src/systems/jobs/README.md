# jobs — 建設・建物管理システム

## 役割

建物の建設フェーズ遷移、完成処理、ドア管理、泥ミキサーワークフローを実装する。
データ型（フェーズ enum・BuildingType 等）は `hw_jobs` クレートに定義されており、このディレクトリは**Bevy システムとして動作するロジック**を担う。

## 主要ファイル

| ファイル/ディレクトリ | 内容 |
|---|---|
| `mod.rs` | `building_completion_system` 等の公開 API |
| `door.rs` | `door_auto_open_system`, `door_auto_close_system` |
| `mud_mixer.rs` | 泥ミキサーワークフロー管理 |
| `floor_construction/` | 床建設フェーズシステム（下表） |
| `wall_construction/` | 壁建設フェーズシステム（下表） |
| `building_completion/` | 建物完成後処理（下表） |

## floor_construction/ ディレクトリ

```
ReinforceReady → Reinforcing → PouredReady → Poured
```

| ファイル | 内容 |
|---|---|
| `mod.rs` | 公開 API |
| `components.rs` | 床建設サイト固有コンポーネント |
| `phase_transition.rs` | フェーズ遷移システム |
| `completion.rs` | 床完成処理 |
| `cancellation.rs` | 床建設キャンセル |

## wall_construction/ ディレクトリ

```
Ready → Framed → ProvisionalReady → CoatedReady → Coated
```

| ファイル | 内容 |
|---|---|
| `mod.rs` | 公開 API |
| `components.rs` | 壁建設サイト固有コンポーネント |
| `phase_transition.rs` | フェーズ遷移システム |
| `completion.rs` | 壁完成処理 |
| `cancellation.rs` | 壁建設キャンセル |

## building_completion/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | `building_completion_system` |
| `spawn.rs` | 完成後エンティティのスポーン |
| `post_process.rs` | 完成後処理（ワールドマップ更新等） |
| `world_update.rs` | ワールドマップ歩行可能性の更新 |
