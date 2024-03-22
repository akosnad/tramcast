use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct TramData {
    #[serde(rename = "stopHeadsign")]
    stop_headsign: Option<String>,
    #[serde(rename = "arrivalTime")]
    arrival_time: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "departureTime")]
    departure_time: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "predictedArrivalTime")]
    predicted_arrival_time: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "predictedDepartureTime")]
    predicted_departure_time: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Tram {
    #[serde(rename = "departAt")]
    depart_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "timeLeftMs")]
    time_left_ms: Option<u64>,
    //data: TramData,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Metro {}

pub enum StateEvent {
    WifiConnected(bool),
    MqttConnected(bool),
    TramStateChanged(Tram),
    MetroStateChanged(Metro),
}
