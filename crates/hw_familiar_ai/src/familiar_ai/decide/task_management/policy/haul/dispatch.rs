use hw_logistics::transport_request::TransportRequestKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HaulRoute {
    Blueprint,
    ReturnBucket,
    ReturnWheelbarrow,
    ProvisionalWall,
    FloorConstruction,
    WallConstruction,
    SoulSpa,
    Stockpile,
    Consolidation,
}

pub(super) const fn route_haul_kind(kind: TransportRequestKind) -> Option<HaulRoute> {
    match kind {
        TransportRequestKind::DeliverToBlueprint => Some(HaulRoute::Blueprint),
        TransportRequestKind::ReturnBucket => Some(HaulRoute::ReturnBucket),
        TransportRequestKind::ReturnWheelbarrow => Some(HaulRoute::ReturnWheelbarrow),
        TransportRequestKind::DeliverToProvisionalWall => Some(HaulRoute::ProvisionalWall),
        TransportRequestKind::DeliverToFloorConstruction => Some(HaulRoute::FloorConstruction),
        TransportRequestKind::DeliverToWallConstruction => Some(HaulRoute::WallConstruction),
        TransportRequestKind::DeliverToSoulSpa => Some(HaulRoute::SoulSpa),
        TransportRequestKind::DepositToStockpile => Some(HaulRoute::Stockpile),
        TransportRequestKind::ConsolidateStockpile => Some(HaulRoute::Consolidation),
        TransportRequestKind::DeliverToMixerSolid
        | TransportRequestKind::DeliverWaterToMixer
        | TransportRequestKind::GatherWaterToTank
        | TransportRequestKind::BatchWheelbarrow => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_transport_request_kind_has_an_explicit_haul_route_decision() {
        let expected = [
            Some(HaulRoute::Stockpile),
            Some(HaulRoute::Blueprint),
            Some(HaulRoute::FloorConstruction),
            Some(HaulRoute::WallConstruction),
            Some(HaulRoute::ProvisionalWall),
            None,
            None,
            None,
            Some(HaulRoute::ReturnBucket),
            Some(HaulRoute::ReturnWheelbarrow),
            None,
            Some(HaulRoute::Consolidation),
            Some(HaulRoute::SoulSpa),
        ];

        assert_eq!(TransportRequestKind::ALL.map(route_haul_kind), expected);
    }
}
