# jobs — 建設・建物管理のroot adapter

## 役割

このディレクトリは建設ownerのcancel/completion、`GameAssets`を使う完成spawn、Soul Spa建設、
provisional wall spawnなど、App shell固有のadapterを保持する。共有modelやindex-backed transitionを
rootへ戻さず、production登録とcross-crate orderingは`plugins/logic.rs`が一意に所有する。

## 現行構成

| ファイル/ディレクトリ | 内容 |
|---|---|
| `mod.rs` | 型の選択的re-export、`TaskOwnerCancellationSet`、root API |
| `blueprint_cancellation.rs` | Blueprint owner cancellation |
| `building_completion/` | 完成判定、root asset付きspawn、建物別post-process |
| `floor_construction/cancellation.rs` | Floor siteのowner cancellation |
| `floor_construction/completion.rs` | Floor完成・curing・WorldMap cleanup |
| `wall_construction/cancellation.rs` | Wall siteのowner cancellation |
| `wall_construction/phase_transition.rs` | `Building3dHandles`を使うprovisional wall spawnだけを所有 |
| `wall_construction/completion.rs` | Wall完成・cleanup |
| `soul_spa_construction/` | Soul SpaのBone request、delivery、tile activation |

## 建設phaseの責務境界

| 層 | owner | 責務 |
|---|---|---|
| model / pure rule | `hw_jobs::construction` | site/tile state、eligibility、局所transition method |
| indexed ECS adapter / metrics | `hw_logistics::construction_phase_transition` | `TileSiteIndex`で当該siteのtileだけを検証し、floor/wall phaseを原子的に進める |
| root adapter / registration | `bevy_app` | cancel、completion、asset依存spawn、`TaskOwnerCancellationSet::Flush`後の一意登録 |

## Doorの責務境界

- `hw_world::door_systems`: `DoorState`適用と1候補に対するpure auto-open/keep-open rule。
- `hw_spatial::door_proximity`: Soul spatial indexから候補を抽出するopen/close systemとprofiling metrics。
- `bevy_app::plugins::logic`: startupから`DoorVisualHandles`を注入し、2 systemをproductionへ一度だけ登録。

`building_completion_system`は`BuildingCompletionSet`でSoul AI Execute後に走る。
WorldMap footprint登録・movement blocker・Soul押し出しは`BuildingCompletedEvent`を受ける
`hw_soul_ai::building_completed::on_building_completed`が所有する。
