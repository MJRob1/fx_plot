use log::error;
use std::process::exit;

fn main() {
    // start log4rs logging framework
    if let Err(e) = log4rs::init_file("logging_config.yaml", Default::default()) {
        eprintln!("error initialising log4rs - {e}");
        exit(1);
    }

    let win_option = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "Plot demo",
        win_option,
        Box::new(|cc| Ok(Box::new(fx_plot::FxViewerApp::init(cc)))),
    ) {
        error!("error starting eframe - {e}");
        exit(1);
    }
}
