//! Obsidian Ultra showcase: exercises every public API. Flags:
//!   --screenshot <path.png>   capture after 1.5 s and exit
//!   --dpi <percent>           initial DPI scale (default 100)
//!   --no-host                 hide the host egui strip (clean screenshots)

use std::path::PathBuf;

use eframe::egui;
use obsidian_ultra_core::*;
use obsidian_ultra_egui::Obsidian;

mod demo;

struct App {
    obsidian: Obsidian,
    screenshot: Option<PathBuf>,
    host: bool,
    frames: u32,
    side_clicks: u32,
    started: std::time::Instant,
    shot_requested: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>, dpi: f32, host: bool) -> Self {
        let ui = Ui::new();
        ui.set_dpi_scale(dpi);
        demo::build(&ui);
        let mut obsidian = Obsidian::new(ui);
        let _ = &cc.egui_ctx;
        obsidian.screen = None;
        Self { obsidian, screenshot, host, frames: 0, side_clicks: 0, started: std::time::Instant::now(), shot_requested: false }
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frames += 1;
        let ctx = root.ctx().clone();
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(28, 30, 36))).show(root, |ui| {
            if self.host {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Host egui panel: this button stays clickable unless the Obsidian window covers it").color(egui::Color32::GRAY));
                    if ui.button(format!("Host button ({})", self.side_clicks)).clicked() {
                        self.side_clicks += 1;
                    }
                });
            }
        });
        let ctx = &ctx;
        self.obsidian.show(ctx);
        if let Some(path) = &self.screenshot {
            if self.frames >= 10 && !self.shot_requested && self.started.elapsed().as_secs_f32() >= 1.5 {
                self.shot_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            let shot = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(img) = shot {
                let (w, h) = (img.size[0] as u32, img.size[1] as u32);
                let mut buf = image::RgbaImage::new(w, h);
                for (i, p) in img.pixels.iter().enumerate() {
                    let x = (i as u32) % w;
                    let y = (i as u32) / w;
                    buf.put_pixel(x, y, image::Rgba([p.r(), p.g(), p.b(), 255]));
                }
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                buf.save(path).expect("write screenshot");
                eprintln!("screenshot written to {}", path.display());
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot = None;
    let mut dpi = 100.0;
    let mut host = true;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" => {
                screenshot = args.get(i + 1).map(PathBuf::from);
                i += 1;
            }
            "--no-host" => host = false,
            "--dpi" => {
                dpi = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(100.0);
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]).with_title("Obsidian Ultra showcase"),
        ..Default::default()
    };
    eframe::run_native("obsidian-showcase", options, Box::new(move |cc| Ok(Box::new(App::new(cc, screenshot, dpi, host)))))
}
