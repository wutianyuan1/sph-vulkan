use std::hash::{Hash, Hasher};
use std::collections::HashMap;
use std::io::BufReader;
use std::fs::File;
use std::mem::size_of;
use vulkanalia::prelude::v1_0::*;
use nalgebra_glm as glm;
use anyhow::Result;

use crate::appdata::AppData;

#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub struct Vertex {
    pub pos: glm::Vec3,
    pub color: glm::Vec3,
}

impl PartialEq for Vertex {
    fn eq(&self, other: &Self) -> bool {
        self.pos == other.pos && self.color == other.color
    }
}

impl Eq for Vertex {}

impl Hash for Vertex {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pos[0].to_bits().hash(state);
        self.pos[1].to_bits().hash(state);
        self.pos[2].to_bits().hash(state);
        self.color[0].to_bits().hash(state);
        self.color[1].to_bits().hash(state);
        self.color[2].to_bits().hash(state);
    }
}

impl Vertex {
    pub fn new(pos: glm::Vec3, color: glm::Vec3) -> Self { 
        Self { pos, color } 
    }

    pub fn binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
            .build()
    }

    pub fn attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
        let pos = vk::VertexInputAttributeDescription::builder()
            .binding(0).location(0).format(vk::Format::R32G32B32_SFLOAT).offset(0).build();
        let color = vk::VertexInputAttributeDescription::builder()
            .binding(0).location(1).format(vk::Format::R32G32B32_SFLOAT).offset(size_of::<glm::Vec3>() as u32).build();
        [pos, color]
    }
}

pub fn load_model(model_path: String, data: &mut AppData) -> Result<()> {
    let mut reader = BufReader::new(File::open(model_path)?);
    let mut unique_vertices = HashMap::new();
    let (models, _) = tobj::load_obj_buf(
        &mut reader,
        &tobj::LoadOptions { triangulate: true, ..Default::default() },
        |_| Ok(Default::default()),
    )?;
    for model in &models {
        for index in &model.mesh.indices {
            let pos_offset = (3 * index) as usize;
            let tex_coord_offset = (2 * index) as usize;
            let vertex = Vertex {
                pos: glm::vec3(model.mesh.positions[pos_offset],
                model.mesh.positions[pos_offset + 1],
                model.mesh.positions[pos_offset + 2]),
                color: glm::vec3(*index as f32 / model.mesh.indices.len() as f32, 
                    *index as f32 / model.mesh.indices.len() as f32, 
                    *index as f32 / model.mesh.indices.len() as f32),
            };
            if let Some(index) = unique_vertices.get(&vertex) {
                data.indices.push(*index as u32);
            } else {
                let index = data.vertices.len();
                unique_vertices.insert(vertex, index);
                data.vertices.push(vertex);
                data.indices.push(index as u32);
            }
        }
    }
    Ok(())
}
