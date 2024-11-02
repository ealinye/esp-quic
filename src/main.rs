use std::net::{Ipv4Addr, SocketAddr};

use esp_idf_svc::{
    // hal::{delay::FreeRtos, prelude::*},
    sys::EspError,
};
use rustls::pki_types::CertificateDer;
use tokio::io::AsyncWriteExt;

mod rgb_led;
mod usc_impl;

fn main() -> Result<(), Box<dyn core::error::Error>> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    ::log::set_max_level(log::LevelFilter::Trace);

    // Keep it around or else the wifi will stop
    let _wifi = wifi_create()?;

    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs()
        .into_iter()
        .flatten()
    {
        _ = roots.add(cert);
    }

    let ca = CertificateDer::from(include_bytes!("ca.cert").to_vec());
    roots.add(ca).expect("failed to add CA certificate");

    let provider = rustls::crypto::ring::default_provider().into();
    let mut tls_config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("unsupported TLS version")
        .with_root_certificates(roots)
        .with_no_client_auth();
    tls_config.alpn_protocols.extend([b"h3".to_vec()]);

    usc_impl::install();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("failed to build tokio runtime");

    let result: std::io::Result<()> = rt.block_on(async move {
        let local_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        let client = quic::QuicClient::bind_with_tls([local_addr], tls_config).build();

        let conn = client
            .connect("localhost", "192.168.165.224:14332".parse().unwrap())
            .expect("failed to connect");

        let (sid, (reader, mut writer)) = conn.open_bi_stream().await?.expect("unreachable");
        writer.write_all(b"Hello, world!").await?;
        writer.shutdown().await?;

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        Ok(())
    });

    // let peripherals = Peripherals::take().unwrap();
    // let led = peripherals.pins.gpio48;
    // let channel = peripherals.rmt.channel0;
    // let mut ws2812 = rgb_led::WS2812RMT::new(led, channel)?;
    // loop {
    //     log::info!("Red!");
    //     ws2812.set_pixel([255, 0, 0])?;
    //     FreeRtos::delay_ms(1000);
    //     log::info!("Green!");
    //     ws2812.set_pixel([0, 255, 0])?;
    //     FreeRtos::delay_ms(1000);
    //     log::info!("Blue!");
    //     ws2812.set_pixel([0, 0, 255])?;
    //     FreeRtos::delay_ms(1000);
    // }
    Ok(result?)
}

fn wifi_create() -> Result<esp_idf_svc::wifi::EspWifi<'static>, EspError> {
    use esp_idf_svc::eventloop::*;
    use esp_idf_svc::hal::prelude::Peripherals;
    use esp_idf_svc::nvs::*;
    use esp_idf_svc::wifi::*;

    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let peripherals = Peripherals::take()?;

    let mut esp_wifi = EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs.clone()))?;
    let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sys_loop.clone())?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: "iQOO Z9x".try_into().unwrap(),
        password: "00000000".try_into().unwrap(),
        ..Default::default()
    }))?;

    wifi.start()?;
    log::info!("Wifi started");

    // too slow to scan once(
    // for scaned in wifi.scan()? {
    //     log::info!("Scaned: {:?}", scaned);
    // }

    wifi.connect()?;
    log::info!("Wifi connected");

    wifi.wait_netif_up()?;
    log::info!("Wifi netif up");

    Ok(esp_wifi)
}
