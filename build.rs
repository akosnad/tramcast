#[derive(serde::Deserialize)]
struct Config {
    wifi_ssid: String,
    wifi_password: String,
    mqtt_endpoint: String,
    mqtt_client_id: String,
}

macro_rules! config_entry_to_env {
    ($config:ident, $env:ident, $name:ident) => {
        println!("cargo:rustc-env={}={}", stringify!($env), $config.$name);
    };
}

fn main() {
    embuild::espidf::sysenv::output();

    let config_file = std::fs::read_to_string("config.yml").expect("config.yml not found");
    let config: Config = serde_yaml::from_str(&config_file).expect("config.yml is invalid");

    config_entry_to_env!(config, ESP_WIFI_SSID, wifi_ssid);
    config_entry_to_env!(config, ESP_WIFI_PASS, wifi_password);
    config_entry_to_env!(config, ESP_MQTT_ENDPOINT, mqtt_endpoint);
    config_entry_to_env!(config, ESP_MQTT_CLIENT_ID, mqtt_client_id);
}
