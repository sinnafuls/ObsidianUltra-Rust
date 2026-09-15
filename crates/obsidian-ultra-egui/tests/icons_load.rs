use egui::load::{SizeHint, TexturePoll};
use obsidian_ultra_egui::LucideIcons;

#[test]
fn lucide_icons_rasterize() {
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    ctx.run_ui(egui::RawInput::default(), |_ui| {}).textures_delta.clear();
    for name in ["bell", "minus", "chevron-up", "gem", "search", "settings", "house"] {
        let svg = LucideIcons::svg(name).expect(name);
        let uri = format!("bytes://obsidian/lucide/{name}.svg");
        ctx.include_bytes(uri.clone(), svg.into_bytes());
        let mut result = None;
        for _ in 0..20 {
            ctx.run_ui(egui::RawInput::default(), |_ui| {}).textures_delta.clear();
            match ctx.try_load_texture(&uri, egui::TextureOptions::LINEAR, SizeHint::Size { width: 16, height: 16, maintain_aspect_ratio: true }) {
                Ok(TexturePoll::Ready { texture }) => {
                    result = Some(Ok(texture.size));
                    break;
                }
                Ok(TexturePoll::Pending { .. }) => continue,
                Err(e) => {
                    result = Some(Err(format!("{e:?}")));
                    break;
                }
            }
        }
        eprintln!("{name}: {result:?}");
        assert!(matches!(result, Some(Ok(_))), "{name} failed: {result:?}");
    }
}
