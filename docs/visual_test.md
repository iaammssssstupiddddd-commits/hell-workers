# Visual Test Scene

ゲーム本体とは独立して、TopDownの建物・地形表示とScene RtT合成を確認するための補助クレートです。
Soulの製品表示は本体の`ActorBillboard3d`とP08 native acceptanceを正本とし、このクレートはGLB、表情atlas、shadow proxyを読み込みません。

## 起動

```bash
python3 scripts/dev.py cargo -- run -p visual_test
```

## 検証できること

| 項目 | 概要 |
|:---|:---|
| ワールド上での建築物配置 | マウス追従のゴーストとグリッド単位の配置・削除 |
| 建築物2D/3D表示 | 同じgridへ置いた補助2D spriteとScene RtT内3D形状の位置関係 |
| 地形表示 | deterministic terrain map上での配置とcamera pan/zoom |
| 影・ライト | 建物receiver向けDirectionalLightとcascade shadow |
| RtTパイプライン | Camera3d → 単一offscreen texture → composite sprite |
| resize / DPI | Window物理解像度変更時のtexture再生成・camera target・material再bind |

## 操作

| キー / 操作 | 内容 |
|:---|:---|
| `H` | メニューパネル表示/非表示 |
| `W/A/S/D` | カメラパン |
| スクロール | カメラズーム。メニュー上ではパネルスクロール |
| マウス移動 | ゴースト追従（緑=空き、赤=占有） |
| 左クリック | 現在gridへ配置、または同位置の建物を削除 |
| `[` / `]` | 建築種別を前/次へ切り替え |
| `Enter` | 現在のゴースト位置で配置/削除 |
| `Del` | 全建築物を削除 |
| `Esc` | 終了 |

右側のメニューでは建築種別、現在grid、配置/削除、全削除を操作できます。パネル上のスクロールはworld cameraへ伝播しません。

## アーキテクチャ

`crates/visual_test/`はゲーム本体の`GameAssets`や`StartupPlugin`に依存しない独立binaryです。

| ファイル | 責務 |
|:---|:---|
| `main.rs` | App、plugin、system順序 |
| `types.rs` + `types/` | RtT、local fixture state、UI action |
| `setup.rs` + `setup/` | camera、RtT、directional light、menu |
| `building.rs` | terrain map、建物asset/shape、配置ghost |
| `systems.rs` | menu action、camera同期、resize/DPI再bind、composite |
| `hud.rs` | panel visibility、button state、grid text |
| `input.rs` | keyboard input |

### 3 pass構造

```text
Camera3dRtt      (LAYER_3D,      order=-2) → scene RtT
TestMainCamera   (LAYER_2D,      order= 0) ← PanCamera
Overlay Camera2d (LAYER_OVERLAY, order= 1) ← composite sprite + UI
```

`sync_test_camera3d`は2D cameraのpan/scaleを固定TopDown Camera3dへ反映します。Scene RtTはWindowの物理解像度で生成し、`ImageRenderTarget.scale_factor`へWindow scale factorを設定します。resizeまたはDPI変更時はScene texture、camera target、composite materialを同時に再生成・再bindします。

Soul billboard、Familiar foreground、load lifecycle、exactly-one presentationの受入は`visual_test`ではなく、production fixtureを通るP02/P08 actual-window・RenderDoc evidenceで行います。
