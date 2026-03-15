# TRELLIS / TRELLIS.2 のライセンスと商用利用に関するメモ

---

## 公式ライセンスの状況

### MIT ライセンス（商用利用可）

- **コアモデル**: 4B パラメータのネットワーク自体
- **学習済みウェイト**: Hugging Face で配布されている `.pt` / `.safetensors`
- **大半の推論コード**: PyTorch 実装の大部分
- **モデルの直接出力**:
  - Gaussian primitives
  - Local radiance fields within active voxels
  - Local signed distance values

これらまでは MIT ライセンスで、**商用利用が可能**です（著作権・ライセンス表示の保持のみ要件）。

### 非商用ライセンス（商用利用不可）

以下のコンポーネントは、公式リポジトリでは **商用利用不可** のライブラリに依存しています:

| コンポーネント | 用途 | ライブラリ |
|----------------|------|------------|
| Gaussian Rasterization | 3D Gaussians のレンダリング | `diff-gaussian-rasterization` |
| Radiance fields | 放射輝度場のレンダリング | `diffoctreerast` |
| Mesh / GLB extraction | メッシュ抽出 | FlexiCubes（Kaolin 内） |
| Texture baking | テクスチャ焼き込み | `nvdiffrast` |

これらが含まれるパイプラインで GLB を出力する場合、**公式のままでは商用利用できない**可能性があります。

---

## 商用利用可能な方法

### 1. コミュニティ fork を使う（推奨）

有志が商用利用可能なライブラリに置換した fork が公開されています:

- **kg-git-dev/trellis-refactored**
  - `nvdiffrast` → **pytorch3d**（商用可）
  - その他も整理
  - GitHub: https://github.com/kg-git-dev/trellis-refactored

- **jclarkk/TRELLIS**
  - `diff-gaussian-rasterization` → **gsplat**（Apache 2.0）
  - FlexiCubes → **Marching Cubes**
  - GitHub: https://github.com/jclarkk/TRELLIS

これらをセルフホストすれば、**コード上のライセンス問題は解決**します。

### 2. 自前で置換する

必要に応じて、以下の組み合わせで置換:

| 置換対象 | 商用可能な代替 |
|----------|----------------|
| Gaussian Rasterization | [gsplat](https://github.com/nerfstudio-project/gsplat) (Apache 2.0) |
| Mesh extraction | Marching Cubes / PyMarchingCubes |
| Texture baking | [PyTorch3D](https://github.com/facebookresearch/pytorch3d) (BSD) |
| Mesh cleanup | [Trimesh](https://github.com/mikedh/trimesh) (MIT) |

---

## データセットのライセンス問題

学習データに含まれる一部のデータセット（3D-FUTURE, HSSD, Toys4K）は **非商用ライセンス** です。

- Microsoft が学習済みウェイトを MIT で提供しているため、**ウェイト自体は商用利用可能**と解釈できる
- ただし、完全にクリアな形にするには、**商用可能なデータセットのみで再学習**する必要がある
- 現時点では「Microsoft の MIT 表示を信頼して利用する」か「法務に確認」が現実的な対応

---

## 代替モデル（参考）

| モデル | 商用利用 | VRAM | 備考 |
|--------|----------|------|------|
| **TRELLIS** + fork | △ 要置換 | 16〜24GB | fork で商用可 |
| **TripoSR** | ○ | 12GB+ | Stability AI + Tripo AI |
| **PartPacker** | ✕ Non-Commercial | 16GB+ | NVIDIA、商用不可 |
| **Hunyuan3D-2** | ✕ 地域制限 | 16GB+ | EU/UK/Korea 禁止 |

---

## このプロジェクトでの扱い

1. **PoC / 研究目的**: 公式 `camenduru/tostui-trellis2` イメージをそのまま利用（ライセンス確認は後回しで可）
2. **商用リリース前**:
   - `kg-git-dev/trellis-refactored` に切り替え
   - または自前で gsplat + PyTorch3D に置換したパイプラインを構築
   - 法務確認（データセットライセンス）

---

## 参照

- [TRELLIS LICENSE](https://github.com/microsoft/TRELLIS/blob/main/LICENSE)
- [Can this be used commercially? - Issue #41](https://github.com/microsoft/TRELLIS/issues/41)
- [kg-git-dev/trellis-refactored](https://github.com/kg-git-dev/trellis-refactored)
- [jclarkk/TRELLIS](https://github.com/jclarkk/TRELLIS)