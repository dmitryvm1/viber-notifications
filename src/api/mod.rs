use super::AppStateType;
use actix_web::middleware::identity::RequestIdentity;
use actix_web::*;
use bitcoin;
use chrono::TimeZone;
use common::*;
use futures::prelude::*;
use models::NewPost;
use models::Post;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::ops::Deref;
use viber::messages::CallbackMessage;
use viber::raw;
use std::borrow::BorrowMut;
use common::messages::TomorrowForecast;
use weather::WeatherInquirer;
use actix::Recipient;

pub fn list(
    (state, query): (State<AppStateType>, Query<HashMap<String, String>>),
) -> Result<HttpResponse, Error> {
    let mut ctx = tera::Context::new();
    ctx.insert("text", &"Welcome!".to_owned());
    let ts = state.lock().unwrap().last_text_broadcast.last_success;
    ctx.insert("last_broadcast", &chrono::Utc.timestamp(ts, 0).to_rfc2822());
    ctx.insert("members", &state.lock().unwrap().viber.subscribers);
    let html = state.lock().unwrap().template.render("index.html", &ctx).map_err(|e| {
        error!("Template error! {:?}", e);
        error::ErrorInternalServerError("Template error")
    })?;
    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}

pub fn viber_webhook(
    req: &HttpRequest<AppStateType>,
) -> Box<Future<Item = HttpResponse, Error = Error>> {
    use std::borrow::Cow;

    let addr:Option<Recipient<TomorrowForecast>> = req.state().lock().unwrap().addr.clone();
    let key = req.state().lock().unwrap().viber.api_key.clone();
    let kb = Some(get_default_keyboard());

    req.payload()
        .concat2()
        .from_err()
        .and_then(move |body| {
            let cb_msg: Result<CallbackMessage, serde_json::Error> =
                serde_json::from_slice(&body);
            info!("viber hook {:?}", cb_msg);
            match cb_msg {
                Ok(ref msg) => {
                    match msg.event.as_ref() {
                        "conversation_started" => {
                            info!("message parsed {:?}", msg);
                            let user = msg.user.as_ref().unwrap();
                            raw::send_text_message(
                                "Welcome to Kiev Alerts",
                                &user.id.to_string(),
                                &key,
                                kb,
                            )
                            .wait();
                        },
                        "message" => {
                            let user = msg.sender.as_ref().unwrap().id.as_ref().unwrap();
                            let message = msg.message.as_ref().unwrap();
                            match message.text.as_ref() {
                                "bitcoin" => {
                                    let price = bitcoin::get_bitcoin_price();
                                    info!("btc {:?}", price);
                                    if price.is_some() {
                                        let price = price.unwrap();
                                        let msg_text = format!(
                                            "{} \n1 BTC = {} $",
                                            price.time.updateduk, price.bpi.USD.rate
                                        );
                                        raw::send_text_message(
                                            msg_text.as_str(),
                                            &user.to_string(),
                                            &key,
                                            kb,
                                        )
                                        .wait();
                                    } else {
                                        error!("Could not get bitcoin price.");
                                    }
                                },
                                "forecast_kiev_tomorrow" => {
                                    info!("message parsed {:?}", msg);
                                    addr.unwrap().do_send(TomorrowForecast { user_id: user.to_string() });
                                }

                                _ => {}
                            }
                        },
                        _ => {}
                    }
                    Ok(HttpResponse::Ok().content_type("text/plain").body(""))
                }
                Err(e) => {
                    debug!("Error parsing json, {:?}", e);
                    Ok(HttpResponse::Ok().content_type("text/plain").body(""))
                }
            }
        })
        .responder()
}

pub fn send_message(
    req: &HttpRequest<AppStateType>,
) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let state = req.state();
    let config = &state.lock().unwrap().config;
    let viber_api_key = &config.viber_api_key;
    let key = &viber_api_key.as_ref();
    super::viber::raw::send_text_message(
        "Hi",
        config.admin_id.as_ref().unwrap().as_str(),
        key.unwrap(),
        None,
    )
    .from_err()
    .and_then(|response| {
        response.body().poll()?;
        Ok(HttpResponse::Ok().content_type("text/plain").body("sent"))
    })
    .responder()
}

pub fn acc_data(
    req: &HttpRequest<AppStateType>,
) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let state = req.state();
    let config: &super::config::Config = &state.lock().unwrap().config;
    super::viber::raw::get_account_data(config.viber_api_key.as_ref().unwrap())
        .from_err()
        .and_then(|response| {
            response.body().from_err().and_then(|data| {
                let contents = String::from_utf8(data.to_vec()).unwrap_or("".to_owned());
                Ok(HttpResponse::Ok().content_type("text/plain").body(contents))
            })
        })
        .responder()
}

pub fn index(req: &HttpRequest<AppStateType>) -> Result<HttpResponse, Error> {
    let state = req.state();
    if req.identity().is_none() {
        let mut ctx = tera::Context::new();
        ctx.insert("app_name", "Viber Alerts");
        let html = state.lock().unwrap().template.render("login.html", &ctx).map_err(|e| {
            error!("Template error! {:?}", e);
            error::ErrorInternalServerError("Template error")
        })?;
        Ok(HttpResponse::Ok().content_type("text/html").body(html))
    } else {
        let mut ctx = tera::Context::new();
        ctx.insert("text", &"Welcome!".to_owned());
        let ts = state.lock().unwrap().last_text_broadcast.last_success;
        ctx.insert("last_broadcast", &chrono::Utc.timestamp(ts, 0).to_rfc2822());
        ctx.insert("members", &state.lock().unwrap().viber.subscribers);
        let html = state.lock().unwrap().template.render("index.html", &ctx).map_err(|e| {
            error!("Template error! {:?}", e);
            error::ErrorInternalServerError("Template error")
        })?;
        Ok(HttpResponse::Ok().content_type("text/html").body(html))
    }
}

#[derive(Deserialize)]
pub struct LoginParams {
    email_or_name: String,
    password: String,
}

pub fn login((req, params): (HttpRequest<AppStateType>, Form<LoginParams>)) -> HttpResponse {
    {
        let pool = &req.state().lock().unwrap().pool;
        let new_post = NewPost {
            body: "test",
            title: "title",
        };
        Post::insert(new_post, pool.get().unwrap().deref()).unwrap_or_else(|e| {
            error!("Failed to insert post");
            0
        });
    }

    req.remember("user1".to_owned());
    HttpResponse::Found().header("location", "/").finish()
}

pub fn users(req: &HttpRequest<AppStateType>) -> HttpResponse {
    let pool = &req.state().lock().unwrap().pool;
    let users = Post::all(pool.get().unwrap().deref()).unwrap();
    HttpResponse::Ok().body(format!("{:?}", users))
}

pub fn logout(req: &HttpRequest<AppStateType>) -> HttpResponse {
    req.forget();
    HttpResponse::Found().header("location", "/").finish()
}
