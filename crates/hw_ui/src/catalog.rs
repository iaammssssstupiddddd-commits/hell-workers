//! Shared player-facing names and purposes for build cards and placement guidance.
use hw_jobs::BuildingType;

pub const fn building_copy(kind: BuildingType) -> (&'static str, &'static str) {
    match kind {
        BuildingType::Wall => ("壁", "部屋を囲う"),
        BuildingType::Floor => ("床", "部屋の土台"),
        BuildingType::Door => ("ドア", "部屋の出入口"),
        BuildingType::Bridge => ("橋", "水辺を渡る"),
        BuildingType::Tank => ("水タンク", "水を保管する"),
        BuildingType::MudMixer => ("泥ミキサー", "建築用の泥"),
        BuildingType::RestArea => ("休息所", "魂が休む場所"),
        BuildingType::SandPile => ("砂置き場", "砂の供給源"),
        BuildingType::BonePile => ("骨置き場", "骨の供給源"),
        BuildingType::WheelbarrowParking => ("手押し車置場", "運搬車の帰還先"),
        BuildingType::SoulSpa => ("Soul Spa", "魂の力で発電"),
        BuildingType::OutdoorLamp => ("屋外灯", "周囲を照らす"),
    }
}
