use self::types::*;
use actix_web::client;
use actix_web::error::{Error, PayloadError};
use actix_web::HttpMessage;
use futures::Future;
use serde_json;
// use serde_json::*;
use failure;
use weather::CustomError;

pub mod types;

pub fn get_bitcoin_price() -> Option<BTCPrice> {
    let response = client::get("http://api.coindesk.com/v1/bpi/currentprice.json")
        .finish()
        .unwrap()
        .send()
        .wait();
    match response {
        Ok(r) => {
            r
                .body()
                .and_then(|data| Ok(serde_json::from_slice(&data).ok()))
                .wait().ok().unwrap_or(None)
        },
        _ => None
    }
}