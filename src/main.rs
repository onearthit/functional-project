use core::convert::TryInto;

use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};

use esp_idf_hal::adc::{AdcContDriver, AdcMeasurement, Attenuated};
use esp_idf_svc::hal::adc::AdcContConfig;
use esp_idf_svc::hal::prelude::Peripherals;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::mqtt::client::{EspMqttClient, EventPayload, MqttClientConfiguration, QoS};
use esp_idf_svc::wifi::{BlockingWifi, EspWifi, WifiEvent};
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};

use log::{debug, info};

const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASS");
const BROKER_URL: &str = env!("MQTT_BROKER_URL");

fn init_wifi() -> anyhow::Result<BlockingWifi<EspWifi<'static>>> {
    info!("Connecting to Wi-Fi: {}, password: {}", SSID, PASSWORD);

    let peripherals = Peripherals::take()?;
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
    connect_wifi(&mut wifi)?;
    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;
    info!("Wifi DHCP info: {:?}", ip_info);

    
    Ok(wifi)
}

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    init_analog_digital_driver(peripherals)

    // let wifi = match init_wifi() {
    //     Ok(wifi) => Some(wifi),
    //     Err(err) => {
    //         info!("WiFi Error: {:?}", err);
    //         None
    //     }
    // };

    // let mut mqtt = init_mqtt()?;

    // loop {
    //     let ip_info = wifi.as_ref().unwrap().wifi().sta_netif().get_ip_info()?;
    //     info!("Wifi DHCP info: {:?}", ip_info);
    //     std::thread::sleep(core::time::Duration::from_secs(1));
    //     mqtt.subscribe("/p/topic", QoS::AtMostOnce);
    // }
}

fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>) -> anyhow::Result<()> {
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: PASSWORD.try_into().unwrap(),
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

fn init_mqtt() -> anyhow::Result<EspMqttClient<'static>> {
    // Initialize the MQTT client here
    info!("Initializing MQTT client");

    // Set up handle for MQTT Config
    let mqtt_config = MqttClientConfiguration::default();

    // Create Client Instance and Define Behaviour on Event
    let client = EspMqttClient::new_cb(
        format!("{}{}", BROKER_URL, "/p/topic").as_str(),
        &mqtt_config,
        move |msg| {
            match msg.payload() {
                EventPayload::BeforeConnect => {
                    println!("Connecting to broker");
                },
                EventPayload::Received { id, topic, data, details } => {
                    println!("Received message: {:?}", data);
                },
                EventPayload::Connected(_) => {
                    println!("Connected to broker");
                },
                EventPayload::Disconnected => {
                    println!("Disconnected from broker");
                },
                _ => {
                    println!("por king ga {:?}", msg.payload());
                }
            }
        }
    )?;
    Ok(client)
}

fn init_analog_digital_driver(peripherals: Peripherals) -> anyhow::Result<()> {
   
    let mut adc = AdcContDriver::new(
        peripherals.adc1 ,
        peripherals.i2s0,
        &AdcContConfig::default() , 
        Attenuated::db11(peripherals.pins.gpio36)
    )?;

    adc.start()?;

     let mut samples = [AdcMeasurement::default(); 10];

     loop {
         if let Ok(num_read) = adc.read(&mut samples, 10) {
            //  println!("Read {} measurement.", num_read);
             for index in 0..num_read {
                println!("{}", samples[index].data());
             }
             std::thread::sleep(core::time::Duration::from_millis(10));
         }
     }

    // Ok(())
}