//! root full-fat query bridge。
//!
//! narrow query 5型（SoulSquadQuery / SoulSupervisingQuery / SoulScoutingQuery /
//! SoulRecruitmentQuery / SoulEncouragementQuery）および
//! FamiliarStateQuery / FamiliarSoulQuery / FamiliarTaskQuery は
//! hw_familiar_ai::familiar_ai::decide::query_types に定義済みで `pub use` から re-export する。

pub use hw_familiar_ai::familiar_ai::decide::query_types::{
    FamiliarSoulQuery, FamiliarStateQuery, FamiliarTaskQuery, SoulEncouragementQuery,
    SoulRecruitmentQuery, SoulScoutingQuery, SoulSquadQuery, SoulSupervisingQuery,
};
