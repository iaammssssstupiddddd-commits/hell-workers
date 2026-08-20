/// ビジュアルテスト内で建築物種別を選択するためのローカル enum。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TestBuildingKind {
    #[default]
    Wall,
    Door,
    Floor,
    Tank,
    MudMixer,
    RestArea,
    Bridge,
    SandPile,
    BonePile,
    WheelbarrowParking,
    SoulSpa,
}

impl TestBuildingKind {
    pub const ALL: [Self; 11] = [
        Self::Wall,
        Self::Door,
        Self::Floor,
        Self::Tank,
        Self::MudMixer,
        Self::RestArea,
        Self::Bridge,
        Self::SandPile,
        Self::BonePile,
        Self::WheelbarrowParking,
        Self::SoulSpa,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Wall => "Wall",
            Self::Door => "Door",
            Self::Floor => "Floor",
            Self::Tank => "Tank",
            Self::MudMixer => "MudMixer",
            Self::RestArea => "RestArea",
            Self::Bridge => "Bridge",
            Self::SandPile => "SandPile",
            Self::BonePile => "BonePile",
            Self::WheelbarrowParking => "WheelbarrowParking",
            Self::SoulSpa => "SoulSpa",
        }
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&v| v == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let idx = Self::ALL.iter().position(|&v| v == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}
