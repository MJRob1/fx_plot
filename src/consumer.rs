use chrono::Utc;
use chrono::prelude::DateTime;
use egui::Context;
use log::{error, info};
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::{ClientConfig, Message};
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::io;
use std::num::ParseFloatError;
use std::num::ParseIntError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

#[derive(Debug, Default)]
pub struct MarketData {
    pub liquidity_providers: Vec<LpBuyPoints>,
}

impl MarketData {
    pub fn update(&mut self, market_data: &str) -> Result<(), AppError> {
        extract_market_data(self, market_data)?;
        Ok(())
    }
    pub fn new() -> Self {
        Self {
            liquidity_providers: Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct LpBuyPoints {
    pub name: String,
    pub buy_points: Vec<[f64; 2]>,
    pub zero_time_ref: u64,
    pub global_start_hour: f64,
    pub global_start_minute: f64,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum AppError {
    NumParams,
    IsEmpty,
    ParseFloat(ParseFloatError),
    ParseInt(ParseIntError),
    Io(io::Error),
}

impl From<ParseFloatError> for AppError {
    fn from(error: ParseFloatError) -> Self {
        Self::ParseFloat(error)
    }
}

impl From<ParseIntError> for AppError {
    fn from(error: ParseIntError) -> Self {
        Self::ParseInt(error)
    }
}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::IsEmpty => f.write_str("empty data field"),
            Self::NumParams => f.write_str("missing market data fields"),
            Self::ParseFloat(e) => Display::fmt(e, f),
            Self::ParseInt(e) => Display::fmt(e, f),
            Self::Io(e) => Display::fmt(e, f),
        }
    }
}

impl std::error::Error for AppError {}

pub async fn start(ctx: Context, market_data_mutex: Arc<Mutex<MarketData>>) {
    let consumer: StreamConsumer = crate::consumer::create();
    info!("fx_plot kafka consumer started");
    consume(consumer, ctx, market_data_mutex).await;
}

fn create() -> StreamConsumer {
    let mut binding = ClientConfig::new();
    let config = binding
        .set("bootstrap.servers", "localhost:9092")
        .set("group.id", "group1")
        .set("auto.offset.reset", "latest")
        .set("socket.timeout.ms", "6000");

    let consumer: StreamConsumer = config.create().expect("Consumer1 creation error"); // handle error properly later
    consumer
}

async fn consume(
    consumer: StreamConsumer,
    ctx: Context,
    market_data_mutex: Arc<Mutex<MarketData>>,
) {
    info!("fx_plot kafka consumer: consuming messages...");
    consumer
        .subscribe(&["fx-topic"])
        .expect("Consumer1 can't subscribe to specified topic"); // handle error properly later

    loop {
        match consumer.recv().await {
            Err(e) => error!(
                "fx_plot kafka consumer: Error while receiving from Kafka: {:?}",
                e
            ),
            Ok(m) => {
                match m.payload_view::<str>() {
                    None => info!("No payload"),
                    Some(Ok(s)) => {
                        info!("Received message: {}", s);
                        //update market data
                        let mut market_data = market_data_mutex.lock().unwrap(); // panic if can't get lock
                        if let Err(e) = market_data.update(s) {
                            error!("market data not processed - {e}");
                        } else {
                            // println!("consume updated market data: {:?}", *market_data);
                            ctx.request_repaint();
                            // thread::sleep(Duration::from_millis(1));
                        }
                    }
                    Some(Err(e)) => error!(
                        "fx_plot kafka consumer: Error while deserializing message payload: {:?}",
                        e
                    ),
                }
                consumer.commit_message(&m, CommitMode::Async).unwrap();
            }
        }
    }
}

fn extract_market_data(market_data: &mut MarketData, payload: &str) -> Result<(), AppError> {
    let mut vol_prices_vec: Vec<(i32, f64, String)> = Vec::new();
    let mut market_data_params = get_params(payload, 9)?;
    let liquidity_provider = get_str_field(market_data_params.next())?;
    let _currency_pair = get_str_field(market_data_params.next())?;
    let one_mill_buy_price: f64 = market_data_params.next().unwrap_or("").trim().parse()?;
    vol_prices_vec.push((1, one_mill_buy_price, String::from("Buy")));
    let one_mill_sell_price: f64 = market_data_params.next().unwrap_or("").trim().parse()?;
    vol_prices_vec.push((1, one_mill_sell_price, String::from("Sell")));
    let three_mill_buy_price: f64 = market_data_params.next().unwrap_or("").trim().parse()?;
    vol_prices_vec.push((3, three_mill_buy_price, String::from("Buy")));
    let three_mill_sell_price: f64 = market_data_params.next().unwrap_or("").trim().parse()?;
    vol_prices_vec.push((3, three_mill_sell_price, String::from("Sell")));
    let five_mill_buy_price: f64 = market_data_params.next().unwrap_or("").trim().parse()?;
    vol_prices_vec.push((5, five_mill_buy_price, String::from("Buy")));
    let five_mill_sell_price: f64 = market_data_params.next().unwrap_or("").trim().parse()?;
    vol_prices_vec.push((5, five_mill_sell_price, String::from("Sell")));
    let timestamp: u64 = market_data_params.next().unwrap_or("").trim().parse()?;

    //build up liquidity_providers buy points vector
    // how quick is this lookup? should we use a lookup table instead?
    if market_data
        .liquidity_providers
        .iter()
        .all(|lp| lp.name != liquidity_provider)
    {
        let new_lp = LpBuyPoints {
            name: liquidity_provider.to_string(),
            buy_points: Vec::new(),
            zero_time_ref: 0,
            global_start_hour: 0.0,
            global_start_minute: 0.0,
        };
        market_data.liquidity_providers.push(new_lp);
    }

    // find the lp in the vector and add the buy point
    if let Some(lp) = market_data
        .liquidity_providers
        .iter_mut()
        .find(|lp| lp.name == liquidity_provider)
    {
        // set first timestamp as zero time reference
        if lp.buy_points.len() == 0 {
            lp.zero_time_ref = timestamp;
            lp.buy_points.push([0.0, one_mill_buy_price]);
            let d = UNIX_EPOCH + Duration::from_nanos(timestamp);
            let datetime = DateTime::<Utc>::from(d);
            let hour: f64 = match datetime.format("%H").to_string().parse::<f64>() {
                Ok(x) => x,
                Err(error) => {
                    println!("Error converting number: {}", error);
                    println!("Setting num to default: 0");
                    0.0
                }
            };
            let minutes: f64 = match datetime.format("%M").to_string().parse::<f64>() {
                Ok(x) => x,
                Err(error) => {
                    println!("Error converting number: {}", error);
                    println!("Setting num to default: 0");
                    0.0
                }
            };
            lp.global_start_hour = hour;
            lp.global_start_minute = minutes;
        } else {
            let adjusted_timestamp = (timestamp - lp.zero_time_ref) / 1000000000; // convert to seconds for egui plot x axis
            lp.buy_points
                .push([adjusted_timestamp as f64, one_mill_buy_price]);
        }
    }

    Ok(())
}

pub fn get_params(data: &str, number: usize) -> Result<std::str::Split<'_, &str>, AppError> {
    let value = data.split("|");
    if value.clone().count() < number {
        return Err(AppError::NumParams);
    } else {
        Ok(data.split("|"))
    }
}

pub fn get_str_field(field: Option<&str>) -> Result<&str, AppError> {
    let value = field.unwrap_or("");
    if value.trim().is_empty() {
        return Err(AppError::IsEmpty);
    } else {
        Ok(value.trim())
    }
}
