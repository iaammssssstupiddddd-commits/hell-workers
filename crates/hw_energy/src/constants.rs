//! Soul Energy・発電・消費の定数

/// Soul 1 体が 1 秒間に生成する発電量（基準値）
pub const OUTPUT_PER_SOUL: f32 = 1.0;

/// 発電中の Soul が 1 秒間に消費する Dream 量
pub const DREAM_CONSUME_RATE_GENERATING: f32 = 0.5;

/// この値を下回ったら GeneratePower タスクを自動終了
/// （参考: hw_core/constants/logistics.rs の REFINE 系終了閾値パターン）
pub const DREAM_GENERATE_FLOOR: f32 = 10.0;

/// この値を上回っていないと GeneratePower タスクをアサインしない（FLOOR より高く設定してループ防止）
pub const DREAM_GENERATE_ASSIGN_THRESHOLD: f32 = 30.0;

/// 屋外ランプ 1 基の電力需要。1 Soul = 5 基まで点灯
pub const OUTDOOR_LAMP_DEMAND: f32 = OUTPUT_PER_SOUL * 0.2;

/// 屋外ランプの照明効果半径（タイル単位）
pub const OUTDOOR_LAMP_EFFECT_RADIUS: f32 = 5.0;

/// Soul Spa のタイル 1 枚あたり建設コスト（Bone）。2×2 = 合計 12
pub const SOUL_SPA_BONE_COST_PER_TILE: u32 = 3;

/// 発電中の疲労蓄積レート（/秒）
/// 参考: hw_core/constants/ai.rs の FATIGUE_WORK_RATE = 0.01。瞑想的な行為のため半分程度
pub const FATIGUE_RATE_GENERATING: f32 = 0.005;

/// 点灯中のランプがソウルに与えるストレス軽減速度（/秒）
/// STRESS_WORK_RATE = 0.005 の 80% 相当
pub const LAMP_STRESS_REDUCTION_RATE: f32 = 0.004;

/// 点灯中のランプがソウルに与える疲労回復ボーナス（/秒）
/// FATIGUE_WORK_RATE = 0.01 の 30% 相当
pub const LAMP_FATIGUE_RECOVERY_BONUS: f32 = 0.003;
