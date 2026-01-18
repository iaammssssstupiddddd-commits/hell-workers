/// Familiar が喋るラテン語のフレーズ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub fn as_str(&self) -> &'static str {
        match self {
            LatinPhrase::Veni => "Veni!",
            LatinPhrase::Laborare => "Laborare!",
            LatinPhrase::Fodere => "Fodere!",
            LatinPhrase::Caede => "Caede!",
            LatinPhrase::Portare => "Portare!",
            LatinPhrase::Requiesce => "Requiesce!",
            LatinPhrase::Abi => "Abi!",
        }
    }
}
