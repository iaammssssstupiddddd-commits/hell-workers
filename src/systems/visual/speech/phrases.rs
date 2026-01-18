use rand::Rng;
use rand::seq::SliceRandom;

/// Familiar が喋るラテン語のフレーズ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LatinPhrase {
    /// 来い！ (リクルート)
    Veni,
    /// 働け！ (通常作業)
    Laborare,
    /// 掘れ！ (採掘)
    Fodere,
    /// 伐れ！ (伐採)
    Caede,
    /// 運べ！ (運搬)
    Portare,
    /// 休め！ (休憩/アイドル)
    Requiesce,
    /// 去れ！ (リリース)
    Abi,
}

impl LatinPhrase {
    /// フレーズ候補のリストを返す (各5つ)
    pub fn variants(&self) -> &'static [&'static str] {
        match self {
            LatinPhrase::Veni => &["Veni!", "Ad me!", "Huc!", "Sequere!", "Adesto!"],
            LatinPhrase::Laborare => &["Laborare!", "Opus!", "Facite!", "Agite!", "Elabora!"],
            LatinPhrase::Fodere => &["Fodere!", "Effodite!", "Excava!", "Pelle!", "Fodite!"],
            LatinPhrase::Caede => &["Caede!", "Seca!", "Tunde!", "Percute!", "Incide!"],
            LatinPhrase::Portare => &["Portare!", "Fer!", "Cape!", "Tolle!", "Affer!"],
            LatinPhrase::Requiesce => &["Requiesce!", "Quiesce!", "Siste!", "Mane!", "Pausa!"],
            LatinPhrase::Abi => &["Abi!", "Discede!", "I!", "Vade!", "Recede!"],
        }
    }

    /// ランダムにフレーズを選択
    pub fn random_str(&self) -> &'static str {
        let variants = self.variants();
        variants
            .choose(&mut rand::thread_rng())
            .unwrap_or(&variants[0])
    }

    /// 使い魔の傾向に基づいてフレーズを選択
    pub fn select_with_preference(
        &self,
        preferred_index: usize,
        preference_weight: f32,
    ) -> &'static str {
        let variants = self.variants();
        let mut rng = rand::thread_rng();

        if rng.r#gen::<f32>() < preference_weight {
            // お気に入りを使用
            variants.get(preferred_index).unwrap_or(&variants[0])
        } else {
            // ランダム選択
            variants.choose(&mut rng).unwrap_or(&variants[0])
        }
    }

    /// enum のインデックスを返す (FamiliarVoice用)
    pub fn index(&self) -> usize {
        match self {
            LatinPhrase::Veni => 0,
            LatinPhrase::Laborare => 1,
            LatinPhrase::Fodere => 2,
            LatinPhrase::Caede => 3,
            LatinPhrase::Portare => 4,
            LatinPhrase::Requiesce => 5,
            LatinPhrase::Abi => 6,
        }
    }

    /// フレーズの種類数
    pub const COUNT: usize = 7;
}
