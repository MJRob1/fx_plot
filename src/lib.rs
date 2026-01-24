mod consumer;

use eframe::egui;
use egui::Context;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use log::error;
use std::process::exit;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use tokio::runtime::Runtime;

pub fn run<F: Future>(future: F) -> F::Output {
    let rt = Runtime::new().unwrap();
    rt.block_on(future)
}

#[derive(Default, Debug)]
pub struct FxViewerApp {
    pub market_data_mutex: Arc<Mutex<consumer::MarketData>>,
}

impl FxViewerApp {
    /// Called once before the first frame.
    pub fn init(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();

        let (ctx_tx, ctx_rx) = mpsc::channel();

        // create Market Data structre
        let market_data = consumer::MarketData::new();

        let market_data_mutex = Arc::new(Mutex::new(market_data));
        let market_data_mutex_ui_clone = Arc::clone(&market_data_mutex);
        let market_data_mutex_fx_clone = Arc::clone(&market_data_mutex);

        thread::spawn(move || {
            // start fx data thread
            let rec_ctx: Context = ctx_rx.recv().unwrap();
            run_async_fx_data(rec_ctx, market_data_mutex_fx_clone);
        }); // end of fx data thread

        if let Err(e) = ctx_tx.send(ctx) {
            error!("error sending from ctx channel - {e}");
            exit(1);
        }

        Self {
            market_data_mutex: market_data_mutex_ui_clone,
        }
    }
}

pub fn run_async_fx_data(rec_ctx: Context, market_data_mutex: Arc<Mutex<consumer::MarketData>>) {
    run(async {
        consumer::start(rec_ctx, market_data_mutex).await;
    });
}

impl eframe::App for FxViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let market_data = self.market_data_mutex.lock().unwrap(); // panic if can't get lock
        //println!("FXviewer market data: {:?}", *market_data);
        println!(
            "latest CITI buy points: {:?}",
            market_data.citi_buy_points.last()
        );
        println!(
            "latest BARX buy points: {:?}",
            market_data.barx_buy_points.last()
        );

        egui::CentralPanel::default().show(ctx, |ui| {
            let buy_plotpoints = PlotPoints::from(market_data.citi_buy_points.clone());
            let citi_line = Line::new("CITI 1M Buy Price", buy_plotpoints);
            let barx_plotpoints = PlotPoints::from(market_data.barx_buy_points.clone());
            let barx_line = Line::new("BARX 1M Buy Price", barx_plotpoints);
            Plot::new("my_plot")
                .x_axis_label("Time(secs)")
                .y_axis_label("EUR/USD Price")
                .legend(Legend::default().title("EUR/USD 1M Prices"))
                //  .view_aspect(2.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(citi_line);
                    plot_ui.line(barx_line);
                });
        });
    }
}
