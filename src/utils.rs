use anyhow::{anyhow, Result, Ok};
use log::*;
use std::collections::HashSet;
use vulkanalia::vk::{ExtensionName, KhrSurfaceExtension};
use vulkanalia::{prelude::v1_0::*, vk::PhysicalDevice};
use thiserror::Error;

use crate::config::*;
use crate::appdata::AppData;

#[derive(Debug, Error)]
#[error("Missing {0}")]
pub struct SuitabilityError(pub &'static str);

#[derive(Debug, Clone, Copy)]
pub struct QueueFamilyIndices {
    pub graphics: u32,
    pub present: u32,
}

impl QueueFamilyIndices {
    pub unsafe fn get(instance: &Instance, data: &AppData, pdev: PhysicalDevice) -> Result<Self> {
        let props = instance.get_physical_device_queue_family_properties(pdev);
        let graphics = props.iter()
            .position(|x| x.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .map(|x| x as u32);
        let mut present = None;
        for (index, _) in props.iter().enumerate() {
            if instance.get_physical_device_surface_support_khr(pdev, index as u32, data.surface,)? {
                present = Some(index as u32);
                break;
            }
        }
        if let (Some(graphics), Some(present)) = (graphics, present) {
            Ok(Self{ graphics, present })
        } else {
            Err(anyhow!(SuitabilityError("SBBB")))
        }
    }
}


pub unsafe fn pick_physical_device(instance: &Instance, data: &mut AppData) -> Result<()> {
    for pdev in instance.enumerate_physical_devices()? {
        let prop = instance.get_physical_device_properties(pdev);
        if let Err(error) = check_physical_device(instance, data, pdev) {
            warn!("skip {}", prop.device_name);
        } else {
            info!("Selected physical device (`{}`).", prop.device_name);
            data.physical_device = pdev;
            return Ok(());
        }
    }
    Err(anyhow!("Failed to find suitable physical device."))
}

pub unsafe fn check_physical_device(instance: &Instance, data: &AppData, pdev: PhysicalDevice) -> Result<()> {
    let props = instance.get_physical_device_properties(pdev);
    println!("physical device: {}", props.device_name);
    QueueFamilyIndices::get(instance, data, pdev)?;
    Ok(())
}

pub unsafe fn create_logical_device(instance: &Instance, data: &mut AppData) -> Result<Device> {
    let indices = QueueFamilyIndices::get(instance, data, data.physical_device)?;
    let queue_priorities = &[1.0];
    let mut unique_indices = HashSet::new();
    unique_indices.insert(indices.graphics);
    unique_indices.insert(indices.present);

    let queue_priorities = &[1.0];
    let queue_infos = unique_indices
        .iter()
        .map(|i| {
            vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(*i)
                .queue_priorities(queue_priorities)
        })
        .collect::<Vec<_>>();
    let layers = if VALIDATION_ENABLED {
        vec![VALIDATION_LAYER.as_ptr()]
    } else {
        vec![]
    };

    let extensions = [ExtensionName::from_bytes(b"VK_KHR_portability_subset").as_ptr()];

    let features = vk::PhysicalDeviceFeatures::builder();
    let info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_infos)
        .enabled_layer_names(&layers)
        .enabled_features(&features)
        .enabled_extension_names(&extensions);
    let device = instance.create_device(data.physical_device, &info, None)?;
    data.graphics_queue = device.get_device_queue(indices.graphics, 0);
    data.present_queue = device.get_device_queue(indices.present, 0);
    Ok(device)
}


