mod consumer;

use eframe::egui;
use egui::Context;
use egui_plot::{Line, Plot, PlotPoints};
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
    pub buy_plotpoints: Vec<[f64; 2]>,
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

        let buy_points: Vec<[f64; 2]> = Vec::new();
        Self {
            market_data_mutex: market_data_mutex_ui_clone,
            buy_plotpoints: buy_points,
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
        println!("market data: {:?}", *market_data);
        let buy_price = market_data.buy_price;
        let timestamp = market_data.timestamp;
        println!("buy price: {}", buy_price);
        println!("timestamp: {}", timestamp);
        // need to convert timestamp to some sensible plot x axis value

        // need to push only when called from market update not a gui event
        self.buy_plotpoints.push([timestamp as f64, buy_price]);
        println!("buy plotpoints: {:?}", self.buy_plotpoints);

        egui::CentralPanel::default().show(ctx, |ui| {
            let sin: PlotPoints = (0..1000)
                .map(|i| {
                    let x = i as f64 * 0.01;
                    [x, x.sin()]
                })
                .collect();
            let line = Line::new("sin", sin);
            Plot::new("my_plot")
                .view_aspect(2.0)
                .show(ui, |plot_ui| plot_ui.line(line));
        });
    }
}
