use crate::server::Notification;
use glium::glutin;
use glium::glutin::event::{Event, WindowEvent};
use glium::glutin::event_loop::{ControlFlow, EventLoop};
use glium::glutin::platform::unix::{WindowBuilderExtUnix, XWindowType};
use glium::glutin::window::WindowBuilder;
use glium::{Display, Surface};
use imgui::Context;
use imgui::*;
use imgui_glium_renderer::Renderer;
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::time::Instant;

pub struct Config {
    pub width: f32,
    pub height: f32,
}

pub struct NotificationWindow {
    event_loop: EventLoop<()>,
    display: Display,
    imgui: Context,
    platform: WinitPlatform,
    renderer: Renderer,
    config: Config,
}

impl NotificationWindow {
    pub fn new(config: Config) -> Self {
        let event_loop = EventLoop::new();
        let context = glutin::ContextBuilder::new().with_vsync(true);
        let builder = WindowBuilder::new()
            .with_x11_window_type(vec![XWindowType::Notification, XWindowType::Utility])
            .with_override_redirect(true)
            .with_resizable(false)
            .with_transparent(true)
            .with_always_on_top(true)
            .with_decorations(false);
        let display =
            Display::new(builder, context, &event_loop).expect("Failed to initialize display");

        {
            let gl_window = display.gl_window();
            let window = gl_window.window();
            let screen_size = window.current_monitor().size();
            use glutin::dpi::{PhysicalPosition, PhysicalSize};
            window.set_inner_size(PhysicalSize::new(config.width, config.height));
            window.set_outer_position(PhysicalPosition::new(
                screen_size.width - config.width as u32,
                0,
            ));
        }

        let mut imgui = Context::create();
        imgui.set_ini_filename(None);

        let mut platform = WinitPlatform::init(&mut imgui);
        {
            let gl_window = display.gl_window();
            let window = gl_window.window();
            // xxx: figure out why the fuck this is weird on s-s
            platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Locked(1.0));
        }

        let renderer = Renderer::init(&mut imgui, &display).expect("Failed to initialize renderer");

        NotificationWindow {
            event_loop,
            display,
            imgui,
            platform,
            renderer,
            config,
        }
    }

    pub fn show(self, notification: Notification) {
        let mut last_frame = Instant::now();
        let NotificationWindow {
            event_loop,
            mut imgui,
            mut platform,
            display,
            mut renderer,
            config,
        } = self;

        event_loop.run(move |event, _, control_flow| match event {
            Event::NewEvents(_) => last_frame = imgui.io_mut().update_delta_time(last_frame),
            Event::MainEventsCleared => {
                let gl_window = display.gl_window();
                platform
                    .prepare_frame(imgui.io_mut(), &gl_window.window())
                    .expect("Failed to prepare frame");
                gl_window.window().request_redraw();
            }
            Event::RedrawRequested(_) => {
                let ui = imgui.frame();
                NotificationWindow::render(&notification, &ui, &config);

                let gl_window = display.gl_window();
                let mut target = display.draw();
                target.clear_color_srgb(0.0, 0.0, 0.0, 0.0);
                platform.prepare_render(&ui, gl_window.window());

                let draw_data = ui.render();
                renderer
                    .render(&mut target, draw_data)
                    .expect("Rendering failed");
                target.finish().expect("Failed to swap buffers");
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            event => {
                let gl_window = display.gl_window();
                platform.handle_event(imgui.io_mut(), gl_window.window(), &event);
            }
        })
    }

    fn render(notification: &Notification, ui: &imgui::Ui, config: &Config) {
        Window::new(im_str!("Hello world"))
            .position([0.0, 0.0], Condition::Always)
            .size([config.width, config.height], Condition::Always)
            .no_decoration()
            .no_inputs()
            .no_nav()
            .focus_on_appearing(false)
            .build(&ui, || {
                ui.text(&notification.application_name);
                ui.text(&notification.summary);
                if let Some(body) = &notification.body {
                    ui.text(body);
                }
            });
    }
}
