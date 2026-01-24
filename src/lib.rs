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

        // if no market data yet, skip plotting
        if market_data.liquidity_providers.len() == 0 {
            //   println!("Waiting for market data...");
            return;
        }

        // plot liquidity provider buy points
        let first_plotpoints =
            PlotPoints::from(market_data.liquidity_providers[0].buy_points.clone());
        let first_line = Line::new(
            market_data.liquidity_providers[0].name.clone(),
            first_plotpoints,
        );

        // if only one liquidity provider, plot single line
        if market_data.liquidity_providers.len() == 1 {
            egui::CentralPanel::default().show(ctx, |ui| {
                Plot::new("my_plot")
                    .x_axis_label("Time(secs)")
                    .y_axis_label("EUR/USD Price")
                    .legend(Legend::default().title("EUR/USD"))
                    .show(ui, |plot_ui| {
                        plot_ui.line(first_line);
                    });
            });
        } else {
            // plot second liquidity provider buy points too
            let second_plotpoints =
                PlotPoints::from(market_data.liquidity_providers[1].buy_points.clone());
            let second_line = Line::new(
                market_data.liquidity_providers[1].name.clone(),
                second_plotpoints,
            );
            egui::CentralPanel::default().show(ctx, |ui| {
                Plot::new("my_plot")
                    .x_axis_label("Time(secs)")
                    .y_axis_label("EUR/USD Price")
                    .legend(Legend::default().title("EUR/USD\nLiquidity Providers"))
                    .show(ui, |plot_ui| {
                        plot_ui.line(first_line);
                        plot_ui.line(second_line);
                    });
            });
        }
    }
}
