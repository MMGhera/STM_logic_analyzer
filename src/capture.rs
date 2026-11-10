//capture.rs
use static_cell::StaticCell;
use embassy_stm32::pac;
use core::slice;
use defmt::info;

const MAX_SAMPLES: usize = 16384; //2^14 for some reason???
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

pub fn cast_u16_to_u8_slice(samples: &[u16]) -> &[u8]{
    let ptr = samples.as_ptr() as *const u8;
    let len = samples.len()*2;
    unsafe{slice::from_raw_parts(ptr, len)}
}

fn enable_cycle_counter(){
    unsafe{
        let demcr = 0xE000_EDFC as *mut u32;
        core::ptr::write_volatile(demcr, core::ptr::read_volatile(demcr) | (1 << 24)); //TRCENA
        core::ptr::write_volatile(0xE000_1FB0 as *mut u32, 0xC5AC_CE55); //unlock DWT (M7)
        let ctrl = 0xE000_1000 as *mut u32;
        core::ptr::write_volatile(ctrl, core::ptr::read_volatile(ctrl) | 1); //CYCCNTENA
    }
}

#[inline(always)]
fn cyccnt() -> u32{
    unsafe{core::ptr::read_volatile(0xE000_1004 as *const u32)}
}

pub struct CaptureEngine{
    buffer: &'static mut [u16; MAX_SAMPLES],
}

impl CaptureEngine{
    pub fn new() ->Self{
        init_gpioe();
        enable_cycle_counter();
        let buf = SAMPLE_BUF.init([0u16; MAX_SAMPLES]);
        Self{buffer:buf}
    }

    pub async fn run_capture(&mut self, requested_samples: u32, divider: u32) -> &[u16]{
        let count = (requested_samples as usize).min(MAX_SAMPLES);

        let sample_rate_hz = 100_000_000 / (divider+1);
        info!("Starting capture: {} samples at {} Hz", count, sample_rate_hz);

        let idr = get_gpioe_idr_address();
        let period = 4 * (divider + 1); //400MHz/100MHz = 4 per SUMP tick
        let mut deadline = cyccnt().wrapping_add(period);
        for s in self.buffer[..count].iter_mut(){
            while (cyccnt().wrapping_sub(deadline) as i32) < 0 {}
            *s = unsafe{core::ptr::read_volatile(idr)};
            deadline = deadline.wrapping_add(period);
        }
        self.buffer[..count].reverse(); //sends newest sample first

        &self.buffer[..count]
    }
}