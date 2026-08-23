use super::super::*;

pub(super) fn validate(candidate: &World) -> Result<(), String> {
    let mut pinned_source_owners: HashMap<Entity, usize> = HashMap::new();
    for entity in candidate.iter_entities() {
        if let Some(request) = entity.get::<TransportRequest>() {
            validate_transport_request_candidate(candidate, &entity, request)?;
            if let Some(source) = entity.get::<TransportRequestFixedSource>() {
                *pinned_source_owners.entry(source.0).or_default() += 1;
            }
        }
    }
    validate_manual_request_ownership(candidate, &pinned_source_owners)?;
    Ok(())
}

fn validate_transport_request_candidate(
    candidate: &World,
    entity: &EntityRef<'_>,
    request: &TransportRequest,
) -> Result<(), String> {
    macro_rules! require_component {
        ($type:ty) => {
            if !entity.contains::<$type>() {
                return Err(format!(
                    "TransportRequest {:?} is missing {}",
                    entity.id(),
                    std::any::type_name::<$type>()
                ));
            }
        };
    }

    require_component!(Transform);
    require_component!(ManagedBy);
    require_component!(TransportDemand);
    require_component!(TransportPolicy);

    // Producers disable a zero-demand request by removing this exact trio and
    // keep the request entity for in-flight accounting. Both the fully active
    // and fully disabled shapes are valid; a partial trio is corruption.
    let active_shape = [
        entity.contains::<Designation>(),
        entity.contains::<TaskSlots>(),
        entity.contains::<Priority>(),
    ];
    if active_shape.iter().any(|present| *present) && active_shape.iter().any(|present| !*present) {
        return Err(format!(
            "TransportRequest {:?} has a partial active task shape",
            entity.id()
        ));
    }
    let demand = entity
        .get::<TransportDemand>()
        .expect("required TransportDemand was checked above");
    if active_shape.iter().all(|present| *present) {
        let slots = entity
            .get::<TaskSlots>()
            .expect("active task shape includes TaskSlots");
        if slots.max == 0 || slots.max != demand.desired_slots {
            return Err(format!(
                "active TransportRequest {:?} has inconsistent TaskSlots/TransportDemand",
                entity.id()
            ));
        }
    }
    candidate
        .get_entity(request.anchor)
        .map_err(|_| format!("TransportRequest anchor {:?} is missing", request.anchor))?;
    let issuer = candidate
        .get_entity(request.issued_by)
        .map_err(|_| format!("TransportRequest issuer {:?} is missing", request.issued_by))?;
    if !issuer.contains::<Familiar>() && !issuer.contains::<Yard>() {
        return Err(format!(
            "TransportRequest issuer {:?} is neither a Familiar nor a Yard",
            request.issued_by
        ));
    }
    if entity
        .get::<ManagedBy>()
        .is_none_or(|managed_by| managed_by.0 != request.issued_by)
    {
        return Err(format!(
            "TransportRequest {:?} issuer and ManagedBy owner differ",
            entity.id()
        ));
    }
    for stockpile in &request.stockpile_group {
        let target = candidate.get_entity(*stockpile).map_err(|_| {
            format!("TransportRequest stockpile-group target {stockpile:?} is missing")
        })?;
        if !target.contains::<Stockpile>() {
            return Err(format!(
                "TransportRequest stockpile-group target {stockpile:?} is not a Stockpile"
            ));
        }
    }

    validate_request_target_shape(candidate, entity, request)?;

    let manual = entity.contains::<ManualTransportRequest>();
    let fixed_source = entity.get::<TransportRequestFixedSource>();
    if manual != fixed_source.is_some() {
        return Err(format!(
            "TransportRequest {:?} must pair ManualTransportRequest with exactly one fixed source",
            entity.id()
        ));
    }
    if manual
        && (!active_shape.iter().all(|present| *present)
            || request.kind != TransportRequestKind::DepositToStockpile
            || !request.stockpile_group.is_empty()
            || !issuer.contains::<Familiar>()
            || entity.get::<TaskSlots>().is_none_or(|slots| slots.max != 1)
            || demand.desired_slots != 1)
    {
        return Err(format!(
            "manual TransportRequest {:?} has a non-canonical active shape",
            entity.id()
        ));
    }
    if let Some(fixed_source) = fixed_source {
        let source = candidate.get_entity(fixed_source.0).map_err(|_| {
            format!(
                "manual TransportRequest fixed source {:?} is missing",
                fixed_source.0
            )
        })?;
        if !source.contains::<ResourceItem>() || !source.contains::<ManualHaulPinnedSource>() {
            return Err(format!(
                "manual TransportRequest fixed source {:?} is not a pinned ResourceItem",
                fixed_source.0
            ));
        }
        if source
            .get::<ResourceItem>()
            .is_none_or(|item| item.0 != request.resource_type)
        {
            return Err(format!(
                "manual TransportRequest {:?} resource type differs from its fixed source",
                entity.id()
            ));
        }
    }
    Ok(())
}

