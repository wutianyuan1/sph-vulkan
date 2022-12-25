#![allow(dead_code, unused_variables, clippy::too_many_arguments, clippy::unnecessary_wraps)]
pub mod callback;
pub mod app;
pub mod appdata;
pub mod config;
pub mod utils;
pub mod camera;
pub mod model;

use anyhow::Result;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use vulkanalia::prelude::v1_0::*;

use crate::app::App;

#[rustfmt::skip]
fn main() -> Result<()> {
    pretty_env_logger::init();

    // Window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Your mother has been slain!")
        .with_inner_size(LogicalSize::new(1024, 768))
        .build(&event_loop)?;

    // App
    let mut app = unsafe { App::create(&window, "viking_room.obj".to_string(),
        "shaders/shader.vert".to_string(), "shaders/shader.frag".to_string())? };
    let mut destroying = false;
    let mut minimized = false;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            // Render a frame if the Vulkan app is not being destroyed.
            Event::MainEventsCleared if !destroying && !minimized => {
                unsafe { app.render(&window) }.unwrap()
            }
            // Destroy the Vulkan app.
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                destroying = true;
                *control_flow = ControlFlow::Exit;
                unsafe { app.device().device_wait_idle().unwrap(); }
                unsafe { app.destroy(); }
            }
            // Resize the window
            Event::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                if size.width == 0 || size.height == 0 {
                    minimized = true;
                } else {
                    minimized = false;
                    app.resized(true);
                }
            }
    
            _ => {}
        }
    });
}
