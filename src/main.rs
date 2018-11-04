#![allow(unused_variables)]
#![feature(try_trait)]
#![cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
extern crate actix_web;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate json;
extern crate victoria_dom;
extern crate openssl;
extern crate futures;
#[macro_use]
extern crate log;
extern crate actix;
extern crate env_logger;
extern crate dirs;
extern crate forecast;
extern crate reqwest;
extern crate chrono;
#[macro_use]
extern crate failure;

use chrono::prelude::*;
use forecast::*;
use std::sync::Arc;
use actix_web::{
    http, middleware, App, AsyncResponder, Error, HttpMessage, HttpRequest, HttpResponse,
    Result,
};
// use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use std::env;
use std::io::Read;
use futures::{Future, Stream};
use actix::{AsyncContext, Arbiter, Actor, Context, Running};
use actix_web::server::HttpServer;
use std::borrow::BorrowMut;
use std::sync::Mutex;
use viber::messages::AccountInfo;
use viber::messages::Member;

static APP_NAME: &str = "viber_alerts";

pub mod viber;
pub mod config;

static LATITUDE: f64 = 50.4501;
static LONGITUDE: f64 = 30.5234;

#[cfg(debug_assertions)]
static QUERY_INTERVAL:u64 = 6;
#[cfg(not(debug_assertions))]
static QUERY_INTERVAL:u64 = 60;

#[derive(Debug, Fail)]
enum JsonError {
    #[fail(display = "field is missing: {}", name)]
    MissingField {
        name: String,
    },
    #[fail(display = "error accessing array")]
    ArrayIndex,
}

#[derive(Debug, Fail)]
#[fail(display = "Custom error: {}", msg)]
struct CustomError {
    msg: String
}

struct Viber {
    api_key: String,
    admin_id: String,
    subscribers: Vec<Member>
}

impl Viber {
    pub fn new(api_key: String, admin_id: String) -> Viber {
        Viber {
            api_key,
            admin_id,
            subscribers: Vec::with_capacity(16)
        }
    }

    pub fn update_subscribers(&mut self) -> std::result::Result<(), failure::Error> {
        viber::raw::get_account_data(&self.api_key)
            .from_err()
            .and_then(|response| {
                response.body()
                    .from_err()
                    .and_then(|data| {
                        let account_info: AccountInfo = serde_json::from_slice(&data)?;
                        self.subscribers.clear();
                        for member in account_info.members {
                            println!("Member: {:?}", member);
                            self.subscribers.push(member);
                        }
                        Ok(())
                    })
            }).wait()
    }

    pub fn broadcast_text(&self, text: &str) -> std::result::Result<(), failure::Error> {
        for m in &self.subscribers {
            if self.send_text_to(text, m.id.as_str()).is_err() {
                warn!("Could not send forecast to user: {}", m.name);
            }
        }
        Ok(())
    }

    pub fn send_text_to(&self, text: &str, to: &str) -> std::result::Result<(), failure::Error> {
        viber::raw::send_text_message(text, to, &self.api_key)
            .from_err()
            .and_then(|response| {
                let body = response.body().poll()?;
                Ok(())
            }).wait()
    }

