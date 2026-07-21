# familiar_ai — Familiar AIのroot wiring

## 役割

Familiarの状態判断、recruit/scouting、task探索・割当、squad applyの実装本体は
`hw_familiar_ai::familiar_ai`が所有する。このrootディレクトリはゲーム全体のrevisionへ接続する
diagnostics、reservation snapshot同期、plugin wiringだけを保持する。

## 現行構成

| ファイル | 内容 |
|---|---|
| `mod.rs` | `FamiliarAiPlugin`。Leaf plugin追加、AI set配線、root diagnostics/reservation sync登録 |
| `diagnostics.rs` | cross-domain input revisionをtask diagnosticsへ同期 |
| `perceive/resource_sync.rs` | `SharedResourceCache` snapshot、signature cache、0.2秒の安全監査 |
| `_rules.md` | この境界に対する局所ルール |

旧rootの`decide/`、`update/`、`execute/`、`helpers/` shellは存在しない。正規pathは例えば次の通り。

- task management: `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/`
- recruit/scouting/state decision: `crates/hw_familiar_ai/src/familiar_ai/decide/`
- squad/max-soul apply: `crates/hw_familiar_ai/src/familiar_ai/execute/`

## Scheduling

`FamiliarAiCorePlugin`がcore systemを登録する。Decide内部は
`StateDecision → StateFlush → BlueprintAutoGather → AutoGatherFlush → TaskRevisionSync → Delegation → Encouragement`
のstable set列を使う。rootは`TaskRevisionSync`へexternal revision systemを接続し、
`TransportRequestSet::Execute`後にDelegationが走る制約を追加する。

`sync_reservations_system`はPerceive先頭でframe deltaを開始し、task/reservation signatureが変わった時か
定期監査時だけsnapshotを置換する。world replacementではcacheとtimerをresetし、次frameに完全再構築する。
