mod history;
mod i18n;
mod server;
mod steam;
mod ui;
mod updater;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_app_id("steamrollbackr-lite")
        .with_inner_size([880.0, 620.0])
        .with_min_inner_size([720.0, 520.0]);

    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../packaging/steamrollbackr-lite.png")) {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "SteamRollbackrLite",
        options,
        Box::new(|cc| Ok(Box::new(ui::SteamRollbackrLiteApp::new(cc)))),
    )
}

