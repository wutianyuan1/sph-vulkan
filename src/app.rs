use std::collections::HashSet;
use std::time::Instant;
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
use crate::camera::{UniformBufferObject, Camera};
use crate::model::Object;

/// The application.
#[derive(Clone, Debug)]
pub struct App{
    entry: Entry,
    instance: Instance,
    data: AppData,
    device: Device,
    frame: usize,
    resized: bool,
    ubo: UniformBufferObject,
    camera: Camera,
    timer: Instant,
}

impl App {
    /// Creates the app instance.
    pub unsafe fn create(window: &Window, model_paths: Vec<String>,
            vshader_path: String, fshader_path: String) -> Result<Self> {
        // loader and entry 
        let loader = LibloadingLoader::new(LIBRARY)?;
        let entry = Entry::new(loader).map_err(|b| anyhow!("{}", b))?;
        // data
        let mut data = AppData::default();
        data.vshader_path = vshader_path;
        data.fshader_path = fshader_path;
        // instance
        let instance = create_instance(window, &entry, &mut data)?;
        // window surface
        data.surface = vk_window::create_surface(&instance, window)?;
        // device
        pick_physical_device(&instance, &mut data)?;
        let device = create_logical_device(&instance, &mut data)?;
        // other setups
        create_swapchain(window, &instance, &device, &mut data)?;
        create_swapchain_image_views(&device, &mut data)?;
        create_render_pass(&instance, &device, &mut data)?;
        create_descriptor_set_layout(&device, &mut data)?;
        create_pipeline(&device, &mut data)?;
        create_command_pool(&instance, &device, &mut data)?;
        create_depth_objects(&instance, &device, &mut data)?;
        create_framebuffers(&device, &mut data)?;
        // load models for each object to render
        for model_path in model_paths {
            let obj = Object::new(model_path, &instance, &device, &mut data)?;
            data.objects.push(obj);
        }
        // uniform and command buffers
        create_uniform_buffers(&instance, &device, &mut data)?;
        create_descriptor_pool(&device, &mut data)?;
        create_descriptor_sets(&device, &mut data)?;
        create_command_buffers(&device, &mut data)?;
        create_sync_objects(&device, &mut data)?;
        Ok(Self { entry, instance, data, device, frame: 0, resized: false, ubo: UniformBufferObject::new(),
            timer: Instant::now(), camera: Camera::new(0.1, 0.2)? })
    }

    /// Renders a frame for the app.
    pub unsafe fn render(&mut self, window: &Window) -> Result<()> {
        let t1 = self.timer.elapsed().as_secs_f32();
        // wait and reset fences for GPU-CPU sync
        let in_flight_fence = self.data.in_flight_fences[self.frame];
        let result = self.device.acquire_next_image_khr(
            self.data.swapchain, u64::MAX, self.data.image_available_semaphores[self.frame], vk::Fence::null());
        let image_index = match result {
            Ok((image_index, _)) => image_index as usize,
            Err(vk::ErrorCode::OUT_OF_DATE_KHR) => return self.recreate_swapchain(window),
            Err(e) => return Err(anyhow!(e)),
        };
        self.device.wait_for_fences(&[in_flight_fence], true, u64::max_value())?;
        // wait for image fence
        let image_in_flight = self.data.images_in_flight[image_index];
        if !image_in_flight.is_null() {
            self.device.wait_for_fences(&[image_in_flight], true, u64::max_value())?;
        }
        self.data.images_in_flight[image_index] = in_flight_fence;
        self.ubo.update(image_index, self.camera.get_view_matrix(), &self.data, &self.device)?;
    
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
        let result = self.device.queue_present_khr(self.data.present_queue, &present_info);
        let changed = result == Ok(vk::SuccessCode::SUBOPTIMAL_KHR) || result == Err(vk::ErrorCode::OUT_OF_DATE_KHR);
        if changed {
            self.recreate_swapchain(window)?;
        } else if let Err(e) = result {
            return Err(anyhow!(e));
        }
        self.frame = (self.frame + 1) % MAX_FRAMES_IN_FLIGHT;
        Ok(())
    }

