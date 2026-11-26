//! Physical infrastructure that uses the construction and power workflows.
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum Equipment {
    SupplyFan,
    ExhaustFan,
    Chimney,
    Duct,
    Intake,
    Conduit,
    Basin,
    Hood,
    Filter,
    HeatPump,
    OutdoorUnit,
    Humidifier,
    Dehumidifier,
    Recuperator,
}

impl Equipment {
    pub const ALL: [Self; 14] = [
        Self::SupplyFan,
        Self::ExhaustFan,
        Self::Chimney,
        Self::Duct,
        Self::Intake,
        Self::Conduit,
        Self::Basin,
        Self::Hood,
        Self::Filter,
        Self::HeatPump,
        Self::OutdoorUnit,
        Self::Humidifier,
        Self::Dehumidifier,
        Self::Recuperator,
    ];
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::SupplyFan => "IN",
            Self::ExhaustFan => "OUT",
            Self::Chimney => "↑",
            Self::Duct => "↔",
            Self::Intake => "取",
            Self::Conduit => "━",
            Self::Basin => "浄",
            Self::Hood => "局",
            Self::Filter => "濾",
            Self::HeatPump => "温",
            Self::OutdoorUnit => "外",
            Self::Humidifier => "湿+",
            Self::Dehumidifier => "湿−",
            Self::Recuperator => "熱",
        }
    }
    pub const fn copy(self) -> (&'static str, &'static str) {
        match self {
            Self::SupplyFan => ("給気送風機", "風穴から給気"),
            Self::ExhaustFan => ("排気送風機", "風穴から排気"),
            Self::Chimney => ("溜息煙突", "温度差で排気"),
            Self::Duct => ("風道", "風穴と機器を接続"),
            Self::Intake => ("取水門", "河岸から取水"),
            Self::Conduit => ("骨の導水管", "地下で水を運ぶ"),
            Self::Basin => ("浄化盤", "給水して排泥"),
            Self::Hood => ("局所排気口", "発生源の瘴気を捕集"),
            Self::Filter => ("循環ろ過機", "室内の瘴気を除去"),
            Self::HeatPump => ("冷暖房室内機", "室温を自動調節"),
            Self::OutdoorUnit => ("室外機", "冷暖房の熱源"),
            Self::Humidifier => ("加湿器", "給水して加湿"),
            Self::Dehumidifier => ("除湿器", "凝縮水を排水"),
            Self::Recuperator => ("熱交換器", "給排気から熱回収"),
        }
    }
    pub const fn demand(self) -> f32 {
        match self {
            Self::SupplyFan | Self::ExhaustFan | Self::Hood | Self::Filter => 0.5,
            Self::Intake | Self::HeatPump => 1.0,
            Self::Humidifier | Self::Dehumidifier => 0.4,
            _ => 0.0,
        }
    }
    pub const fn indoor(self) -> bool {
        !matches!(
            self,
            Self::Intake | Self::Conduit | Self::OutdoorUnit | Self::Duct
        )
    }
}
