#![no_std]
#![no_main]

mod sump;

use sump::SumpParser;
use defmt::info;
use embassy_executor::Spawner;
use embassy_stm32::{bind_interrupts, Config, peripherals, usb};
use embassy_stm32::usb::Driver;
use {defmt_rtt as _, panic_probe as _};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::Builder;
use embassy_futures::join::join;

bind_interrupts!(struct Irqs{
    OTG_HS => usb::InterruptHandler<peripherals::USB_OTG_HS>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("1. Execution reached main()");
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        use embassy_stm32::rcc::VoltageScale;
        use embassy_stm32::time::Hertz;
        config.rcc.hse = Some(Hse{
            freq: Hertz(24_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.hsi = Some(HSIPrescaler::Div1);
        config.rcc.csi = true;
        config.rcc.hsi48 = Some(Hsi48Config{sync_from_usb: true});
        config.rcc.pll1 = Some(Pll{
            source: PllSource::Hsi,
            prediv: PllPreDiv::Div4,
            mul: PllMul::Mul50,
            divp: Some(PllDiv::Div2),
            divq: None,
            divr: None,
            divs: None,
            divt: None,
        });
        config.rcc.sys = Sysclk::Pll1P; //400MHz
        config.rcc.ahb_pre = AHBPrescaler::Div2;
        config.rcc.apb1_pre = APBPrescaler::Div2;
        config.rcc.apb2_pre = APBPrescaler::Div2;
        config.rcc.apb5_pre = APBPrescaler::Div2; //5 and not 3???
        config.rcc.apb4_pre = APBPrescaler::Div2;
        config.rcc.voltage_scale = VoltageScale::High;
        //
        config.rcc.mux.usbphycsel = embassy_stm32::rcc::mux::Usbphycsel::Hse;
    }

    info!("2. Config set, calling embassy_stm32::init...");
    let p = embassy_stm32::init(config);
    info!("3. embassy_stm32::init finished successfully!");

    //
    embassy_stm32::pac::PWR.csr2().modify(|w|{
        w.set_usb33den(true); //enable VDD33USB level detector/supply
        w.set_usbhsregen(true); //enable the HS PHY's regulator — powers up CN2
    });
    while !embassy_stm32::pac::PWR.csr2().read().usb33rdy(){}
    info!("PWR initialized successfully!");

    let mut ep_out_buffer = [0u8; 256];

    let mut config = embassy_stm32::usb::Config::default();
    config.vbus_detection = false;

    let driver = Driver::new_hs(p.USB_OTG_HS, Irqs, p.PM6, p.PM5, &mut ep_out_buffer, config);

    let mut usb_config = embassy_usb::Config::new(0x16c0, 0x05dc);
    usb_config.manufacturer = Some("Custom");
    usb_config.product = Some("STM32H7S3L8 Logic Analyzer");
    usb_config.serial_number = Some("00000001");

    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];

    let mut state = State::new();
    let mut builder = Builder::new(
        driver,
        usb_config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [],
        &mut control_buf,
    );

    let mut class = CdcAcmClass::new(&mut builder, &mut state, 64);
    let mut usb = builder.build();

    let serial_loop = async {
        let mut buf = [0u8; 64];
        loop{
            class.wait_connection().await;
            info!("Connected");
            loop{
                match class.read_packet(&mut buf).await {
                    Ok(n) => {
                        info!("Received {} bytes", n);
                        let _ = class.write_packet(&buf[..n]).await;
                    }
                    Err(_) => {
                        info!("Disconnected");
                        break;
                    }
                }
            }
        }
    };

    join(usb.run(), serial_loop).await;
}