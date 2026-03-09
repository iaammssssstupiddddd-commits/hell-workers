use hw_core::constants::BUCKET_CAPACITY;

pub fn projected_tank_water(current_water: usize, incoming_bucket_deliveries: usize) -> usize {
    current_water
        .saturating_add(incoming_bucket_deliveries.saturating_mul(BUCKET_CAPACITY as usize))
}

pub fn tank_can_accept_new_bucket(
    current_water: usize,
    incoming_bucket_deliveries: usize,
    capacity: usize,
) -> bool {
    projected_tank_water(current_water, incoming_bucket_deliveries)
        .saturating_add(BUCKET_CAPACITY as usize)
        <= capacity
}

pub fn tank_has_capacity_for_full_bucket(current_water: usize, capacity: usize) -> bool {
    current_water.saturating_add(BUCKET_CAPACITY as usize) <= capacity
}
