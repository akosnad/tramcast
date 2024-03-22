use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Tram {
    #[serde(rename = "departAt")]
    pub depart_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "timeLeftMs")]
    pub time_left_ms: Option<u64>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Metro {
    #[serde(rename = "departAt")]
    pub depart_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "timeLeftMs")]
    pub time_left_ms: Option<u64>,
}

pub enum StateEvent {
    WifiConnected(bool),
    MqttConnected(bool),
    TramStateChanged(Tram),
    MetroStateChanged(Metro),
}
