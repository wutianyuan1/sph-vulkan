use anyhow::{anyhow, Result, Ok};
use log::*;
use std::collections::HashSet;
use vulkanalia::vk::{KhrSurfaceExtension, KhrSwapchainExtension};
use vulkanalia::{prelude::v1_0::*, vk::PhysicalDevice};
use thiserror::Error;
use winit::window::Window;

use crate::config::*;
use crate::appdata::AppData;
use crate::config::DEVICE_EXTENSIONS;

#[derive(Debug, Error)]
#[error("Missing {0}")]
pub struct SuitabilityError(pub &'static str);

#[derive(Debug, Clone, Copy)]
pub struct QueueFamilyIndices {
    pub graphics: u32,
    pub present: u32,
}

#[derive(Debug, Clone)]
pub struct SwapchainSupport {
    capabilities: vk::SurfaceCapabilitiesKHR,
    formats: Vec<vk::SurfaceFormatKHR>,
    present_modes: Vec<vk::PresentModeKHR>,
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

impl SwapchainSupport {
    pub unsafe fn get(instance: &Instance, data: &AppData, pdev: PhysicalDevice) -> Result<Self> {
        Ok(Self { 
            capabilities: instance.get_physical_device_surface_capabilities_khr(pdev, data.surface)?, 
            formats: instance.get_physical_device_surface_formats_khr(pdev, data.surface)?, 
            present_modes: instance.get_physical_device_surface_present_modes_khr(pdev, data.surface)?,
        })
    }
}


/// Physical Device helpers
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

unsafe fn check_physical_device(instance: &Instance, data: &AppData, pdev: PhysicalDevice) -> Result<()> {
    let props = instance.get_physical_device_properties(pdev);
    QueueFamilyIndices::get(instance, data, pdev)?;
    check_physical_device_extensions(instance, pdev)?;
    let swapchain_support = SwapchainSupport::get(instance, data, pdev)?;
    if swapchain_support.formats.is_empty() || swapchain_support.present_modes.is_empty() {
        return Err(anyhow!(SuitabilityError("No Swapchain Support!")));
    }
    println!("physical device: {} OK!", props.device_name);
    Ok(())
}

unsafe fn check_physical_device_extensions(instance: &Instance, pdev: vk::PhysicalDevice,
) -> Result<()> {
    let extensions = instance.enumerate_device_extension_properties(pdev, None)?
        .iter().map(|e| e.extension_name).collect::<HashSet<_>>();
    if DEVICE_EXTENSIONS.iter().all(|e| extensions.contains(e)) {
        Ok(())
    } else {
        Err(anyhow!(SuitabilityError("Missing required device extensions.")))
    }
}

/// Logical Device helpers
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

    let extensions = DEVICE_EXTENSIONS.iter().map(|e| e.as_ptr()).collect::<Vec<_>>();
    
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

/// Swapchain helpers
pub fn get_swapchain_surface_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
    formats.iter().cloned()
        .find(|fmt| fmt.format == vk::Format::B8G8R8A8_SRGB && fmt.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
        .unwrap_or_else(|| formats[0])
}

pub fn get_swapchain_present_mode(present_modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    present_modes.iter().cloned()
        .find(|m| *m == vk::PresentModeKHR::MAILBOX)
        .unwrap_or(vk::PresentModeKHR::FIFO)
}

pub fn get_swapchain_extent(window: &Window, capabilities: vk::SurfaceCapabilitiesKHR) -> vk::Extent2D {
    if capabilities.current_extent.width != u32::max_value() {
        capabilities.current_extent
    } else {
        let size = window.inner_size();
        let clamp = |min: u32, max: u32, v: u32| min.max(max.min(v));
        vk::Extent2D::builder()
            .width(clamp(
                capabilities.min_image_extent.width,
                capabilities.max_image_extent.width,
                size.width,
            ))
            .height(clamp(
                capabilities.min_image_extent.height,
                capabilities.max_image_extent.height,
                size.height,
            ))
            .build()
    }
}

pub unsafe fn create_swapchain(window: &Window, instance: &Instance, device: &Device, data: &mut AppData) -> Result<()> {
    let indices = QueueFamilyIndices::get(instance, data, data.physical_device)?;
    let support = SwapchainSupport::get(instance, data, data.physical_device)?;
    let surface_format = get_swapchain_surface_format(&support.formats);
    let present_mode = get_swapchain_present_mode(&support.present_modes);
    let extent = get_swapchain_extent(window, support.capabilities);
    let mut img_cnt = support.capabilities.min_image_count + 1;
    if support.capabilities.max_image_count != 0 && img_cnt > support.capabilities.max_image_count {
        img_cnt = support.capabilities.max_image_count;
    }
    let mut queue_family_indices = vec![];
    let image_sharing_mode = if indices.graphics != indices.present {
        queue_family_indices.push(indices.graphics);
        queue_family_indices.push(indices.present);
        vk::SharingMode::CONCURRENT
    } else {
        vk::SharingMode::EXCLUSIVE
    };
    let info = vk::SwapchainCreateInfoKHR::builder()
        .surface(data.surface)
        .min_image_count(img_cnt)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(image_sharing_mode)
        .queue_family_indices(&queue_family_indices)
        .pre_transform(support.capabilities.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode)
        .clipped(true)
        .old_swapchain(vk::SwapchainKHR::null());
    data.swapchain = device.create_swapchain_khr(&info, None)?;
    data.swapchain_images = device.get_swapchain_images_khr(data.swapchain)?;
    data.swapchain_format = surface_format.format;
    data.swapchain_extent = extent;
    Ok(())
}
