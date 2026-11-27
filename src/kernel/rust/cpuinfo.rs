//! CPU identification and topology collection via `raw-cpuid`.
//!
//! CPUID only describes the package the boot processor belongs to. The counts
//! below are therefore per package until the kernel parses ACPI/MP tables.

use crate::console::BootConsole;
use core::fmt::Write;
use raw_cpuid::{CpuId, TopologyType};

#[derive(Clone, Copy, PartialEq, Eq)]
enum TopologySource {
    Leaf1F,
    Leaf0B,
    CacheLeaf4,
    Leaf1,
    Unknown,
}

impl TopologySource {
    fn label(self) -> &'static str {
        match self {
            Self::Leaf1F => "leaf 0x1F",
            Self::Leaf0B => "leaf 0x0B",
            Self::CacheLeaf4 => "cache leaf 4",
            Self::Leaf1 => "leaf 1",
            Self::Unknown => "unavailable",
        }
    }
}

/// Topology and identification data collected from the boot processor.
#[derive(Clone, Copy)]
pub(super) struct CpuInfo {
    vendor: [u8; 12],
    vendor_len: usize,
    brand: [u8; 48],
    brand_len: usize,
    family: u8,
    model: u8,
    stepping: u8,
    initial_apic_id: u8,
    x2apic_id: Option<u32>,
    physical_cores: u16,
    logical_processors: u16,
    threads_per_core: u16,
    apic: bool,
    x2apic: bool,
    htt: bool,
    hypervisor: bool,
    topology_source: TopologySource,
}

impl CpuInfo {
    fn new() -> Self {
        Self {
            vendor: [0; 12],
            vendor_len: 0,
            brand: [0; 48],
            brand_len: 0,
            family: 0,
            model: 0,
            stepping: 0,
            initial_apic_id: 0,
            x2apic_id: None,
            physical_cores: 0,
            logical_processors: 0,
            threads_per_core: 0,
            apic: false,
            x2apic: false,
            htt: false,
            hypervisor: false,
            topology_source: TopologySource::Unknown,
        }
    }
}

fn copy_string(dst: &mut [u8], len: &mut usize, src: &str) {
    let bytes = src.as_bytes();
    let n = bytes.len().min(dst.len());
    dst[..n].copy_from_slice(&bytes[..n]);
    *len = n;
}

fn as_str(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).unwrap_or("unknown")
}

/// Query CPUID once and derive the topology usable for early boot.
pub(super) fn collect() -> CpuInfo {
    let cpuid = CpuId::new();
    let mut info = CpuInfo::new();

    if let Some(vendor) = cpuid.get_vendor_info() {
        copy_string(&mut info.vendor, &mut info.vendor_len, vendor.as_str());
    }
    if let Some(brand) = cpuid.get_processor_brand_string() {
        copy_string(&mut info.brand, &mut info.brand_len, brand.as_str());
    }

    let features = cpuid.get_feature_info();
    if let Some(f) = &features {
        info.family = f.family_id();
        info.model = f.model_id();
        info.stepping = f.stepping_id();
        info.initial_apic_id = f.initial_local_apic_id();
        info.apic = f.has_apic();
        info.x2apic = f.has_x2apic();
        info.htt = f.has_htt();
        info.hypervisor = f.has_hypervisor();
    }

    // Intel recommends leaf 0x1F over 0x0B when both are available. Both
    // describe the current package and enumerate SMT/core levels the same way.
    let extended = cpuid
        .get_extended_topology_info_v2()
        .or_else(|| cpuid.get_extended_topology_info());
    if let Some(levels) = extended {
        for level in levels {
            match level.level_type() {
                TopologyType::SMT => info.threads_per_core = level.processors(),
                TopologyType::Core => info.logical_processors = level.processors(),
                _ => {}
            }
            if info.x2apic_id.is_none() {
                info.x2apic_id = Some(level.x2apic_id());
            }
        }
        if info.logical_processors > 0 {
            let threads = info.threads_per_core.max(1);
            if info.logical_processors >= threads {
                info.physical_cores = info.logical_processors / threads;
            }
        }
    }

    if info.physical_cores == 0 {
        if let Some(caches) = cpuid.get_cache_parameters() {
            for cache in caches {
                if cache.level() != 1 {
                    continue;
                }
                // Leaf 4 reports physical cores in ECX[31:26] and logical
                // processors sharing this cache in ECX[25:0]; they are not
                // the same quantity and must not be divided.
                let cores = cache.max_cores_for_package();
                let sharing = cache.max_cores_for_cache();
                if cores > 0 && sharing > 0 {
                    info.physical_cores = cores as u16;
                    info.threads_per_core = sharing as u16;
                    info.logical_processors = cores.saturating_mul(sharing) as u16;
                    break;
                }
            }
            if info.physical_cores > 0 {
                info.topology_source = TopologySource::CacheLeaf4;
            }
        }
    } else {
        info.topology_source = if cpuid.get_extended_topology_info_v2().is_some() {
            TopologySource::Leaf1F
        } else {
            TopologySource::Leaf0B
        };
    }

    if info.physical_cores == 0 {
        if let Some(f) = &features {
            info.logical_processors = if f.has_htt() {
                u16::from(f.max_logical_processor_ids())
            } else {
                1
            };
            info.physical_cores = info.logical_processors;
            info.threads_per_core = 1;
            info.topology_source = TopologySource::Leaf1;
        }
    }

    if info.threads_per_core == 0 {
        info.threads_per_core = 1;
    }

    info
}

/// Print the collected data on the early boot console.
pub(super) fn report(console: &mut BootConsole, info: &CpuInfo) {
    let vendor = as_str(&info.vendor[..info.vendor_len]);
    let brand = as_str(&info.brand[..info.brand_len]);

    let _ = writeln!(console, "CPU: {vendor} | {brand}");
    let _ = writeln!(
        console,
        "CPU ID: family {} model {} stepping {}",
        info.family, info.model, info.stepping
    );
    let _ = writeln!(
        console,
        "CPU topology: {} physical cores, {} logical processors, {} thread(s)/core (per package)",
        info.physical_cores, info.logical_processors, info.threads_per_core
    );
    let _ = writeln!(
        console,
        "APIC: initial id {}, x2APIC id {}, apic {}, x2apic {}, htt {}, hypervisor {}",
        info.initial_apic_id,
        info.x2apic_id.map_or(u32::from(info.initial_apic_id), |id| id),
        info.apic,
        info.x2apic,
        info.htt,
        info.hypervisor
    );
    let _ = writeln!(console, "Topology source: {}", info.topology_source.label());
}