    pub fn send_text_to_admin(&self, text: &str) -> std::result::Result<(), failure::Error> {
        self.send_text_to(text, self.admin_id.as_str())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct MyObj {
    name: String,
    number: i32,
}

type AppStateType = Arc<AppState>;

fn index(req: &HttpRequest<AppStateType>) -> Box<Future<Item=HttpResponse, Error=Error>> {
    req.json()
        .from_err()  // convert all errors into `Error`
        .and_then(|val: MyObj| {
            println!("model: {:?}", val);
            Ok(HttpResponse::Ok().json(val))  // <- send response
        })
        .responder()
}

fn viber_webhook(req: &HttpRequest<AppStateType>) -> Box<Future<Item=HttpResponse, Error=Error>> {
    req.payload()
        .concat2()
        .from_err()
        .and_then(|body| {
            info!("{}", std::str::from_utf8(&body)?);
            Ok(HttpResponse::Ok()
                .content_type("text/plain")
                .body(""))
        }).responder()
}

fn send_message(req: &HttpRequest<AppStateType>) -> Box<Future<Item=HttpResponse, Error=Error>> {
    let state = req.state();
    let config = &state.config;
    let viber_api_key = &config.viber_api_key;
    let key = &viber_api_key.as_ref();
    viber::raw::send_text_message("Hi", config.admin_id.as_ref().unwrap().as_str(), key.unwrap())
        .from_err()
        .and_then(|response| {
            response.body().poll()?;
            Ok(HttpResponse::Ok()
                .content_type("text/plain")
                .body("sent"))
        }).responder()
}

fn send_file_message(req: &HttpRequest<AppStateType>) -> Box<Future<Item=HttpResponse, Error=Error>> {
    let state = req.state();
    let config: &config::Config = &state.config;
    viber::raw::send_file_message(format!("{}css/styles.css", config.domain_root_url.as_ref().unwrap().as_str()).as_str(),
                                  "styles.css", 3506, config.admin_id.as_ref().unwrap().as_str(),
                                  config.viber_api_key.as_ref().unwrap())
        .from_err()
        .and_then(|response| {
            response.body().poll()?;
            Ok(HttpResponse::Ok()
                .content_type("text/plain")
                .body("sent"))
        }).responder()
}

fn acc_data(req: &HttpRequest<AppStateType>) -> Box<Future<Item=HttpResponse, Error=Error>> {
    let state = req.state();
    let config: &config::Config = &state.config;
    viber::raw::get_account_data(config.viber_api_key.as_ref().unwrap())
        .from_err()
        .and_then(|response| {
            response.body()
                .from_err()
                .and_then(|data| {
                    let contents = String::from_utf8(data.to_vec()).unwrap_or("".to_owned());
                    Ok(HttpResponse::Ok()
                        .content_type("text/plain")
                        .body(contents))
                })
        }).responder()
}

struct WeatherInquirer {
    app_state: AppStateType,
    last_response: Option<ApiResponse>,
    last_broadcast: i64,
    last_subscriber_update: i64
}

impl WeatherInquirer {
    fn new(app_state: AppStateType) -> WeatherInquirer {
        WeatherInquirer {
            app_state,
            last_response: None,
            last_broadcast: 0,
            last_subscriber_update: 0
        }
    }
}

impl WeatherInquirer {
    fn inquire_if_needed(&mut self) -> Result<bool, failure::Error> {
        if self.last_response.is_none() {
            self.last_response = self.inquire().map_err(|e| {
                error!("Error while requesting forecast: {:?}", e.as_fail())
            }).ok();
            return Ok(true);
        } else {
            let today = Utc::now();
            // check if the second daily forecast is for today:
            let dt = {
                let lr = self.last_response.as_ref().unwrap();
                let daily = lr.daily.as_ref().ok_or(JsonError::MissingField { name: "daily".to_owned() })?;
                let first = daily.data.get(1).ok_or(JsonError::ArrayIndex)?;
                Utc.timestamp(first.time as i64, 0)
            };
            if dt.day() == today.day() {
                return Ok(false);
            } else {
                self.last_response = self.inquire().map_err(|e| {
                    error!("Error while requesting forecast: {:?}", e.as_fail())
                }).ok();
                return Ok(true);
            }
        }
    }

    #[allow(dead_code)]
    fn today(&self) -> Result<&DataPoint, failure::Error> {
        if let Some(ref lr) = self.last_response {
            let daily = lr.daily.as_ref().ok_or(JsonError::MissingField { name: "daily".to_owned() })?;
            let first = daily.data.get(1);
            return first.ok_or(failure::Error::from(JsonError::ArrayIndex));
        }
        Err(failure::Error::from(CustomError { msg: "Forecast data is not present.".to_owned() }))
    }

    fn tomorrow(&self) -> Result<&DataPoint, failure::Error> {
        if let Some(ref lr) = self.last_response {
            let daily = lr.daily.as_ref().ok_or(JsonError::MissingField { name: "daily".to_owned() })?;
            let second = daily.data.get(2);
            return second.ok_or(failure::Error::from(JsonError::ArrayIndex));
        }
        Err(failure::Error::from(CustomError { msg: "Forecast data is not present.".to_owned() }))
    }

    fn inquire(&self) -> Result<ApiResponse, failure::Error> {
        let config = &self.app_state.config;
        let api_key = &config.dark_sky_api_key;
        let reqwest_client = reqwest::Client::new();
        let api_client = forecast::ApiClient::new(&reqwest_client);
        let mut blocks = vec![ExcludeBlock::Alerts];

        let forecast_request = ForecastRequestBuilder::new(api_key.as_ref().unwrap().as_str(), LATITUDE, LONGITUDE)
            .exclude_block(ExcludeBlock::Hourly)
            .exclude_blocks(&mut blocks)
            .extend(ExtendBy::Hourly)
            .lang(Lang::Ukranian)
            .units(Units::UK)
            .build();
        info!("Requesting weather forecast");
        let mut forecast_response = api_client.get_forecast(forecast_request)?;
        if !forecast_response.status().is_success() {
            let mut body = String::new();
            forecast_response.read_to_string(&mut body)?;
            return Err(failure::Error::from(CustomError { msg: format!("Dark sky response failure: {}", body) }));
        }
        serde_json::from_reader(forecast_response).map_err(|e| {
            failure::Error::from(e)
        })
    }

    fn should_broadcast(&self) -> bool {
        let now = Utc::now();
        if (now.timestamp() - self.last_broadcast > 60 * 60 * 24) && (now.hour() >= 19 && now.hour() <= 21) {
            return true;
        }
        false
    }

    fn broadcast_forecast(&mut self) -> Result<(), failure::Error> {
        if !self.should_broadcast() {
            return Ok(());
        }
        {
            let day = self.tomorrow()?;
            let dt = Utc.timestamp(day.time as i64, 0);
            let msg = format!("Прогноз на завтра {}.{}: \nТемпература: {:?} - {:?} \nОсадки: {:?} с вероятностью {}%", dt.day(),
                              dt.month(),
                              day.temperature_low.ok_or(
                                  JsonError::MissingField { name: "temperature_low".to_owned() }
                              )?,
                              day.temperature_high.ok_or(
                                  JsonError::MissingField { name: "temperature_high".to_owned() }
                              )?,
                              day.precip_type.as_ref().ok_or(
                                  JsonError::MissingField { name: "precip_type".to_owned() }
                              )?, day.precip_probability.ok_or(
                    JsonError::MissingField { name: "precip_probability".to_owned() }
                )? * 100.0
            );
            info!("Sending viber message");
            self.app_state.viber.lock().unwrap().broadcast_text(msg.as_str())?;
        }
        self.last_broadcast = Utc::now().timestamp();
        Ok(())
    }
}

impl Actor for WeatherInquirer {
    type Context = Context<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(std::time::Duration::new(QUERY_INTERVAL, 0), |_t: &mut WeatherInquirer, _ctx: &mut Context<Self>| {

            if _t.inquire_if_needed().map_err(|e| {
                error!("Error inquiring weather forecast. {}", e.as_fail());
            }).is_ok() {
                if _t.app_state.viber.lock().unwrap().update_subscribers().is_err() {
                    warn!("Failed to read subscribers.");
                }
                _t.broadcast_forecast().map_err(|e| {
                    error!("Error broadcasting weather forecast. {}", e.as_fail());
                });
            }
        });
    }

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        Running::Stop
    }
}

