use std::mem::size_of;
use std::ptr::copy_nonoverlapping as memcpy;
use vulkanalia::vk;
use nalgebra_glm as glm;
use vulkanalia::prelude::v1_0::*;

use anyhow::Result;
use crate::appdata::AppData;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Camera {
    pub model: glm::Mat4,
    pub view: glm::Mat4,
    pub proj: glm::Mat4,
    pub base_light: glm::Vec3,
	pub ambient_strength: f32,
    pub light_pos: glm::Vec3,
    pub specular_strength: f32,
    pub view_pos: glm::Vec3,
    
}

impl Camera {
    pub fn new() -> Self {
        Self { model: glm::identity(), view: glm::identity(), proj: glm::identity(), 
            base_light: glm::Vec3::default(), ambient_strength: 0.1, 
            light_pos: glm::vec3(1.0, 1.0, 1.0), view_pos: glm::vec3(1.0, 1.0, 1.0),
            specular_strength: 0.8,
        }
    }

    pub unsafe fn update_viewport(&mut self, image_index: usize, alpha: f32,
        data: &AppData, device: &Device) 
    -> Result<()> {
        self.view_pos = glm::vec3(1.0, 1.0, 1.0);
        self.model = glm::rotate(
            &glm::identity(),
            alpha * glm::radians(&glm::vec1(90.0))[0],
            &glm::vec3(0.0, 0.0, 1.0),
        );
        self.view = glm::look_at(
            &self.view_pos,
            &glm::vec3(0.0, 0.0, 0.0),
            &glm::vec3(0.0, 0.0, 1.0),
        );
        self.proj = glm::perspective_rh_zo(
            data.swapchain_extent.width as f32 / data.swapchain_extent.height as f32,
            glm::radians(&glm::vec1(45.0))[0],
            0.1,
            10.0,
        );
        self.proj[(1, 1)] *= -1.0;
        self.base_light = glm::vec3(1.0, 1.0, 1.0);

        let memory = device.map_memory(
            data.uniform_buffers_memory[image_index], 0,
            size_of::<Camera>() as u64, vk::MemoryMapFlags::empty())?;
        
        memcpy(self, memory.cast(), 1);
        device.unmap_memory(data.uniform_buffers_memory[image_index]);
        Ok(())
    }
    
}