    /// Destroys the app.
    #[rustfmt::skip]
    pub unsafe fn destroy(&mut self) {
        self.destroy_swapchain();
        self.device.destroy_descriptor_set_layout(self.data.descriptor_set_layout, None);
        self.data.objects.iter().for_each(|obj| {
            self.device.destroy_buffer(obj.index_buffer, None);
            self.device.free_memory(obj.index_buffer_memory, None);
            self.device.destroy_buffer(obj.vertex_buffer, None);
            self.device.free_memory(obj.vertex_buffer_memory, None);
        });
        self.data.in_flight_fences.iter().for_each(|f| self.device.destroy_fence(*f, None));
        self.data.render_finished_semaphores.iter().for_each(|s| self.device.destroy_semaphore(*s, None));
        self.data.image_available_semaphores.iter().for_each(|s| self.device.destroy_semaphore(*s, None));
        self.device.destroy_command_pool(self.data.command_pool, None);
        self.device.destroy_device(None);
        self.instance.destroy_surface_khr(self.data.surface, None);
        if VALIDATION_ENABLED {
            self.instance.destroy_debug_utils_messenger_ext(self.data.messenger, None);
        }
        self.instance.destroy_instance(None);
    }

    unsafe fn destroy_swapchain(&mut self) {
        self.device.destroy_image_view(self.data.depth_image_view, None);
        self.device.free_memory(self.data.depth_image_memory, None);
        self.device.destroy_image(self.data.depth_image, None);
        self.device.destroy_descriptor_pool(self.data.descriptor_pool, None);
        self.data.uniform_buffers.iter().for_each(|b| self.device.destroy_buffer(*b, None));
        self.data.uniform_buffers_memory.iter().for_each(|m| self.device.free_memory(*m, None));
        self.data.framebuffers.iter().for_each(|f| self.device.destroy_framebuffer(*f, None));
        self.device.free_command_buffers(self.data.command_pool, &self.data.command_buffers);
        self.device.destroy_pipeline(self.data.pipeline, None);
        self.device.destroy_pipeline_layout(self.data.pipeline_layout, None);
        self.device.destroy_render_pass(self.data.render_pass, None);
        self.data.swapchain_image_views.iter().for_each(|v| self.device.destroy_image_view(*v, None));
        self.device.destroy_swapchain_khr(self.data.swapchain, None);
    }

    pub unsafe fn recreate_swapchain(&mut self, window: &Window) -> Result<()> {
        self.device.device_wait_idle()?;
        self.destroy_swapchain();
        create_swapchain(window, &self.instance, &self.device, &mut self.data)?;
        create_swapchain_image_views(&self.device, &mut self.data)?;
        create_render_pass(&self.instance, &self.device, &mut self.data)?;
        create_pipeline(&self.device, &mut self.data)?;
        create_depth_objects(&self.instance, &self.device, &mut self.data)?;
        create_framebuffers(&self.device, &mut self.data)?;
        create_uniform_buffers(&self.instance, &self.device, &mut self.data)?;
        create_descriptor_pool(&self.device, &mut self.data)?;
        create_descriptor_sets(&self.device, &mut self.data)?;
        create_command_buffers(&self.device, &mut self.data)?;
        self.data.images_in_flight.resize(self.data.swapchain_images.len(), vk::Fence::null());
        Ok(())
    }

    /// accessors & modifiers
    pub fn device(&mut self) -> &Device {
        &self.device
    }

    pub fn data(&mut self) -> &AppData {
        &self.data
    }

    pub fn resized(&mut self, newval: bool) {
        self.resized = newval;
    }

    pub fn handle_mouse(&mut self, x_diff: f32, y_diff: f32) -> Result<()> {
        self.camera.handle_mouse(x_diff, y_diff)?;
        Ok(())
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

    // if cfg!(target_os = "macos") {
    let names = &[ExtensionName::from_bytes(b"VK_KHR_portability_enumeration"),
        ExtensionName::from_bytes(b"VK_KHR_get_physical_device_properties2")];
    names.iter().for_each(|name| extensions.push(name.as_ptr()));
    // }
   
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


