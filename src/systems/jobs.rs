use crate::assets::GameAssets;
use crate::constants::*;
use crate::world::map::WorldMap;
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;
use std::collections::HashMap;

// --- Events ---


// --- Components ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum BuildingType {
    #[default]
    Wall,
    Floor,
    Tank,
    MudMixer,
}

impl BuildingType {
    /// この建物タイプに必要な資材を返す
    pub fn required_materials(&self) -> HashMap<ResourceType, u32> {
        let mut materials = HashMap::new();
        match self {
            BuildingType::Wall => {
                materials.insert(ResourceType::Wood, 1);
            }
            BuildingType::Floor => {
                materials.insert(ResourceType::Rock, 1);
            }
            BuildingType::Tank => {
                materials.insert(ResourceType::Wood, 2);
            }
            BuildingType::MudMixer => {
                materials.insert(ResourceType::Wood, 4);
            }
        }
        materials
    }
}

#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
pub struct Building {
    pub kind: BuildingType,
    pub is_provisional: bool,
}

#[derive(Component)]
pub struct SandPile;

#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
pub struct MudMixerStorage {
    pub sand: u32,
    pub water: u32,
    pub rock: u32,
}

/// 資材の運搬先となる Blueprint を示すマーカー
#[derive(Component)]
pub struct TargetBlueprint(pub Entity);

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TargetMixer(pub Entity);

#[derive(Component)]
pub struct Tree;

#[derive(Component)]
pub struct Rock;

/// 障害物のグリッド座標を保持するコンポーネント
#[derive(Component)]
pub struct ObstaclePosition(pub i32, pub i32);

/// 設計図コンポーネント - 建設中の建物を表す
#[derive(Component)]
pub struct Blueprint {
    pub kind: BuildingType,
    /// 建築進捗 (0.0 to 1.0) - 資材が揃った後の建築作業の進捗
    pub progress: f32,
    /// 必要な資材量
    pub required_materials: HashMap<ResourceType, u32>,
    /// 搬入済みの資材量
    pub delivered_materials: HashMap<ResourceType, u32>,
    /// 占有するグリッド座標リスト
    pub occupied_grids: Vec<(i32, i32)>,
}

impl Blueprint {
    /// 新しい設計図を作成
    pub fn new(kind: BuildingType, occupied_grids: Vec<(i32, i32)>) -> Self {
        Self {
            kind,
            progress: 0.0,
            required_materials: kind.required_materials(),
            delivered_materials: HashMap::new(),
            occupied_grids,
        }
    }

    /// 資材が全て揃っているかチェック
    pub fn materials_complete(&self) -> bool {
        // 壁の場合、木材さえあれば建築作業開始は可能とする（仮設状態になる）
        if self.kind == BuildingType::Wall {
            let wood_delivered = self.delivered_materials.get(&ResourceType::Wood).unwrap_or(&0);
            let wood_required = self.required_materials.get(&ResourceType::Wood).unwrap_or(&1);
            return wood_delivered >= wood_required;
        }

        for (resource_type, required) in &self.required_materials {
            let delivered = self.delivered_materials.get(resource_type).unwrap_or(&0);
            if delivered < required {
                return false;
            }
        }
        true
    }

    /// 本来の全資材が揃っているか（仮設ではなく完全な状態か）
    pub fn is_fully_complete(&self) -> bool {
        for (resource_type, required) in &self.required_materials {
            let delivered = self.delivered_materials.get(resource_type).unwrap_or(&0);
            if delivered < required {
                return false;
            }
        }
        true
    }

