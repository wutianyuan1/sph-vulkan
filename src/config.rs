use vulkanalia::{prelude::v1_0::*};

/// Whether the validation layers should be enabled.
pub const VALIDATION_ENABLED: bool = cfg!(debug_assertions);

/// The name of the validation layers & extensions.
pub const VALIDATION_LAYER: vk::ExtensionName = vk::ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");

#[cfg(target_os = "macos")]
pub const DEVICE_EXTENSIONS: &[vk::ExtensionName] = &[
    vk::KHR_SWAPCHAIN_EXTENSION.name, 
    vk::KHR_PORTABILITY_SUBSET_EXTENSION.name,  // this is a provisional feature...
];

#[cfg(not(target_os = "macos"))]
pub const DEVICE_EXTENSIONS: &[vk::ExtensionName] = &[vk::KHR_SWAPCHAIN_EXTENSION.name, ];

/// Max frames in flight to be presented.
pub const MAX_FRAMES_IN_FLIGHT: usize = 2;
