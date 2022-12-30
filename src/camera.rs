use std::mem::size_of;
use std::ptr::copy_nonoverlapping as memcpy;
use vulkanalia::vk;
use nalgebra_glm as glm;
use vulkanalia::prelude::v1_0::*;

use anyhow::{Result, Ok};
use crate::appdata::AppData;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct UniformBufferObject {
    pub model: glm::Mat4,
    pub view: glm::Mat4,
    pub proj: glm::Mat4,
    pub base_light: glm::Vec3,
	pub ambient_strength: f32,
    pub light_pos: glm::Vec3,
    pub specular_strength: f32,
    pub view_pos: glm::Vec3, 
}


#[derive(Copy, Clone, Debug, Default)]
pub struct Camera {
    dist_from_origin: f32,
    sensitivity: f32,
    zoom_speed: f32,
    yaw: f32,
    pitch: f32,
    facing: glm::Vec3,
}


impl UniformBufferObject {
    pub fn new() -> Self {
        Self { model: glm::identity(), view: glm::identity(), proj: glm::identity(), 
            base_light: glm::Vec3::default(), ambient_strength: 0.1, 
            light_pos: glm::vec3(1.0, 1.0, 1.0), view_pos: glm::vec3(1.0, 1.0, 1.0),
            specular_strength: 0.8,
        }
    }

    pub unsafe fn update(&mut self, image_index: usize, view_mat: glm::Mat4,
        data: &AppData, device: &Device) 
    -> Result<()> {
        self.view_pos = glm::vec3(1.0, 1.0, 1.0);
        self.model = view_mat;
        
        self.model = glm::rotate(
            &glm::identity(),
            glm::radians(&glm::vec1(90.0))[0],
            &glm::vec3(0.0, 0.0, 1.0),
        );
        self.view = view_mat;
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
            size_of::<UniformBufferObject>() as u64, vk::MemoryMapFlags::empty())?;
        
        memcpy(self, memory.cast(), 1);
        device.unmap_memory(data.uniform_buffers_memory[image_index]);
        Ok(())
    } 
}

impl Camera {
    pub fn new(sensitivity: f32, zoom_speed: f32) -> Result<Self> {
        let mut retval = Self{sensitivity, zoom_speed, yaw: 0.0, pitch: -10.0, 
            dist_from_origin: 2.0, facing: glm::vec3(0.0, 0.0, 0.0)};
        retval.handle_mouse(0.0, 0.0)?;
        Ok(retval)
    }

    pub fn handle_scroll(&mut self, diff: f32){
        self.dist_from_origin -= diff * self.zoom_speed;
        if self.dist_from_origin < 0.5 {
            self.dist_from_origin = 0.5;
        }
    }

    pub fn get_view_matrix(&self) -> glm::Mat4 {
        let position = self.facing * self.dist_from_origin;
        glm::look_at(&position, &glm::vec3(0.0, 0.0, 0.0), &glm::vec3(0.0, 1.0, 0.0))
    }

    pub fn handle_mouse(&mut self, x_diff: f32, y_diff: f32) -> Result<()> {
        self.rotate(self.sensitivity * x_diff, self.sensitivity * y_diff)?;
        Ok(())
    }

    fn rotate(&mut self, yaw: f32, pitch: f32) -> Result<()> {
        self.yaw += yaw;
        self.pitch += pitch;

        if self.pitch > 89.0 {
            self.pitch = 89.0;
        }
        else if self.pitch < -89.0 {
            self.pitch = -89.0;
        }
        if self.yaw > 180.0 {
            self.yaw -= 360.0;
        }
        else if self.yaw < -180.0 {
            self.yaw += -360.0;
        }
        let temp_yaw = glm::radians(&glm::vec1(self.yaw))[0];
        let temp_pit = glm::radians(&glm::vec1(self.pitch))[0];
        let front: glm::Vec3 = glm::vec3(
            temp_yaw.cos() * temp_pit.cos(),
            temp_pit.sin(),
            temp_pit.cos() * temp_yaw.sin(),
        );
        self.facing = glm::normalize(&front);
        Ok(())
    }
}
