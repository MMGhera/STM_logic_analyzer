//capture.rs
use static_cell::StaticCell;
use embassy_stm32::pac;
use core::slice;
use defmt::info;

const MAX_SAMPLES: usize = 16384;
static SAMPLE_BUF: StaticCell<[u16; MAX_SAMPLES]> = StaticCell::new();

pub fn init_gpioe(){
    pac::RCC.ahb4enr().modify(|w| w.set_gpioeen(true));
    pac::GPIOE.moder().write(|w| w.0 = 0x0000_0000);
    pac::GPIOE.ospeedr().write(|w| w.0 = 0xFFFF_FFFF);
    pac::GPIOE.pupdr().write(|w| w.0 = 0xAAAA_AAAA);
}

pub fn get_gpioe_idr_address() -> *const u16{
    pac::GPIOE.idr().as_ptr() as *const u16
}

