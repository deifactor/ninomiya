use crate::server::Notification;
use glium::glutin;
use glium::glutin::event::{Event, WindowEvent};
use glium::glutin::event_loop::{ControlFlow, EventLoop, EventLoopProxy, EventLoopWindowTarget};
use glium::glutin::platform::unix::{WindowBuilderExtUnix, XWindowType};
use glium::glutin::window::{WindowBuilder, WindowId};
use glium::{Display, Surface};
use imgui::Context;
use imgui::*;
use imgui_glium_renderer::Renderer;
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::collections::HashMap;
use std::time::Instant;

/// Configures how the GUI is rendered.
#[derive(Debug)]
pub struct Config {
    /// Width of notification windows.
    pub width: f32,
    /// Height of notification windows.
    pub height: f32,
}

#[derive(Debug)]
pub enum NinomiyaEvent {
    Notification(Notification),
}

pub struct WindowManager {
    config: Config,
    windows: HashMap<WindowId, NotificationWindow>,
}

impl WindowManager {
    /// Must be called from the main thread.
    pub fn new(config: Config) -> Self {
        WindowManager {
            config,
            windows: HashMap::new(),
        }
    }

    fn add_notification(
        &mut self,
        notification: Notification,
        event_loop: &EventLoopWindowTarget<NinomiyaEvent>,
    ) {
        let window = NotificationWindow::new(notification, &event_loop, &self.config);
        self.windows.insert(window.window_id(), window);
    }

    pub fn run(mut self, event_loop: EventLoop<NinomiyaEvent>) {
        event_loop.run(move |event, target, _control_flow| match event {
            Event::NewEvents(_) => self
                .windows
                .values_mut()
                .for_each(|w| w.update_delta_time()),
            Event::MainEventsCleared => self
                .windows
                .values_mut()
                .for_each(|w| w.main_events_cleared()),
            Event::RedrawRequested(id) => self
                .windows
                .get_mut(&id)
                .unwrap()
                .redraw_requested(&self.config),
            Event::WindowEvent { .. } => {}
            Event::UserEvent(ev) => self.handle_user_event(ev, target),
            _ => {}
        })
    }

    fn handle_user_event(
        &mut self,
        event: NinomiyaEvent,
        event_loop: &EventLoopWindowTarget<NinomiyaEvent>,
    ) {
        match event {
            NinomiyaEvent::Notification(notification) => {
                self.add_notification(notification, event_loop)
            }
        }
    }
}

pub struct NotificationWindow {
    display: Display,
    imgui: Context,
    platform: WinitPlatform,
    renderer: Renderer,
    last_frame: Instant,
    notification: Notification,
}

impl NotificationWindow {
    pub fn new(
        notification: Notification,
        event_loop: &EventLoopWindowTarget<NinomiyaEvent>,
        config: &Config,
    ) -> Self {
        let context = glutin::ContextBuilder::new().with_vsync(true);
        let builder = WindowBuilder::new()
            .with_x11_window_type(vec![XWindowType::Notification, XWindowType::Utility])
            .with_override_redirect(true)
            .with_resizable(false)
            .with_transparent(true)
            .with_always_on_top(true)
            .with_decorations(false);
        let gl_window = glutin::ContextBuilder::new()
            .with_vsync(true)
            .build_windowed(builder, event_loop)
            .expect("failed to build inner window");
        let display = Display::from_gl_window(gl_window).expect("Failed to initialize display");

        // TODO: Make this work on HiDPI screens.

        {
            let gl_window = display.gl_window();
            let window = gl_window.window();
            let screen_size = window.current_monitor().size();
            use glutin::dpi::{LogicalPosition, LogicalSize};
            window.set_inner_size(LogicalSize::new(config.width, config.height));
            window.set_outer_position(LogicalPosition::new(
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
            platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);
        }

        let renderer = Renderer::init(&mut imgui, &display).expect("Failed to initialize renderer");

        NotificationWindow {
            display,
            imgui,
            platform,
            renderer,
            notification,
            last_frame: Instant::now(),
        }
    }

    pub fn window_id(&self) -> WindowId {
        self.display.gl_window().window().id()
    }

    pub fn update_delta_time(&mut self) {
        self.last_frame = self.imgui.io_mut().update_delta_time(self.last_frame);
    }

    pub fn main_events_cleared(&mut self) {
        let gl_window = self.display.gl_window();
        self.platform
            .prepare_frame(self.imgui.io_mut(), &gl_window.window())
            .expect("Failed to prepare frame");
        gl_window.window().request_redraw();
    }

    pub fn redraw_requested(&mut self, config: &Config) {
        let ui = self.imgui.frame();
        NotificationWindow::render(&self.notification, &ui, &config);

        let gl_window = self.display.gl_window();
        let mut target = self.display.draw();
        target.clear_color_srgb(0.0, 0.0, 0.0, 0.0);
        self.platform.prepare_render(&ui, gl_window.window());

        let draw_data = ui.render();
        self.renderer
            .render(&mut target, draw_data)
            .expect("Rendering failed");
        target.finish().expect("Failed to swap buffers");
    }

    fn render(notification: &Notification, ui: &imgui::Ui, config: &Config) {
        println!("{:?}", config);
        Window::new(im_str!("Hello world"))
            .position([0.0, 0.0], Condition::Always)
            .size([config.width, config.height], Condition::Always)
            .no_decoration()
            .no_inputs()
            .no_nav()
            .focus_on_appearing(false)
            .build(&ui, || {
                if let Some(name) = &notification.application_name {
                    ui.text(name);
                }
                ui.text(&notification.summary);
                if let Some(body) = &notification.body {
                    ui.text(body);
                }
            });
    }
}
