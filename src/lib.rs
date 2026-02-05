mod consumer;

use eframe::egui;
use egui::Context;
use egui_plot::{AxisHints, GridMark, Legend, Line, Plot, PlotPoints};
use log::error;
use std::ops::RangeInclusive;
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

        let mut lines: Vec<Line> = Vec::new();

        for i in 0..market_data.liquidity_providers.len() {
            let plotpoints =
                PlotPoints::from(market_data.liquidity_providers[i].buy_points.clone());
            let line = Line::new(market_data.liquidity_providers[i].name.clone(), plotpoints);

            lines.push(line);
        }

        let time_formatter = |mark: GridMark, _range: &RangeInclusive<f64>| {
            let seconds = mark.value;
            let start_minute = market_data.liquidity_providers[0].global_start_minute as f64;
            let start_hour = market_data.liquidity_providers[0].global_start_hour as f64;
            get_time_axis_string(seconds, start_hour, start_minute)
        };

        let x_axes = vec![
            AxisHints::new_x()
                .label("Time of Day (hrs:mins to nearest minute)")
                .formatter(time_formatter),
            AxisHints::new_x().label("Time (seconds since start)"),
        ];

        egui::CentralPanel::default().show(ctx, |ui| {
            Plot::new("fx_plot")
                .custom_x_axes(x_axes)
                .y_axis_label("EUR/USD 1M Buy Price")
                .legend(Legend::default().title("EUR/USD\nLiquidity Providers"))
                //  .time_formatter(time_formatter)
                .show(ui, |plot_ui| {
                    for line in lines.into_iter() {
                        plot_ui.line(line);
                    }
                });
        });
    }
}

fn get_time_axis_string(seconds: f64, start_hour: f64, start_minute: f64) -> String {
    const SECONDS_PER_MINUTE: f64 = 60.0;
    const MINUTES_PER_HOUR: f64 = 60.0;
    const SECONDS_PER_HOUR: f64 = SECONDS_PER_MINUTE * MINUTES_PER_HOUR;

    let minutes_part = seconds as u32 / 60;

    if seconds >= SECONDS_PER_HOUR {
        // more than an hour
        let remaining_minutes = minutes_part % 60;

        if remaining_minutes + start_minute as u32 >= 60 {
            // if adding the remaining minutes exceeds 60, increment hour
            let adjusted_hour = (start_hour + (minutes_part / 60) as f64 + 1.0) % 24.0;
            let adjusted_minute = (start_minute + (minutes_part % 60) as f64) - 60.0;
            format!("{:02}:{:02}", adjusted_hour as u32, adjusted_minute as u32)
        } else {
            // no hour increment needed
            let adjusted_hour = (start_hour + (minutes_part / 60) as f64) % 24.0;
            let adjusted_minute = start_minute + (minutes_part % 60) as f64;
            format!("{:02}:{:02}", adjusted_hour as u32, adjusted_minute as u32)
        }
    } else if seconds >= SECONDS_PER_MINUTE {
        // more than a minute
        if minutes_part + start_minute as u32 >= 60 {
            // if adding the minutes exceeds 60, increment hour
            let adjusted_hour = (start_hour + 1.0) % 24.0;
            let adjusted_minute = (start_minute + minutes_part as f64) - 60.0;
            format!("{:02}:{:02}", adjusted_hour as u32, adjusted_minute as u32)
        } else {
            // no hour increment needed
            let adjusted_hour = start_hour;
            let adjusted_minute = start_minute + minutes_part as f64;
            format!("{:02}:{:02}", adjusted_hour as u32, adjusted_minute as u32)
        }
    } else {
        // less than a minute
        String::new()
    }
}
