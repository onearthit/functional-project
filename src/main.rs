use core::convert::TryInto;

use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};

use esp_idf_svc::hal::prelude::Peripherals;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi, WifiEvent};
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};

use log::info;

const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASS");

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

    let wifi = match init_wifi() {
        Ok(wifi) => Some(wifi),
        Err(err) => {
            info!("WiFi Error: {:?}", err);
            None
        }
    };

    loop {
        let ip_info = wifi.as_ref().unwrap().wifi().sta_netif().get_ip_info()?;
        info!("Wifi DHCP info: {:?}", ip_info);
        std::thread::sleep(core::time::Duration::from_secs(1));
    }
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
