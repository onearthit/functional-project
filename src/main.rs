use core::convert::TryInto;

use accumfft::sma::SMA;
use accumfft::accumfft::AccumFFT;


use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};


use esp_idf_hal::adc::{ AdcContDriver, AdcMeasurement, Attenuated};
use esp_idf_svc::hal::adc::AdcContConfig;
use esp_idf_svc::hal::prelude::Peripherals;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::mqtt::client::{EspMqttClient, EventPayload, MqttClientConfiguration, QoS};
use esp_idf_svc::wifi::{BlockingWifi, EspWifi, WifiEvent};
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};
use esp_idf_hal::reset::restart;

use log::info;

#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
    #[default("")]
    mqtt_url: &'static str,
    #[default("")]
    mqtt_topic: &'static str
}

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let app_config = CONFIG;
    let mut accum = AccumFFT::new(50.0);
    let mut sma = SMA::new(0.05);

    info!("Connecting to Wi-Fi: {}, password: {}", app_config.wifi_ssid, app_config.wifi_psk);
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    sys_loop.subscribe::<WifiEvent, _>(|event: WifiEvent| match event {
        WifiEvent::StaDisconnected(reason) => {
            info!("WiFi disconnected. Reason: {:?}", reason)
        }
        _ => {}
    })?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    if let Err(e) = connect_wifi(&app_config, &mut wifi) {
        info!("Failed to connect to Wi-Fi: {:?}", e);
        restart();
    }

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;
    info!("Wifi DHCP info: {:?}", ip_info);

    let mut mqtt = match init_mqtt(&app_config) {
        Ok(mqtt) => mqtt,
        Err(err) => {
            info!("MQTT Error: {:?}", err);
            return Err(err);
        }
    };

    let mut adc = AdcContDriver::new(
        peripherals.adc1,
        peripherals.i2s0,
        &AdcContConfig::default(),
        Attenuated::db11(peripherals.pins.gpio36),
    )?;

    adc.start()?;
    let mut samples = [AdcMeasurement::default(); 100];
    loop {
        if let Ok(num_read) = adc.read(&mut samples, 100) {
            for index in 0..num_read {
                accum.feed(samples[index].data() as f32);
            }
            if let Some(amplitude) = accum.amplitude() {
                sma.feed(amplitude);
                let smoothed_amplitude = sma.value();
                if let Err(err) = mqtt.publish(
                    app_config.mqtt_topic,
                    QoS::AtLeastOnce,
                    false,
                    format!("{}", smoothed_amplitude).as_bytes(),
                ) {
                    info!("Failed to publish MQTT message: {:?}", err);
                    restart();
                }
            } else {
                info!("Amplitude is None, skipping MQTT publish");
            }
            accum.reset();
        }
        std::thread::sleep(core::time::Duration::from_millis(100));
    }
}

fn connect_wifi(app_config: &Config, wifi: &mut BlockingWifi<EspWifi<'static>>) -> anyhow::Result<()> {
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: app_config.wifi_ssid.try_into().unwrap(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: app_config.wifi_psk.try_into().unwrap(),
        channel: None,
        ..Default::default()
    }))?;

    wifi.start()?;
    info!("Wifi started");

    wifi.connect()?;
    info!("Wifi connected");

    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    Ok(())
}


fn init_mqtt(app_config: &Config) -> anyhow::Result<EspMqttClient<'static>> {
    info!("Initializing MQTT client");
    let mqtt_config = MqttClientConfiguration::default();

    let client = EspMqttClient::new_cb(
        format!("{}", app_config.mqtt_url).as_str(),
        &mqtt_config,
        move |msg| {
            match msg.payload() {
                EventPayload::BeforeConnect => {
                    println!("Connecting to broker");
                },
                EventPayload::Received { id, topic, data, details } => {
                    println!("Received message: id={}, topic={:?}, data={:?} , details={:?}", id, topic, data , details);
                },
                EventPayload::Connected(_) => {
                    println!("Connected to broker");
                },
                EventPayload::Disconnected => {
                    println!("Disconnected from broker");
                    restart();
                },
                _ => {
                    println!("Unhandle : {:?}", msg.payload());
                }
            }
        }
    )?;
    Ok(client)
}
