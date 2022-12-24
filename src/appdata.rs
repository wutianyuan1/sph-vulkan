use vulkanalia::prelude::v1_0::*;

/// The Vulkan handles and associated properties used by our Vulkan app.
#[derive(Clone, Debug, Default)]
pub struct AppData {
    // Debug
    pub messenger: vk::DebugUtilsMessengerEXT,
    pub physical_device: vk::PhysicalDevice,
    pub graphics_queue: vk::Queue,
    pub surface: vk::SurfaceKHR,
    pub present_queue: vk::Queue,
    pub swapchain_format: vk::Format,
    pub swapchain_extent: vk::Extent2D,
    pub swapchain: vk::SwapchainKHR,
    pub swapchain_images: Vec<vk::Image>,
}