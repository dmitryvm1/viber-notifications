use forecast::ApiResponse;
use AppStateType;
use chrono::*;
use forecast::*;
use std::io::Read;

static LATITUDE: f64 = 50.4501;
static LONGITUDE: f64 = 30.5234;

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

pub struct WeatherInquirer {
    pub app_state: AppStateType,
    pub last_response: Option<ApiResponse>,
    pub last_broadcast: i64,
    pub last_subscriber_update: i64
}

impl WeatherInquirer {
    pub fn new(app_state: AppStateType) -> WeatherInquirer {
        WeatherInquirer {
            app_state,
            last_response: None,
            last_broadcast: 0,
            last_subscriber_update: 0
        }
    }
}

impl WeatherInquirer {
    pub fn inquire_if_needed(&mut self) -> Result<bool, failure::Error> {
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
        let now = Utc::now().with_timezone(&FixedOffset::east(2*3600));
        if (now.timestamp() - self.last_broadcast > 60 * 60 * 24) && (now.hour() >= 19 && now.hour() <= 21) {
            return true;
        }
        debug!("Should broadcast: false. Hour: {}", now.hour());
        false
    }

    pub fn broadcast_forecast(&mut self) -> Result<(), failure::Error> {
        if !self.should_broadcast() {
            return Ok(());
        }
        {
            let day = self.tomorrow()?;
            let dt = Utc.timestamp(day.time as i64, 0);
            let (precip, probability) = match day.precip_type.as_ref() {
                Some(p) => {
                    let pr = match p {
                        PrecipType::Rain => "Дождь",
                        PrecipType::Snow => "Снег",
                        PrecipType::Sleet => "Дождь со снегом"
                    };
                    (pr, day.precip_probability.unwrap())
                },
                None => ("-", 0.0)
            };
            let msg = format!("Прогноз на завтра {}.{}: \nТемпература: {:?} - {:?} \nОсадки: {:?} с вероятностью {}%", dt.day(),
                              dt.month(),
                              day.temperature_low.ok_or(
                                  JsonError::MissingField { name: "temperature_low".to_owned() }
                              )?,
                              day.temperature_high.ok_or(
                                  JsonError::MissingField { name: "temperature_high".to_owned() }
                              )?, precip, probability * 100.0);
            info!("Sending viber message");
            // self.app_state.viber.lock().unwrap().broadcast_text(msg.as_str())?;
            self.app_state.viber.lock().unwrap().send_text_to_admin(msg.as_str())?;
        }
        self.last_broadcast = Utc::now().timestamp();
        Ok(())
    }
}