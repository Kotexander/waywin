use waywin::Window;

pub struct ColorClearer<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    t: f64,
    timer: std::time::Instant,
}
impl<'a> ColorClearer<'a> {
    pub async fn new(window: &'a Window) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::default(),
            flags: wgpu::InstanceFlags::default(),
            dx12_shader_compiler: wgpu::Dx12Compiler::default(),
            gles_minor_version: wgpu::Gles3MinorVersion::default(),
        });
        let surface = instance
            .create_surface(window)
            .or_else(|err| Err(format!("{err}")))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .ok_or_else(|| String::from("Failed to request adapter"))?;

        let size = window.size();
        let capabilities = surface.get_capabilities(&adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: capabilities.formats[0],
            width: size.0,
            height: size.1,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .or_else(|err| Err(format!("{err}")))?;

        surface.configure(&device, &config);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            t: 0.0,
            timer: std::time::Instant::now(),
        })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        let w = w.max(1);
        let h = h.max(1);
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn clear(&mut self) {
        if let Ok(output) = self.surface.get_current_texture() {
            let view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

            {
                let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: (self.t.sin() + 1.0) / 2.0,
                                g: ((self.t + std::f64::consts::TAU / 3.0).sin() + 1.0) / 2.0,
                                b: ((self.t + 2.0 * std::f64::consts::TAU / 3.0).sin() + 1.0) / 2.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
            }

            self.queue.submit(std::iter::once(encoder.finish()));
            output.present();
        }

        let dt = self.timer.elapsed();
        self.timer = std::time::Instant::now();
        self.t += dt.as_secs_f64();
        // self.t = std::f64::consts::FRAC_PI_2;
    }
}
