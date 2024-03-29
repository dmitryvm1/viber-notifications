use std::borrow::Cow;

#[derive(Serialize, Deserialize, Debug)]
pub struct Location {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub role: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Sender<'a> {
    pub id: Option<Cow<'a, str>>,
    pub name: Cow<'a, str>,
    pub avatar: Cow<'a, str>,
    pub country: Option<Cow<'a, str>>,
    pub language: Option<Cow<'a, str>>,
    pub api_version: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ViberMessage<'a> {
    #[serde(rename = "type")]
    pub _type: Cow<'a, str>,
    pub text: Option<Cow<'a, str>>,
    pub media: Option<Cow<'a, str>>,
    pub location: Option<Location>,
    pub tracking_data: Option<Cow<'a, str>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum EventTypes<'a> {
    Subscribed,
    Unsubscribed,
    ConversationStarted,
    Delivered,
    Failed,
    Message,
    Seen,
    #[doc(hidden)]
    Unknown(&'a str),
}

impl<'a> EventTypes<'a> {
    pub fn value(&self) -> &'a str {
        match self {
            EventTypes::Subscribed => "subscribed",
            EventTypes::Unsubscribed => "unsubscribed",
            EventTypes::ConversationStarted => "conversation_started",
            EventTypes::Delivered => "delivered",
            EventTypes::Failed => "failed",
            EventTypes::Message => "message",
            EventTypes::Seen => "seen",
            EventTypes::Unknown(s) => s,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Button<'s> {
    #[serde(rename = "ActionType")]
    pub action_type: Cow<'s, str>,
    #[serde(rename = "ActionBody")]
    pub action_body: Cow<'s, str>,
    #[serde(rename = "Text")]
    pub text: Cow<'s, str>,
    #[serde(rename = "TextSize")]
    pub text_size: Cow<'s, str>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Keyboard<'s> {
    #[serde(rename = "Type")]
    pub _type: Cow<'s, str>,
    #[serde(rename = "DefaultHeight")]
    pub default_height: bool,
    #[serde(rename = "Buttons")]
    pub buttons: Vec<Button<'s>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AccountInfo {
    pub status: i64,
    pub status_message: String,
    pub id: String,
    pub name: String,
    pub uri: String,
    pub icon: String,
    pub background: String,
    pub category: String,
    pub subcategory: String,
    pub location: Location,
    pub country: String,
    pub webhook: String,
    pub event_types: Vec<String>,
    pub members: Vec<Member>,
    pub subscribers_count: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TextMessage<'s> {
    pub receiver: Cow<'s, str>,
    pub min_api_version: i64,
    pub sender: Sender<'s>,
    pub tracking_data: Cow<'s, str>,
    #[serde(rename = "type")]
    pub _type: Cow<'s, str>,
    pub keyboard: Option<Keyboard<'s>>,
    pub text: Cow<'s, str>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileMessage<'s> {
    pub receiver: Cow<'s, str>,
    pub min_api_version: i64,
    pub sender: Sender<'s>,
    pub tracking_data: Cow<'s, str>,
    #[serde(rename = "type")]
    pub _type: Cow<'s, str>,
    pub media: Cow<'s, str>,
    pub keyboard: Option<Keyboard<'s>>,
    pub size: usize,
    pub file_name: Cow<'s, str>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PictureMessage<'s> {
    pub receiver: Cow<'s, str>,
    pub min_api_version: i64,
    pub sender: Sender<'s>,
    pub tracking_data: Cow<'s, str>,
    #[serde(rename = "type")]
    pub _type: Cow<'s, str>,
    pub keyboard: Option<Keyboard<'s>>,
    pub media: Cow<'s, str>,
    pub text: Cow<'s, str>,
    pub thumbnail: Cow<'s, str>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VideoMessage<'s> {
    pub receiver: Cow<'s, str>,
    pub min_api_version: i64,
    pub sender: Sender<'s>,
    pub tracking_data: Cow<'s, str>,
    #[serde(rename = "type")]
    pub _type: Cow<'s, str>,
    pub keyboard: Option<Keyboard<'s>>,
    pub media: Cow<'s, str>,
    pub size: usize,
    pub duration: u16,
    pub thumbnail: Cow<'s, str>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct User<'s> {
    pub id: Cow<'s, str>,
    pub name: Cow<'s, str>,
    pub avatar: Cow<'s, str>,
    pub country: Cow<'s, str>,
    pub language: Cow<'s, str>,
    pub api_version: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CallbackMessage<'s> {
    pub event: Cow<'s, str>,
    pub timestamp: u64,
    pub message_token: u64,
    pub message: Option<ViberMessage<'s>>,
    pub sender: Option<Sender<'s>>,
    pub user_id: Option<Cow<'s, str>>,
    #[serde(rename = "type")]
    pub _type: Option<Cow<'s, str>>,
    pub context: Option<Cow<'s, str>>,
    pub user: Option<User<'s>>,
    pub subscribed: Option<bool>,
}

impl<'a> Sender<'a> {
    pub fn new(from: &str) -> &mut Self {
        &mut Sender {
            id: None,
            api_version: None,
            country: None,
            language: None,
            name: Cow::from(from),
            avatar: Cow::from("")
        }
    }

    pub fn avatar(&mut self, url: &str) -> &mut Self {
        self.avatar = Cow::from(url);
        self
    }

    pub fn id(&mut self, id: &str) -> &mut Self {
        self.id = Some(Cow::from(id));
        self
    }

    pub fn language(&mut self, language: &str) -> &mut Self {
        self.language = Some(Cow::from(language));
        self
    }

    pub fn country(&mut self, country: &str) -> &mut Self {
        self.country = Some(Cow::from(country));
        self
    }

    pub fn api_version(&mut self, api_version: i64) -> &mut Self {
        self.api_version = Some(api_version);
        self
    }
}
