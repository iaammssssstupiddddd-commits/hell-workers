//! セーブ/ロード対象になる「シミュレーション実体」を選別するヘルパー。
//!
//! カメラ・UI エンティティはここに列挙したマーカーコンポーネントのいずれも
//! 持たないため、この一覧に絞り込むことで `Transform` のような汎用コンポーネントを
//! allow-list に含めても巻き込まれない（`extract_entities` は渡された entity の
//! 集合だけを走査するため、型レベルの許可リストとは独立して対象を絞れる）。

use std::collections::HashSet;

use bevy::prelude::*;

use crate::world::map::Tile;

use hw_core::area::TaskArea;
use hw_core::familiar::Familiar;
use hw_core::soul::DamnedSoul;

use hw_jobs::construction::{
    FloorConstructionSite, FloorTileBlueprint, WallConstructionSite, WallTileBlueprint,
};
use hw_jobs::{Blueprint, Building, Designation, Door, RestArea, Rock, Tree};

use hw_energy::{PowerConsumer, PowerGenerator, PowerGrid, SoulSpaSite, SoulSpaTile};

use hw_logistics::transport_request::TransportRequest;
use hw_logistics::types::WheelbarrowParking;
use hw_logistics::zone::Stockpile;
use hw_logistics::{ResourceItem, Wheelbarrow};

fn ids_with<T: Component>(world: &mut World, out: &mut HashSet<Entity>) {
    let mut query = world.query_filtered::<Entity, With<T>>();
    out.extend(query.iter(world));
}

/// セーブ/ロードの対象になる全エンティティ（マーカーコンポーネントで判定）を集める。
pub fn collect_persisted_entities(world: &mut World) -> Vec<Entity> {
    let mut set: HashSet<Entity> = HashSet::new();

    ids_with::<DamnedSoul>(world, &mut set);
    ids_with::<Familiar>(world, &mut set);
    ids_with::<Designation>(world, &mut set);
    ids_with::<Building>(world, &mut set);
    ids_with::<Door>(world, &mut set);
    ids_with::<RestArea>(world, &mut set);
    ids_with::<TaskArea>(world, &mut set);
    ids_with::<Blueprint>(world, &mut set);
    ids_with::<FloorConstructionSite>(world, &mut set);
    ids_with::<FloorTileBlueprint>(world, &mut set);
    ids_with::<WallConstructionSite>(world, &mut set);
    ids_with::<WallTileBlueprint>(world, &mut set);
    ids_with::<ResourceItem>(world, &mut set);
    ids_with::<Wheelbarrow>(world, &mut set);
    ids_with::<WheelbarrowParking>(world, &mut set);
    ids_with::<Stockpile>(world, &mut set);
    ids_with::<TransportRequest>(world, &mut set);
    ids_with::<PowerGrid>(world, &mut set);
    ids_with::<PowerGenerator>(world, &mut set);
    ids_with::<PowerConsumer>(world, &mut set);
    ids_with::<SoulSpaSite>(world, &mut set);
    ids_with::<SoulSpaTile>(world, &mut set);
    ids_with::<Tree>(world, &mut set);
    ids_with::<Rock>(world, &mut set);
    ids_with::<Tile>(world, &mut set);
    ids_with::<hw_world::zones::Site>(world, &mut set);
    ids_with::<hw_world::zones::Yard>(world, &mut set);

    set.into_iter().collect()
}
