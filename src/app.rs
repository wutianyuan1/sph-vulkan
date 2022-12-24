use std::collections::HashSet;
use anyhow::{anyhow, Result};
use winit::window::Window;
use vulkanalia::window as vk_window;
use vulkanalia::loader::{LibloadingLoader, LIBRARY};
use vulkanalia::prelude::v1_0::*;
use vulkanalia::vk::{ExtDebugUtilsExtension, KhrSurfaceExtension, InstanceCreateFlags,
    ExtensionName, KhrSwapchainExtension};

use crate::appdata::AppData;
use crate::callback::debug_callback;
use crate::config::{VALIDATION_ENABLED, VALIDATION_LAYER, MAX_FRAMES_IN_FLIGHT};
use crate::utils::*;

/// Our Vulkan app.
#[derive(Clone, Debug)]
pub struct App {
    entry: Entry,
    instance: Instance,
    data: AppData,
    device: Device,
    frame: usize,
}

impl App {
    /// Creates our Vulkan app.
    pub unsafe fn create(window: &Window) -> Result<Self> {
        let loader = LibloadingLoader::new(LIBRARY)?;
        let entry = Entry::new(loader).map_err(|b| anyhow!("{}", b))?;
        let mut data = AppData::default();
        let instance = create_instance(window, &entry, &mut data)?;
        data.surface = vk_window::create_surface(&instance, window)?;
        pick_physical_device(&instance, &mut data)?;
        let device = create_logical_device(&instance, &mut data)?;
        create_swapchain(window, &instance, &device, &mut data)?;
        create_swapchain_image_views(&device, &mut data)?;
        create_render_pass(&instance, &device, &mut data)?;
        create_pipeline(&device, &mut data)?;
        create_framebuffers(&device, &mut data)?;
        create_command_pool(&instance, &device, &mut data)?;
        create_command_buffers(&device, &mut data)?;
        create_sync_objects(&device, &mut data)?;
        Ok(Self { entry: entry, instance: instance, data: data, device: device, frame: 0 })
    }

    /// Renders a frame for our Vulkan app.
    pub unsafe fn render(&mut self, window: &Window) -> Result<()> {
        // wait and reset fences for GPU-CPU sync
        let in_flight_fence = self.data.in_flight_fences[self.frame];
        let image_index = self.device.acquire_next_image_khr(
            self.data.swapchain, u64::MAX, self.data.image_available_semaphores[self.frame], vk::Fence::null())?.0 as usize;
        self.device.wait_for_fences(&[in_flight_fence], true, u64::max_value())?;
        // wait for image fence
        let image_in_flight = self.data.images_in_flight[image_index];
        if !image_in_flight.is_null() {
            self.device.wait_for_fences(&[image_in_flight], true, u64::max_value())?;
        }
        self.data.images_in_flight[image_index] = in_flight_fence;
    
        // get image from swapchain, and get ready to submit it to present queue
        let wait_semaphores = &[self.data.image_available_semaphores[self.frame]];
        let wait_stages = &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = &[self.data.command_buffers[image_index as usize]];
        let signal_semaphores = &[self.data.render_finished_semaphores[self.frame]];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_stages)
            .command_buffers(command_buffers)
            .signal_semaphores(signal_semaphores);
        // submit to the graphics queue
        self.device.reset_fences(&[self.data.in_flight_fences[self.frame]])?;
        self.device.queue_submit(self.data.graphics_queue, &[submit_info], in_flight_fence)?;
        // present to screen
        let swapchains = &[self.data.swapchain];
        let image_indices = &[image_index as u32];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(signal_semaphores)
            .swapchains(swapchains)
            .image_indices(image_indices);
        self.device.queue_present_khr(self.data.present_queue, &present_info)?;
        self.frame = (self.frame + 1) % MAX_FRAMES_IN_FLIGHT;
        Ok(())
    }

    /// Destroys our Vulkan app.
    #[rustfmt::skip]
    pub unsafe fn destroy(&mut self) {
        self.data.in_flight_fences.iter().for_each(|f| self.device.destroy_fence(*f, None));
        self.data.render_finished_semaphores.iter().for_each(|s| self.device.destroy_semaphore(*s, None));
        self.data.image_available_semaphores.iter().for_each(|s| self.device.destroy_semaphore(*s, None));
        self.device.destroy_command_pool(self.data.command_pool, None);
        self.data.framebuffers.iter().for_each(|f| self.device.destroy_framebuffer(*f, None));
        self.device.destroy_pipeline(self.data.pipeline, None);
        self.device.destroy_pipeline_layout(self.data.pipeline_layout, None);
        self.device.destroy_render_pass(self.data.render_pass, None);
        self.data.swapchain_image_views.iter()
            .for_each(|img| {self.device.destroy_image_view(*img, None)});
        self.device.destroy_swapchain_khr(self.data.swapchain, None);
        self.device.destroy_device(None);
        if VALIDATION_ENABLED {
            self.instance.destroy_debug_utils_messenger_ext(self.data.messenger, None);
        }
        self.instance.destroy_surface_khr(self.data.surface, None);
        self.instance.destroy_instance(None);
    }

    /// accessors
    pub fn device(&mut self) -> &Device {
        &self.device
    }
}

unsafe fn create_instance(window: &Window, entry: &Entry, data: &mut AppData) -> Result<Instance> {
    // Application Info

    let application_info = vk::ApplicationInfo::builder()
        .application_name(b"Vulkan Tutorial (Rust)\0")
        .application_version(vk::make_version(1, 0, 0))
        .engine_name(b"No Engine\0")
        .engine_version(vk::make_version(1, 0, 0))
        .api_version(vk::make_version(1, 0, 0));

    // Layers
    let available_layers = entry.enumerate_instance_layer_properties()?.iter().map(|l| l.layer_name).collect::<HashSet<_>>();
    if VALIDATION_ENABLED && !available_layers.contains(&VALIDATION_LAYER) {
        return Err(anyhow!("Validation layer requested but not supported."));
    }

    let layers: Vec<*const i8> = if VALIDATION_ENABLED {
        vec![VALIDATION_LAYER.as_ptr()]
    } else {
        Vec::new()
    };

    // Extensions
    let mut extensions = vk_window::get_required_instance_extensions(window).iter().map(|e| e.as_ptr()).collect::<Vec<_>>();
    extensions.push(vk::KHR_PORTABILITY_ENUMERATION_EXTENSION.name.as_ptr());
    extensions.push(ExtensionName::from_bytes(b"VK_KHR_get_physical_device_properties2").as_ptr());

    if VALIDATION_ENABLED {
        extensions.push(vk::EXT_DEBUG_UTILS_EXTENSION.name.as_ptr());
    }

    // Create
    let mut info = vk::InstanceCreateInfo::builder()
        .flags(InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR)
        .application_info(&application_info)
        .enabled_layer_names(&layers)
        .enabled_extension_names(&extensions);
        

    let mut debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
        .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::all())
        .message_type(vk::DebugUtilsMessageTypeFlagsEXT::all())
        .user_callback(Some(debug_callback));

    if VALIDATION_ENABLED {
        info = info.push_next(&mut debug_info);
    }
    let instance = entry.create_instance(&info, None)?;

    // Messenger
    if VALIDATION_ENABLED {
        data.messenger = instance.create_debug_utils_messenger_ext(&debug_info, None)?;
    }
    Ok(instance)
}


