use std::borrow::Cow;
use viber::messages::{Button, Keyboard};

pub mod messages;

pub fn get_default_keyboard<'a>() -> Keyboard<'a> {
    Keyboard {
        DefaultHeight: true,
        Type: Cow::from("keyboard"),
        Buttons: vec![
            Button {
                ActionBody: Cow::from("bitcoin"),
                ActionType: Cow::from("reply"),
                Text: Cow::from("Bitcoin Price"),
                TextSize: Cow::from("regular"),
            },
            Button {
                ActionBody: Cow::from("forecast_kiev_tomorrow"),
                ActionType: Cow::from("reply"),
                Text: Cow::from("Weather For Tomorrow"),
                TextSize: Cow::from("regular"),
            },
        ],
    }
}