//main.rs
#![no_std]
#![no_main]

mod sump;

use core::char::decode_utf16;
use sump::SumpParser;
use defmt::info;
use embassy_executor::Spawner;
use embassy_stm32::{bind_interrupts, Config, peripherals, usb};
use embassy_stm32::usb::Driver;
use {defmt_rtt as _, panic_probe as _};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::Builder;
use embassy_futures::join::join;
use static_cell::StaticCell;

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

    //let ep_out_buffer = static_cell::make_static!([0u8; 256]);
    static EP_OUT: StaticCell<[u8; 256]> = StaticCell::new();
    let ep_out_buffer = EP_OUT.init([0u8; 256]);

    let mut config = embassy_stm32::usb::Config::default();
    config.vbus_detection = false;

    let driver = Driver::new_hs(p.USB_OTG_HS, Irqs, p.PM6, p.PM5, ep_out_buffer, config);

    let mut usb_config = embassy_usb::Config::new(0x16c0, 0x05dc);
    usb_config.manufacturer = Some("Custom");
    usb_config.product = Some("STM32H7S3L8 Logic Analyzer");
    usb_config.serial_number = Some("00000001");

    static CONF: StaticCell<[u8; 256]> = StaticCell::new();
    let config_descriptor = CONF.init([0u8; 256]);
    static BOS: StaticCell<[u8; 256]> = StaticCell::new();
    let bos_descriptor = BOS.init([0u8; 256]);
    static CTRL: StaticCell<[u8; 64]> = StaticCell::new();
    let control_buf = CTRL.init([0u8; 64]);

    static STATE: StaticCell<State> = StaticCell::new();
    let state= STATE.init(State::new());

    let mut builder = Builder::new(
        driver,
        usb_config,
        config_descriptor,
        bos_descriptor,
        &mut [],
        control_buf,
    );

    let mut class = CdcAcmClass::new(&mut builder, state, 64);
    let mut usb = builder.build();

    let serial_loop = async {
        let mut buf = [0u8; 64];

        let mut parser = SumpParser::new();

        loop{
            class.wait_connection().await;
            info!("Connected");
            parser.reset();
            loop{
                match class.read_packet(&mut buf).await {
                    Ok(n) => {
                        info!("Received {} bytes", n);
                        for &byte in &buf[..n]{
                            if parser.process_byte(byte, &mut class).await{
                                let mut dummy_sample = [0u8; 64];
                                for i in (0..dummy_sample.len()).step_by(2){
                                    let pattern = if (i/2) % 2 == 0 {0xAA} else {0x55};
                                    dummy_sample[i]=pattern;
                                    dummy_sample[i+1]=pattern;
                                }
                                let _ = class.write_packet(&dummy_sample).await;
                            }
                        }
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