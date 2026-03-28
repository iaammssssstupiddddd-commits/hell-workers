use bevy::prelude::*;

/// Yard の電力網エンティティ。定期的に再計算される。
/// Yard 追加 Observer によって 1 対 1 で自動生成される。
/// 初期状態: generation=0, consumption=0, powered=true（消費者なし＝停電ではない）
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct PowerGrid {
    /// 接続全 PowerGenerator の current_output 合計
    pub generation: f32,
    /// 接続全 PowerConsumer の demand 合計
    pub consumption: f32,
    /// generation >= consumption のとき true
    pub powered: bool,
}

impl Default for PowerGrid {
    fn default() -> Self {
        Self {
            generation: 0.0,
            consumption: 0.0,
            powered: true, // 空グリッドは powered（消費者がいない＝停電ではない）
        }
    }
}

/// SoulSpaSite に付与。サイト単位の発電集計。
/// Phase 1b で SoulSpaSite スポーン時に追加される（ここでは型定義のみ）。
#[derive(Component, Reflect, Debug, Default, Clone)]
#[reflect(Component)]
pub struct PowerGenerator {
    /// 実際の出力: 占有スロット数 × output_per_soul
    pub current_output: f32,
    /// Soul 1 体あたりの発電量。通常は OUTPUT_PER_SOUL 定数と同値。
    /// フィールドとして保持する理由: 将来の上位施設（効率の良い Soul Spa 等）で
    /// 施設ごとに異なる値を設定可能にするため。
    pub output_per_soul: f32,
}

/// 電力消費建物（OutdoorLamp 等）に付与。
/// `#[require(Unpowered)]` により、グリッド接続前はデフォルトで停電状態になる。
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
#[require(Unpowered)]
pub struct PowerConsumer {
    /// 稼働時の消費電力（/秒）
    pub demand: f32,
}

/// マーカー: この Consumer は電力供給を受けていない。
/// `#[require(Unpowered)]` によりデフォルトで付与。
/// グリッド再計算で供給が確認されると除去され、停電時に再挿入される。
#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
#[reflect(Component)]
pub struct Unpowered;

/// PowerGrid エンティティ上に付与。所属する Yard への逆参照。
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct YardPowerGrid(pub Entity);

impl Default for YardPowerGrid {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}