struct AppState {
    pub config: config::Config,
    pub viber: Mutex<Viber>,
}

impl AppState {
    pub fn new(config: config::Config) -> AppState {
        let viber_api_key = config.viber_api_key.clone();
        let admin_id = config.admin_id.clone();
        AppState {
            config: config,
            viber: Mutex::new(Viber::new(viber_api_key.unwrap(), admin_id.unwrap())),
        }
    }
}

fn get_server_port() -> u16 {
    env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080)
}

fn main() {
    env::set_var("RUST_LOG", "viber_alerts=debug");
    env::set_var("RUST_BACKTRACE", "1");
    env_logger::init();
    let sys = actix::System::new(APP_NAME);

    let mut privkey_path = config::Config::get_config_dir(APP_NAME);
    let mut fullchain_path = privkey_path.clone();
    privkey_path.push("privkey.pem");
    fullchain_path.push("fullchain.pem");

    // load ssl keys
    // let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    // builder
    //      .set_private_key_file(privkey_path.to_str().unwrap(), SslFiletype::PEM)
    //       .unwrap();
    //   builder.set_certificate_chain_file(fullchain_path.to_str().unwrap()).unwrap();

    let _server = Arbiter::start(move |_| {
        let state = AppState::new(config::Config::read(APP_NAME));
        let state = Arc::new(state);
        let _state = state.clone();
        let addr = HttpServer::new(
            move || {
                App::with_state(state.clone())
                    .middleware(middleware::Logger::default())
                    .resource("/api/", |r| r.f(index))
                    .resource("/api/send_message/", |r| r.f(send_message))
                    .resource("/api/send_file_message/", |r| r.f(send_file_message))
                    .resource("/api/acc_data/", |r| r.f(acc_data))
                    .resource("/api/viber/webhook", |r| r.method(http::Method::POST).f(viber_webhook))
            })
            .bind(format!("0.0.0.0:{}", get_server_port()))
            .unwrap().workers(1)
            .shutdown_timeout(1)
            .start();
        WeatherInquirer::new(_state)
    });

    let _ = sys.run();
}
