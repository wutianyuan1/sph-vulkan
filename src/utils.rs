use anyhow::{anyhow, Result};
use log::*;
use shaderc::CompilationArtifact;
use std::fs::File;
use std::path::Path;
use std::collections::HashSet;
use std::io::Read;
use vulkanalia::vk::{KhrSurfaceExtension, KhrSwapchainExtension, ShaderModule};
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

pub unsafe fn create_swapchain_image_views(device: &Device, data: &mut AppData) -> Result<()> {
    data.swapchain_image_views = data.swapchain_images.iter()
        .map(|i| {
            let components = vk::ComponentMapping::builder()
                .r(vk::ComponentSwizzle::IDENTITY)
                .g(vk::ComponentSwizzle::IDENTITY)
                .b(vk::ComponentSwizzle::IDENTITY)
                .a(vk::ComponentSwizzle::IDENTITY);
            let subresource_range = vk::ImageSubresourceRange::builder()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1);
            let info = vk::ImageViewCreateInfo::builder()
                .image(*i)
                .view_type(vk::ImageViewType::_2D)
                .format(data.swapchain_format)
                .components(components)
                .subresource_range(subresource_range);
            device.create_image_view(&info, None)
        })
        .collect::<Result<Vec<_>, _> >()?;
    Ok(())
}

/// Pipeline helpers
pub unsafe fn create_pipeline(device: &Device, data: &mut AppData) -> Result<()> {
    let vshader = compile_shader(&data.vshader_path, shaderc::ShaderKind::Vertex)?;
    let fshader = compile_shader(&data.fshader_path, shaderc::ShaderKind::Fragment)?;
    let vert_shader_module = create_shader_module(device, &vshader.as_binary_u8()[..])?;
    let frag_shader_module = create_shader_module(device, &fshader.as_binary_u8()[..])?;
    let vert_stage = vk::PipelineShaderStageCreateInfo::builder()
        .stage(vk::ShaderStageFlags::VERTEX)
        .module(vert_shader_module)
        .name(b"main\0");
    let frag_stage = vk::PipelineShaderStageCreateInfo::builder()
        .stage(vk::ShaderStageFlags::FRAGMENT)
        .module(frag_shader_module)
        .name(b"main\0");
    let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder();
    let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .primitive_restart_enable(false);
    let viewport = vk::Viewport::builder().x(0.0).y(0.0)
        .width(data.swapchain_extent.width as f32).height(data.swapchain_extent.height as f32)
        .min_depth(0.0).max_depth(1.0);
    let scissor = vk::Rect2D::builder().offset(vk::Offset2D{x: 0, y: 0}).extent(data.swapchain_extent);
    let (viewports,  scissors) = (&[viewport], &[scissor]);
    let viewport_state = vk::PipelineViewportStateCreateInfo::builder().viewports(viewports).scissors(scissors);
    let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::BACK)
        .front_face(vk::FrontFace::CLOCKWISE)
        .depth_bias_enable(false);
    let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
        .sample_shading_enable(false)
        .rasterization_samples(vk::SampleCountFlags::_1);
    let attachment = vk::PipelineColorBlendAttachmentState::builder()
        .color_write_mask(vk::ColorComponentFlags::all())
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
        .alpha_blend_op(vk::BlendOp::ADD);
    let attachments = &[attachment];
    let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
        .logic_op_enable(false)
        .logic_op(vk::LogicOp::COPY)
        .attachments(attachments)
        .blend_constants([0.0, 0.0, 0.0, 0.0]);
    let dynamic_states = &[vk::DynamicState::VIEWPORT, vk::DynamicState::LINE_WIDTH];
    let dynamic_state = vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(dynamic_states);

    let layout_info = vk::PipelineLayoutCreateInfo::builder();
    data.pipeline_layout = device.create_pipeline_layout(&layout_info, None)?;

    let stages = &[vert_stage, frag_stage];
    let info = vk::GraphicsPipelineCreateInfo::builder()
        .stages(stages)
        .vertex_input_state(&vertex_input_state)
        .input_assembly_state(&input_assembly_state)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterization_state)
        .multisample_state(&multisample_state)
        .color_blend_state(&color_blend_state)
        .layout(data.pipeline_layout)
        .render_pass(data.render_pass)
    .subpass(0);
    data.pipeline = device.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)?.0;
    
    device.destroy_shader_module(vert_shader_module, None);
    device.destroy_shader_module(frag_shader_module, None);
    Ok(())
}

fn compile_shader(shader_path: &String, shader_kind: shaderc::ShaderKind) -> Result<CompilationArtifact>{
    let mut shader_file = File::open(Path::new(shader_path))?;
    let mut shader_buffer = String::new();
    shader_file.read_to_string(&mut shader_buffer)?;
    let compiler = shaderc::Compiler::new().unwrap();
    let options = shaderc::CompileOptions::new().unwrap();
    let binary_result = compiler.compile_into_spirv(&shader_buffer, shader_kind, &shader_path, "main", Some(&options));
    let binary_result = match binary_result {
        Ok(binary_result) => binary_result,
        Err(e) => {
            let error_lines = e.to_string().split('\n').for_each(|line| println!("{}", line));
            panic!("Compilation Error!");
        }
    };
    Ok(binary_result)
}