fn validate_request_target_shape(
    candidate: &World,
    entity: &EntityRef<'_>,
    request: &TransportRequest,
) -> Result<(), String> {
    let blueprint_target = entity.get::<TargetBlueprint>().map(|target| target.0);
    let floor_target = entity.get::<TargetFloorConstructionSite>();
    let wall_target = entity.get::<TargetWallConstructionSite>();
    let mixer_target = entity.get::<TargetMixer>().map(|target| target.0);
    let soul_spa_target = entity.get::<TargetSoulSpaSite>().map(|target| target.0);

    let expected_marker = match request.kind {
        TransportRequestKind::DeliverToBlueprint => Some("blueprint"),
        TransportRequestKind::DeliverToFloorConstruction => Some("floor"),
        TransportRequestKind::DeliverToWallConstruction => Some("wall"),
        TransportRequestKind::DeliverToMixerSolid | TransportRequestKind::DeliverWaterToMixer => {
            Some("mixer")
        }
        TransportRequestKind::DeliverToSoulSpa => Some("soul-spa"),
        _ => None,
    };
    for (label, target) in [
        ("blueprint", blueprint_target),
        ("floor", floor_target.map(|target| target.0)),
        ("wall", wall_target.map(|target| target.0)),
        ("mixer", mixer_target),
        ("soul-spa", soul_spa_target),
    ] {
        if let Some(target) = target
            && (expected_marker != Some(label) || target != request.anchor)
        {
            return Err(format!(
                "TransportRequest {:?} has an invalid {label} target marker",
                entity.id()
            ));
        }
    }

    let anchor = candidate
        .get_entity(request.anchor)
        .map_err(|_| format!("TransportRequest anchor {:?} is missing", request.anchor))?;
    let (anchor_valid, expected_work, resource_valid) = match request.kind {
        TransportRequestKind::DepositToStockpile => {
            (anchor.contains::<Stockpile>(), WorkType::Haul, true)
        }
        TransportRequestKind::DeliverToBlueprint => {
            (anchor.contains::<Blueprint>(), WorkType::Haul, true)
        }
        TransportRequestKind::DeliverToFloorConstruction => (
            anchor.contains::<FloorConstructionSite>(),
            WorkType::Haul,
            matches!(
                request.resource_type,
                hw_core::logistics::ResourceType::Bone
                    | hw_core::logistics::ResourceType::StasisMud
            ),
        ),
        TransportRequestKind::DeliverToWallConstruction => (
            anchor.contains::<WallConstructionSite>(),
            WorkType::Haul,
            matches!(
                request.resource_type,
                hw_core::logistics::ResourceType::Wood
                    | hw_core::logistics::ResourceType::StasisMud
            ),
        ),
        TransportRequestKind::DeliverToProvisionalWall => (
            anchor.contains::<Building>() && anchor.contains::<ProvisionalWall>(),
            WorkType::Haul,
            request.resource_type == hw_core::logistics::ResourceType::StasisMud,
        ),
        TransportRequestKind::DeliverToMixerSolid => (
            anchor.contains::<MudMixerStorage>(),
            WorkType::HaulToMixer,
            matches!(
                request.resource_type,
                hw_core::logistics::ResourceType::Sand | hw_core::logistics::ResourceType::Rock
            ),
        ),
        TransportRequestKind::DeliverWaterToMixer => (
            anchor.contains::<MudMixerStorage>(),
            WorkType::HaulWaterToMixer,
            request.resource_type == hw_core::logistics::ResourceType::Water,
        ),
        TransportRequestKind::GatherWaterToTank => (
            anchor.get::<Stockpile>().is_some_and(|stockpile| {
                stockpile.resource_type == Some(hw_core::logistics::ResourceType::Water)
            }),
            WorkType::GatherWater,
            request.resource_type == hw_core::logistics::ResourceType::Water,
        ),
        TransportRequestKind::ReturnBucket => (
            anchor.get::<Stockpile>().is_some_and(|stockpile| {
                stockpile.resource_type == Some(hw_core::logistics::ResourceType::Water)
            }),
            WorkType::Haul,
            request.resource_type == hw_core::logistics::ResourceType::BucketEmpty,
        ),
        TransportRequestKind::ReturnWheelbarrow | TransportRequestKind::BatchWheelbarrow => (
            anchor.contains::<Wheelbarrow>(),
            WorkType::WheelbarrowHaul,
            request.resource_type == hw_core::logistics::ResourceType::Wheelbarrow,
        ),
        TransportRequestKind::ConsolidateStockpile => {
            (anchor.contains::<Stockpile>(), WorkType::Haul, true)
        }
        TransportRequestKind::DeliverToSoulSpa => (
            anchor.contains::<SoulSpaSite>(),
            WorkType::Haul,
            request.resource_type == hw_core::logistics::ResourceType::Bone,
        ),
    };
    if !anchor_valid || !resource_valid {
        return Err(format!(
            "TransportRequest {:?} has an invalid anchor/resource shape for {:?}",
            entity.id(),
            request.kind
        ));
    }
    if let Some(designation) = entity.get::<Designation>()
        && designation.work_type != expected_work
    {
        return Err(format!(
            "TransportRequest {:?} has the wrong active WorkType",
            entity.id()
        ));
    }

    // Marker-less legacy payloads are accepted because kind+anchor is a complete
    // source of truth. DurableNormalize restores the marker before producers run.
    match request.kind {
        TransportRequestKind::DeliverToFloorConstruction if wall_target.is_some() => {
            return Err(format!(
                "floor TransportRequest {:?} also has a wall target",
                entity.id()
            ));
        }
        TransportRequestKind::DeliverToWallConstruction if floor_target.is_some() => {
            return Err(format!(
                "wall TransportRequest {:?} also has a floor target",
                entity.id()
            ));
        }
        _ => {}
    }
    Ok(())
}

fn validate_manual_request_ownership(
    candidate: &World,
    owners: &HashMap<Entity, usize>,
) -> Result<(), String> {
    for entity in candidate.iter_entities() {
        if (entity.contains::<ManualTransportRequest>()
            || entity.contains::<TransportRequestFixedSource>())
            && !entity.contains::<TransportRequest>()
        {
            return Err(format!(
                "manual request component is attached to non-request {:?}",
                entity.id()
            ));
        }
        if entity.contains::<ManualHaulPinnedSource>() {
            if !entity.contains::<ResourceItem>() {
                return Err(format!(
                    "ManualHaulPinnedSource {:?} is not a ResourceItem",
                    entity.id()
                ));
            }
            if owners.get(&entity.id()).copied().unwrap_or(0) != 1 {
                return Err(format!(
                    "ManualHaulPinnedSource {:?} must have exactly one request owner",
                    entity.id()
                ));
            }
        }
    }
    Ok(())
}
