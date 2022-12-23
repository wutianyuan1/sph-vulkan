use anyhow::{anyhow, Result, Ok};
use log::*;
use vulkanalia::vk::ExtensionName;
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
}

impl QueueFamilyIndices {
    pub unsafe fn get(instance: &Instance, data: &AppData, pdev: PhysicalDevice) -> Result<Self> {
        let props = instance.get_physical_device_queue_family_properties(pdev);
        let graphics = props.iter()
            .position(|x| x.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .map(|x| x as u32);
        if let Some(graphics) = graphics {
            Ok(Self{ graphics })
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
    // if props.device_type != vk::PhysicalDeviceType::DISCRETE_GPU {
    //     return Err(anyhow!(SuitabilityError("Only discrete GPUs are supported.")));
    // }
    // let features = instance.get_physical_device_features(pdev);
    // if features.geometry_shader != vk::TRUE {
    //     return Err(anyhow!(SuitabilityError("Missing geometry shader support.")));
    // }
    QueueFamilyIndices::get(instance, data, pdev)?;
    Ok(())
}

pub unsafe fn create_logical_device(instance: &Instance, data: &mut AppData) -> Result<Device> {
    let indices = QueueFamilyIndices::get(instance, data, data.physical_device)?;
    let queue_priorities = &[1.0];
    let queue_info = vk::DeviceQueueCreateInfo::builder()
        .queue_family_index(indices.graphics)
        .queue_priorities(queue_priorities);
    let layers = if VALIDATION_ENABLED {
        vec![VALIDATION_LAYER.as_ptr()]
    } else {
        vec![]
    };

    let extensions = [ExtensionName::from_bytes(b"VK_KHR_portability_subset").as_ptr()];

    let features = vk::PhysicalDeviceFeatures::builder();
    let queue_infos = &[queue_info];
    let info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(queue_infos)
        .enabled_layer_names(&layers)
        .enabled_features(&features)
        .enabled_extension_names(&extensions);
    print!("{}", info.enabled_extension_count);
    for i in 0..info.enabled_extension_count {
        print!("{}", i);
    }
    let device = instance.create_device(data.physical_device, &info, None)?;
    data.graphics_queue = device.get_device_queue(indices.graphics, 0);
    Ok(device)
}


