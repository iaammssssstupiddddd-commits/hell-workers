//! Instantaneous device-allocator observations, never whole-process GPU usage.
use bevy::prelude::*;
use bevy::render::renderer::{RenderAdapterInfo, RenderDevice};
use serde_json::{Value, json};

const SOURCE: &str = "wgpu-vulkan-allocator";

fn unavailable(reason: &str) -> Value {
    json!({"schema": 1, "source": SOURCE, "status": "unavailable", "reason": reason,
        "measurement_scope": "device-allocator-instantaneous",
        "allocated_bytes": null, "reserved_bytes": null,
        "allocation_count": null, "block_count": null, "adapter": null})
}

fn summarize(adapter: &wgpu::AdapterInfo, report: Option<wgpu::AllocatorReport>) -> Value {
    if adapter.backend != wgpu::Backend::Vulkan {
        return unavailable("non-vulkan-backend");
    }
    let Some(report) = report else {
        return unavailable("allocator-report-unsupported");
    };
    // The API defines these totals as the sums of live allocation/block sizes.
    // Check arithmetic and preserve the two distinct units; never substitute
    // RSS, compressed image sizes, reserved capacity, or a missing report.
    let allocated = report
        .allocations
        .iter()
        .try_fold(0_u64, |sum, row| sum.checked_add(row.size));
    let reserved = report
        .blocks
        .iter()
        .try_fold(0_u64, |sum, row| sum.checked_add(row.size));
    if allocated != Some(report.total_allocated_bytes)
        || reserved != Some(report.total_reserved_bytes)
        || report.total_allocated_bytes > report.total_reserved_bytes
        || report.blocks.iter().any(|block| {
            block.allocations.start > block.allocations.end
                || block.allocations.end > report.allocations.len()
        })
    {
        return unavailable("allocator-report-accounting-differs");
    }
    json!({"schema": 1, "source": SOURCE, "status": "available", "reason": null,
        "measurement_scope": "device-allocator-instantaneous",
        "allocated_bytes": report.total_allocated_bytes,
        "reserved_bytes": report.total_reserved_bytes,
        "allocation_count": report.allocations.len(), "block_count": report.blocks.len(),
        "adapter": {"name": adapter.name, "backend": "Vulkan",
            "vendor": adapter.vendor, "device": adapter.device,
            "pci_bus_id": adapter.device_pci_bus_id,
            "driver": adapter.driver, "driver_info": adapter.driver_info}})
}

pub(super) fn snapshot(world: &World) -> Value {
    let Some(adapter) = world.get_resource::<RenderAdapterInfo>() else {
        return unavailable("render-adapter-unavailable");
    };
    let Some(device) = world.get_resource::<RenderDevice>() else {
        return unavailable("render-device-unavailable");
    };
    if adapter.backend != wgpu::Backend::Vulkan {
        return unavailable("non-vulkan-backend");
    }
    // Generates allocation metadata under the backend allocator lock. Keep its
    // observation overhead visible; this is not a GPU-timestamp or peak probe.
    summarize(adapter, device.wgpu_device().generate_allocator_report())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> wgpu::AdapterInfo {
        wgpu::AdapterInfo {
            name: "test adapter".into(),
            vendor: 0x8086,
            device: 1,
            device_type: wgpu::DeviceType::IntegratedGpu,
            device_pci_bus_id: "test-bus".into(),
            driver: "test driver".into(),
            driver_info: "test revision".into(),
            backend: wgpu::Backend::Vulkan,
            subgroup_min_size: 8,
            subgroup_max_size: 32,
            transient_saves_memory: false,
        }
    }

    fn report() -> wgpu::AllocatorReport {
        wgpu::AllocatorReport {
            allocations: vec![wgpu::wgt::AllocationReport {
                name: "test".into(),
                offset: 0,
                size: 64,
            }],
            blocks: vec![wgpu::wgt::MemoryBlockReport {
                size: 256,
                allocations: 0..1,
            }],
            total_allocated_bytes: 64,
            total_reserved_bytes: 256,
        }
    }

    #[test]
    fn missing_or_unsupported_report_never_becomes_zero_measurement() {
        assert_eq!(snapshot(&World::new())["status"], "unavailable");
        let missing = summarize(&adapter(), None);
        assert!(missing["allocated_bytes"].is_null());
        assert!(missing["reserved_bytes"].is_null());
        let foreign = summarize(
            &wgpu::AdapterInfo {
                backend: wgpu::Backend::Gl,
                ..adapter()
            },
            Some(report()),
        );
        assert_eq!(foreign["status"], "unavailable");
        assert!(foreign["allocated_bytes"].is_null());
    }

    #[test]
    fn live_allocations_and_reserved_capacity_remain_distinct() {
        let value = summarize(&adapter(), Some(report()));
        assert_eq!(value["status"], "available");
        assert_eq!(value["allocated_bytes"], 64);
        assert_eq!(value["reserved_bytes"], 256);
        assert_eq!(value["measurement_scope"], "device-allocator-instantaneous");
        let mut inconsistent = report();
        inconsistent.total_allocated_bytes = 65;
        assert_eq!(
            summarize(&adapter(), Some(inconsistent))["status"],
            "unavailable"
        );
        let mut invalid_range = report();
        invalid_range.blocks[0].allocations = 0..2;
        assert_eq!(
            summarize(&adapter(), Some(invalid_range))["status"],
            "unavailable"
        );
    }

    #[test]
    fn zero_is_valid_only_when_an_actual_consistent_report_exists() {
        let empty = wgpu::AllocatorReport {
            allocations: vec![],
            blocks: vec![],
            total_allocated_bytes: 0,
            total_reserved_bytes: 0,
        };
        let value = summarize(&adapter(), Some(empty));
        assert_eq!(value["status"], "available");
        assert_eq!(value["allocated_bytes"], 0);
        assert_ne!(value, summarize(&adapter(), None));
    }

    #[test]
    fn overflow_capacity_mismatch_and_invalid_block_ranges_are_unavailable() {
        let mut overflow = report();
        overflow.allocations.push(wgpu::wgt::AllocationReport {
            name: "overflow".into(),
            offset: 0,
            size: u64::MAX,
        });
        assert_eq!(
            summarize(&adapter(), Some(overflow))["status"],
            "unavailable"
        );
        let mut wrong_capacity = report();
        wrong_capacity.total_reserved_bytes = 257;
        assert_eq!(
            summarize(&adapter(), Some(wrong_capacity))["status"],
            "unavailable"
        );
        let mut insufficient_capacity = report();
        insufficient_capacity.blocks[0].size = 32;
        insufficient_capacity.total_reserved_bytes = 32;
        assert_eq!(
            summarize(&adapter(), Some(insufficient_capacity))["status"],
            "unavailable"
        );
        let mut reversed = report();
        reversed.blocks[0].allocations = std::ops::Range { start: 1, end: 0 };
        assert_eq!(
            summarize(&adapter(), Some(reversed))["status"],
            "unavailable"
        );
    }
}
