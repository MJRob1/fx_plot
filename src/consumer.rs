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

#[derive(Debug, Default)]
pub struct MarketData {
    pub lp: String,
    pub currency_pair: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub timestamp: u64,
}

impl MarketData {
    pub fn update(&mut self, market_data: &str) -> Result<(), AppError> {
        extract_market_data(self, market_data)?;
        Ok(())
    }
    pub fn new() -> Self {
        MarketData {
            lp: String::new(),
            currency_pair: String::new(),
            buy_price: 0.0,
            sell_price: 0.0,
            timestamp: 0,
        }
    }
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
                            ctx.request_repaint();
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
    let currency_pair = get_str_field(market_data_params.next())?;
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
    market_data.lp = liquidity_provider.to_string();
    market_data.currency_pair = currency_pair.to_string();
    market_data.buy_price = one_mill_buy_price;
    market_data.sell_price = one_mill_sell_price;
    market_data.timestamp = timestamp;

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
