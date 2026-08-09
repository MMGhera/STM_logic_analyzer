use core::fmt::write;
use defmt::info;
use embassy_usb::class::cdc_acm::CdcAcmClass;
use embassy_usb::driver::Driver;

//short commands:
pub const CMD_RESET: u8 = 0x00;
pub const CMD_RUN: u8 = 0x01;
pub const CMD_ID: u8 = 0x02;
pub const CMD_METADATA: u8 = 0x04;//not found on wiki
pub const CMD_XON: u8 = 0x11;
pub const CMD_XOFF: u8 = 0x13;

//long commands:
pub const CMD_SET_DIV: u8 = 0x80;
pub const CMD_SET_READ_COUNT: u8 = 0x81;
pub const CMD_SET_FLAGS: u8 = 0x82;

#[derive(Debug)]
pub struct SumpConfig{
    pub divider: u32,
    pub read_count: u32,
    pub delay_count: u32,
    pub flags: u32,
}

impl Default for SumpConfig{
    fn default() -> Self {
        Self{
            divider: 0,
            read_count: 1024,
            delay_count: 0,
            flags: 0,
        }
    }
}

pub struct SumpParser{
    pub config: SumpConfig,
    cmd_buffer: [u8; 5],
    cmd_idx: usize,
}

impl SumpParser{
    pub fn new() -> Self{
        Self{
            config: SumpConfig::default(),
            cmd_buffer: [0u8; 5],
            cmd_idx: 0,
        }
    }

    pub fn reset(&mut self){
        self.cmd_idx = 0;
        self.config = SumpConfig::default();
    }

    async fn send_metadata<D: Driver<'static>>(&self, class: &mut CdcAcmClass<'static, D>){ //right now static...
        const METADATA: &[u8] = &[0x01, b'S', b'T', b'M', b'3', b'2', b'H', b'7', b'S', 0x00,
        0x02, b'0', b'.', b'1', 0x00,
        0x20, 0x05, 0xF5, 0xE1, 0x00,  //100MHz
        0x21, 0x00, 0x01, 0x00, 0x00, //64kb
        0x23, 0x10,
        0x00];
        class.write_packet(METADATA).await;
    }

    pub async fn process_byte<D: Driver<'static>>(&mut self, byte: u8, class: &mut CdcAcmClass<'static, D>) -> bool {
        if self.cmd_idx == 0{
            match byte {
                CMD_RESET => self.reset(),
                CMD_ID => {
                    let _ = class.write_packet(b"1ALS").await;
                },
                CMD_METADATA => self.send_metadata(class).await,
                CMD_RUN => return true,
                CMD_SET_DIV | CMD_SET_READ_COUNT | CMD_SET_FLAGS => {
                    self.cmd_buffer[0] = byte;
                    self.cmd_idx = 1;
                },
                _ => {},
            }
        }
        else{
            self.cmd_buffer[self.cmd_idx] = byte;
            self.cmd_idx += 1;
            if self.cmd_idx == 5{
                let parsed = u32::from_le_bytes([self.cmd_buffer[1], self.cmd_buffer[2], self.cmd_buffer[3], self.cmd_buffer[4]]);
                match self.cmd_buffer[0]{
                    CMD_SET_DIV => self.config.divider = parsed,
                    CMD_SET_READ_COUNT => self.config.read_count = (parsed & 0xFFFF) + 1,
                    CMD_SET_FLAGS => self.config.flags = parsed,
                    _ => {},
                }
                self.cmd_idx = 0;
            }
        }

        false
    }
}