    pub fn deliver_material(&mut self, resource_type: ResourceType, amount: u32) {
        *self.delivered_materials.entry(resource_type).or_insert(0) += amount;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum WorkType {
    #[default]
    Chop, // 伐採
    Mine,  // 採掘
    Build, // 建築
    Haul,  // 運搬
    GatherWater, // 水汲み
    CollectSand, // 砂採取
    Refine,      // 精製
    HaulWaterToMixer, // Tankから水をミキサーへ運ぶ
}

#[derive(Component)]
pub struct Designation {
    pub work_type: WorkType,
}

#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component, Default)]
pub struct Priority(pub u32);

#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component, Default)]
pub struct TaskSlots {
    pub max: u32,
}

impl TaskSlots {
    pub fn new(max: u32) -> Self {
        Self { max }
    }
}

// IssuedBy は relationships.rs の ManagedBy に移行
// 後方互換性のため、エイリアスを提供
pub use crate::relationships::ManagedBy as IssuedBy;

// --- Systems ---

pub fn building_completion_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
    mut q_blueprints: Query<(Entity, &Blueprint, &Transform)>,
    mut q_souls: Query<(&mut Transform, Entity), (With<crate::entities::damned_soul::DamnedSoul>, Without<Blueprint>)>,
) {
    for (entity, bp, transform) in q_blueprints.iter_mut() {
        // 資材が揃っていて、建築進捗が100%に達したら完成
        if bp.materials_complete() && bp.progress >= 1.0 {
            info!(
                "BUILDING: Completed at {:?} (materials: {:?})",
                transform.translation, bp.delivered_materials
            );
            commands.entity(entity).despawn();

            let (sprite_image, custom_size) = match bp.kind {
                BuildingType::Wall => (game_assets.wall_isolated.clone(), Vec2::splat(TILE_SIZE)),
                BuildingType::Floor => (game_assets.stone.clone(), Vec2::splat(TILE_SIZE)),
                BuildingType::Tank => (game_assets.tank_empty.clone(), Vec2::splat(TILE_SIZE * 2.0)),
                BuildingType::MudMixer => (game_assets.mud_mixer.clone(), Vec2::splat(TILE_SIZE * 2.0)),
            };

            let building_entity = commands.spawn((
                Building { 
                    kind: bp.kind,
                    is_provisional: !bp.is_fully_complete(),
                },
                Sprite {
                    image: sprite_image,
                    custom_size: Some(custom_size),
                    ..default()
                },
                *transform,
                Name::new(format!("Building ({:?})", bp.kind)),
                // Phase 5: バウンス効果
                crate::systems::visual::blueprint::BuildingBounceEffect {
                    bounce_animation: crate::systems::utils::animations::BounceAnimation {
                        timer: 0.0,
                        config: crate::systems::utils::animations::BounceAnimationConfig {
                            duration: crate::systems::visual::blueprint::BOUNCE_DURATION,
                            min_scale: 1.0,
                            max_scale: 1.2,
                        },
                    },
                },
            )).id();

            // 壁やタンクなどの建物の場合、通行不可設定を行う
            let is_obstacle = match bp.kind {
                BuildingType::Wall | BuildingType::Tank | BuildingType::MudMixer => true,
                BuildingType::Floor => false,
            };

            if is_obstacle {
                commands.entity(building_entity).with_children(|parent| {
                    for &(gx, gy) in &bp.occupied_grids {
                        parent.spawn((
                            ObstaclePosition(gx, gy),
                            Name::new("Building Obstacle"),
                        ));
                    }
                });

                for &(gx, gy) in &bp.occupied_grids {
                    world_map.add_obstacle(gx, gy);
                    // 完成した建物でマップ情報を更新（古いBlueprintのEntityIDを上書き）
                    world_map.buildings.insert((gx, gy), building_entity);
                }

                // --- ソウル埋まり解消の処理 ---
                // 新しい障害物のグリッドに重なっているソウルを検索して押し出す
                for &(gx, gy) in &bp.occupied_grids {
                    for (mut soul_transform, soul_entity) in q_souls.iter_mut() {
                        let soul_pos = soul_transform.translation.truncate();
                        let (sgx, sgy) = WorldMap::world_to_grid(soul_pos);

                        if sgx == gx && sgy == gy {
                            // 隣接する8マスから通行可能な場所を探す
                            let directions = [
                                (0, 1), (0, -1), (1, 0), (-1, 0),
                                (1, 1), (1, -1), (-1, 1), (-1, -1)
                            ];
                            
                            let mut found = false;
                            for (dx, dy) in directions {
                                let nx = gx + dx;
                                let ny = gy + dy;
                                
                                if world_map.is_walkable(nx, ny) && !bp.occupied_grids.contains(&(nx, ny)) {
                                    let new_pos = WorldMap::grid_to_world(nx, ny);
                                    soul_transform.translation.x = new_pos.x;
                                    soul_transform.translation.y = new_pos.y;
                                    info!("BUILD: Soul {:?} was pushed out to ({}, {})", soul_entity, nx, ny);
                                    found = true;
                                    break;
                                }
                            }
                            
                            if !found {
                                // 万が一周囲が全て通行不可の場合（非常に稀）、さらに遠くを探すか
                                // 建築予定マス以外で最も近いWalkableを探す必要があるが、
                                // 通常は建設作業員が隣にいるはずなので隣接マスが見つかるはず
                                warn!("BUILD: Soul {:?} is stuck and could not find simple push-out position!", soul_entity);
                            }
                        }
                    }
                }
            }

            // タンクが完成した場合、周囲にバケツを5つ生成し、貯水機能を追加
            if bp.kind == BuildingType::Tank {
                commands.entity(building_entity).insert(crate::systems::logistics::Stockpile {
                    capacity: 50,
                    resource_type: Some(crate::systems::logistics::ResourceType::Water),
                });

                // タンクの前方（下側）2マスをバケツ置き場（Stockpile）として設定
                // タンクの真下に配置する (bx, bx+1)
                let (bx, by) = WorldMap::world_to_grid(transform.translation.truncate());
                let storage_grids = [(bx, by - 2), (bx + 1, by - 2)];
                let mut storage_entities = Vec::new();

                for (gx, gy) in storage_grids {
                    let pos = WorldMap::grid_to_world(gx, gy);
                    let storage_entity = commands
                        .spawn((
                            crate::systems::logistics::Stockpile {
                                capacity: 10,
                                resource_type: None, // 所有権チェックで専用化するためResourceTypeはNoneでOK
                            },
                            crate::systems::logistics::BelongsTo(building_entity),
                            Sprite {
                                color: Color::srgba(1.0, 1.0, 0.0, 0.2),
                                custom_size: Some(Vec2::splat(TILE_SIZE)),
                                ..default()
                            },
                            Transform::from_xyz(pos.x, pos.y, Z_MAP + 0.01),
                            Name::new("Tank Bucket Storage"),
                        ))
                        .id();
                    world_map.stockpiles.insert((gx, gy), storage_entity);
                    storage_entities.push(storage_entity);
                }

                // バケツを5つ生成して専用ストレージに配分
                for i in 0..5 {
                    let storage_idx = if i < 3 { 0 } else { 1 };
                    let storage_entity = storage_entities[storage_idx];
                    let grid = storage_grids[storage_idx];
                    let base_pos = WorldMap::grid_to_world(grid.0, grid.1);
                    
                    // オフセットを削除し、グリッド中心に確実に配置する
                    // これにより「見た目は拾えそうだが論理的に遠い」といった問題を排除
                    let offset = Vec2::ZERO;

                    let spawn_pos = base_pos + offset;

                    commands.spawn((
                        crate::systems::logistics::ResourceItem(crate::systems::logistics::ResourceType::BucketEmpty),
                        crate::systems::logistics::BelongsTo(building_entity),
                        crate::relationships::StoredIn(storage_entity),
                        crate::systems::logistics::InStockpile(storage_entity),
                        crate::systems::jobs::Designation {
                            work_type: crate::systems::jobs::WorkType::GatherWater,
                        },
                        crate::systems::jobs::TaskSlots::new(1),
                        Sprite {
                            image: game_assets.bucket_empty.clone(),
                            custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                            ..default()
                        },
                        Transform::from_xyz(spawn_pos.x, spawn_pos.y, Z_ITEM_PICKUP),
                        Name::new("Empty Bucket (Tank Dedicated)"),
                    ));
                }
            }

            // MudMixer が完成した場合、原料ストレージを追加し、隣接マスに SandPile を生成
            if bp.kind == BuildingType::MudMixer {
                commands.entity(building_entity).insert(MudMixerStorage::default());

                // MudMixer (2x2) の周辺に2つの SandPile を生成
                let (bx, by) = WorldMap::world_to_grid(transform.translation.truncate());
                let sand_positions = [(bx - 2, by - 1), (bx - 2, by)];

                for (sx, sy) in sand_positions {
                    let pos = WorldMap::grid_to_world(sx, sy);
                    commands.spawn((
                        SandPile,
                        ObstaclePosition(sx, sy),
                        crate::systems::logistics::BelongsTo(building_entity), // MudMixer への参照
                        Sprite {
                            image: game_assets.sand_pile.clone(),
                            custom_size: Some(Vec2::splat(TILE_SIZE * 0.8)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, Z_ITEM_OBSTACLE),
                        Name::new("SandPile"),
                    ));
                    world_map.add_obstacle(sx, sy);
                }
            }

            // Phase 5: フローティングテキスト
            let completion_config = crate::systems::utils::floating_text::FloatingTextConfig {
                lifetime: crate::systems::visual::blueprint::COMPLETION_TEXT_LIFETIME,
                velocity: Vec2::new(0.0, 15.0),
                initial_color: Color::srgb(0.2, 1.0, 0.4),
                fade_out: true,
            };
            let completion_entity = crate::systems::utils::floating_text::spawn_floating_text(
                &mut commands,
                "Construction Complete!",
                transform.translation.truncate().extend(Z_FLOATING_TEXT)
                    + Vec3::new(0.0, 20.0, 0.0),
                completion_config.clone(),
                Some(16.0),
                game_assets.font_ui.clone(),
            );
            commands.entity(completion_entity).insert((
                crate::systems::visual::blueprint::CompletionText {
                    floating_text: crate::systems::utils::floating_text::FloatingText {
                        lifetime: completion_config.lifetime,
                        config: completion_config,
                    },
                },
                TextLayout::new_with_justify(Justify::Center),
            ));
        }
    }
}