unsafe fn create_shader_module(device: &Device, bytecode: &[u8],) -> Result<ShaderModule> {
    let bytecode = Vec::from(bytecode);
    let (prefix, code, suffix) = bytecode.align_to::<u32>();
    if !prefix.is_empty() || !suffix.is_empty() {
        return Err(anyhow!("Shader bytecode is not properly aligned."));
    }
    let info = vk::ShaderModuleCreateInfo::builder().code_size(bytecode.len()).code(code);
    Ok(device.create_shader_module(&info, None)?)
}

pub unsafe fn create_render_pass(instance: &Instance, device: &Device, data: &mut AppData) -> Result<()> {
    let color_attachment = vk::AttachmentDescription::builder()
        .format(data.swapchain_format).samples(vk::SampleCountFlags::_1)
        .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE).stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED).final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

    // Subpasses
    let color_attachment_ref = vk::AttachmentReference::builder().attachment(0).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

    let color_attachments = &[color_attachment_ref];
    let subpass = vk::SubpassDescription::builder()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(color_attachments);
    let dependency = vk::SubpassDependency::builder()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);

    // Create
    let attachments = &[color_attachment];
    let subpasses = &[subpass];
    let dependencies = &[dependency];
    let info = vk::RenderPassCreateInfo::builder()
        .attachments(attachments)
        .subpasses(subpasses)
        .dependencies(dependencies);
    data.render_pass = device.create_render_pass(&info, None)?;


    Ok(())
}

/// Frambebuffer helpers
pub unsafe fn create_framebuffers(device: &Device, data: &mut AppData) -> Result<()> {
    data.framebuffers = data.swapchain_image_views.iter()
        .map(|i| {
            let attachments = &[*i];
            let create_info = vk::FramebufferCreateInfo::builder()
                .render_pass(data.render_pass)
                .attachments(attachments)
                .width(data.swapchain_extent.width)
                .height(data.swapchain_extent.height)
                .layers(1);
            device.create_framebuffer(&create_info, None)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(())
}

/// Commandbuffer helpers
pub unsafe fn create_command_pool(instance: &Instance, device: &Device, data: &mut AppData) -> Result<()> {
    let indices = QueueFamilyIndices::get(instance, data, data.physical_device)?;
    let info = vk::CommandPoolCreateInfo::builder()
        .flags(vk::CommandPoolCreateFlags::empty())
        .queue_family_index(indices.graphics);
        data.command_pool = device.create_command_pool(&info, None)?;
    Ok(())
}

pub unsafe fn create_command_buffers(device: &Device, data: &mut AppData) -> Result<()> {
    let allocate_info = vk::CommandBufferAllocateInfo::builder()
        .command_pool(data.command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(data.framebuffers.len() as u32);
    data.command_buffers = device.allocate_command_buffers(&allocate_info)?;

    for (i, command_buffer) in data.command_buffers.iter().enumerate() {
        let inheritance = vk::CommandBufferInheritanceInfo::builder();
    
        let inherit_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::empty()) // Optional.
            .inheritance_info(&inheritance);             // Optional.
    
        let render_area = vk::Rect2D::builder().offset(vk::Offset2D::default()).extent(data.swapchain_extent);
        let color_clear_value = vk::ClearValue {
            color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] },
        };
        let clear_values = &[color_clear_value];
        let render_info = vk::RenderPassBeginInfo::builder()
            .render_pass(data.render_pass)
            .framebuffer(data.framebuffers[i])
            .render_area(render_area)
            .clear_values(clear_values);
        
        // render pass!!
        device.begin_command_buffer(*command_buffer, &inherit_info)?;
        device.cmd_begin_render_pass(*command_buffer, &render_info, vk::SubpassContents::INLINE);
        device.cmd_bind_pipeline(*command_buffer, vk::PipelineBindPoint::GRAPHICS, data.pipeline);
        device.cmd_draw(*command_buffer, 3, 1, 0, 0);
        device.cmd_end_render_pass(*command_buffer);  // device.cmd_begin_render_pass
        device.end_command_buffer(*command_buffer)?;  // device.begin_command_buffer
    }
    Ok(())
}

pub unsafe fn create_sync_objects(device: &Device, data: &mut AppData) -> Result<()> {
    let semaphore_info = vk::SemaphoreCreateInfo::builder();
    let fence_info = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);
    for _ in 0..MAX_FRAMES_IN_FLIGHT {
        data.image_available_semaphores
            .push(device.create_semaphore(&semaphore_info, None)?);
        data.render_finished_semaphores
            .push(device.create_semaphore(&semaphore_info, None)?);
        data.in_flight_fences.push(device.create_fence(&fence_info, None)?);
    }
    data.images_in_flight = data.swapchain_images.iter().map(|_| vk::Fence::null()).collect();
    Ok(())
}
