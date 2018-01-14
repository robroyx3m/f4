//! Reading and page writing to an external EEPROM Microchip 24LC64 using I2C3

// #![deny(warnings)]
#![feature(proc_macro)]
#![no_std]

extern crate cortex_m;
extern crate cortex_m_debug;
extern crate cortex_m_rtfm as rtfm;
#[macro_use]
extern crate f4;
extern crate stm32f40x;

use core::mem::transmute;
use f4::I2c;
use f4::led::{self, LED};
use rtfm::{app, Threshold};
use core::result::Result;
use stm32f40x::I2C3;
use f4::clock;

const EEPROM_PAGE_SIZE: usize = 32;
const RX_BUFFER_SIZE: usize = core::mem::size_of::<u32>();

/// Errors for reading EEPROM
#[derive(Debug)]
pub enum Error {
    /// Invalid eeprom memory address
    InvalidMemory,
}

app! {
    device: f4::stm32f40x,

    idle: {
        resources: [I2C3, ITM],
    },
}

fn init(p: init::Peripherals) {
    clock::set_84_mhz(&p.RCC, &p.FLASH);

    led::init(p.GPIOA, p.RCC);

    // Init the I2C peripheral
    let i2c = I2c(p.I2C3);
    i2c.init(p.GPIOA, p.GPIOB, p.RCC);
    i2c.enable();
}

// 24LC64 sequential read. See datasheet DS21189F.
fn read_eeprom(
    i2c: &I2c<I2C3>,
    mem_addr: u16,
    rx_buffer: &mut [u8; RX_BUFFER_SIZE],
) -> Result<(), Error> {
    // Check if we are addressing inside eeprom memory space
    if mem_addr > 0x1fff - RX_BUFFER_SIZE as u16 {
        return Err(Error::InvalidMemory);
    }
    // Write device address and memory address to set eeprom internal cursor
    while i2c.start(0xa0).is_err() {}
    while i2c.write((mem_addr >> 8) as u8).is_err() {}
    while i2c.write(mem_addr as u8).is_err() {}

    // Read incoming bytes and ACK them
    while i2c.start(0xa1).is_err() {}
    for i in 0..RX_BUFFER_SIZE {
        rx_buffer[i] = loop {
            if i == RX_BUFFER_SIZE - 1 {
                // Do not ACK the last byte received and send STOP
                if let Ok(byte) = i2c.read_nack() {
                    break byte;
                }
            } else {
                // ACK the byte after receiving
                if let Ok(byte) = i2c.read_ack() {
                    break byte;
                }
            }
        }
    }
    Ok(())
}
// 24LC64 page write. See datasheet DS21189F.
fn write_eeprom(
    i2c: &I2c<I2C3>,
    mem_addr: u16,
    tx_buffer: &[u8; EEPROM_PAGE_SIZE],
) -> Result<(), Error> {
    // Check if we are addressing inside eeprom memory space and address is page aligned
    if mem_addr > 0x1fff - EEPROM_PAGE_SIZE as u16 || mem_addr % EEPROM_PAGE_SIZE as u16 != 0 {
        return Err(Error::InvalidMemory);
    }
    // Write device address and memory address to set eeprom internal cursor
    while i2c.start(0xa0).is_err() {}
    while i2c.write((mem_addr >> 8) as u8).is_err() {}
    while i2c.write(mem_addr as u8).is_err() {}

    // Write data
    for i in 0..EEPROM_PAGE_SIZE {
        while i2c.write(tx_buffer[i]).is_err() {}
    }
    while i2c.stop().is_err() {}
    Ok(())
}

// Test writing and reading the eeprom
fn idle(_t: &mut Threshold, r: idle::Resources) -> ! {
    let i2c = I2c(r.I2C3);

    // Write in 32 byte pages (max for this eeprom)
    let mut mem_addr = 0x0000;
    let mut page: [u8; EEPROM_PAGE_SIZE] = [0; EEPROM_PAGE_SIZE];
    let mut page_index = 0;
    for (_data_index, data) in DATA.iter().enumerate() {
        // Store u32 into u8 page buffer
        let data_bytes: [u8; 4] = unsafe { transmute(data.to_le()) };
        page[page_index..(page_index + 4)].clone_from_slice(&data_bytes);
        page_index += 4;
        if page_index >= page.len() {
            page_index = 0;
            // We have filled the page, now write it.
            write_eeprom(&i2c, mem_addr, &page).unwrap();
            mem_addr += EEPROM_PAGE_SIZE as u16;
        }
    }
    // The data might not be 32 byte page aligned, so
    let remainder_len = DATA.len() * 4 % EEPROM_PAGE_SIZE;
    if remainder_len > 0 {
        // Just send the whole page...
        write_eeprom(&i2c, mem_addr, &page).unwrap();
    }

    // Read back to check that it worked
    let mut rx: [u8; RX_BUFFER_SIZE] = [0; RX_BUFFER_SIZE];
    let mut status_ok = true;
    for (data_index, written_data) in DATA.iter().enumerate() {
        let mem_addr: u16 = data_index as u16 * 4;
        match read_eeprom(&i2c, mem_addr, &mut rx) {
            Err(_) => {}
            Ok(_) => {
                // Read the byte array as
                let read_data: u32 = unsafe { core::ptr::read(rx.as_ptr() as *const _) };
                let _w = *written_data;
                if read_data != *written_data {
                    status_ok = false;
                    break;
                }
            }
        }
    }
    if status_ok {
        LED.on();
    } else {
        LED.off();
    }
    loop {
        rtfm::bkpt()
    }
}

// The 24LC64 has space for 64000/32 integers
const DATA: [u32; 2000] = [
    0xee431a62, 0xcc0f04fe, 0x1d82d37a, 0x8fbd60f2, 0x799a9518, 0x809b2394, 0x8ef86021, 0xace538fe,
    0x6b5b9772, 0x645b120d, 0xba759e27, 0x3945c39b, 0x5b9dd4e0, 0x17d8f77e, 0xc865e590, 0x4b360ce2,
    0xca43e905, 0x201be683, 0x3e3a975d, 0xcfd5e5aa, 0x1c660879, 0x9fed4fcc, 0x65a4fb6c, 0x8c29d52a,
    0xff865d8f, 0x8c470024, 0x14bb4703, 0x3a07899e, 0xe993c997, 0xb7eaec3e, 0xfde811d8, 0x07aad58d,
    0xec007943, 0x5598a360, 0x3f7d5701, 0x6dd02a31, 0x2fe40c61, 0xee7dd438, 0x8c429d24, 0x94d0e6c8,
    0xa93671bb, 0x0f131f00, 0x0dec6c38, 0x80ca08d3, 0x381e48da, 0xb6849a0e, 0x332aadca, 0xf317c179,
    0xfda9ed49, 0xb4b832ca, 0x1bc1d749, 0xc05b4691, 0x46d73a21, 0xd1c6c889, 0x05631b49, 0x0d74cd8c,
    0xc575662e, 0xc9552eec, 0x09b3474a, 0x2d6a67ef, 0xfd5500f4, 0xc121f4b1, 0x4c5c3c0f, 0xbe3ef236,
    0x0957ff29, 0xe15329ef, 0x4458e3d3, 0xaa5df069, 0x35504eb7, 0x3dc68427, 0x765bbece, 0x4f283bdc,
    0x2c44cc31, 0x0d6de1f9, 0x42187b4d, 0x74b5bc86, 0x27a93590, 0xcc2eccc6, 0x087856dc, 0xf7225738,
    0x3cd59d4a, 0xb5ed12b4, 0xafc262e3, 0x9373278e, 0x77c85f6b, 0x53436194, 0x21abb9de, 0x53847055,
    0xc99351bc, 0x897823f7, 0x38bb0797, 0xca7c7760, 0x1f1cfa53, 0x5e895e5b, 0xbfe9fa51, 0x95c74188,
    0xf9a0ce00, 0x1f2b3a87, 0xb4e71133, 0x29141ef5, 0x7cc70a4e, 0x389f184e, 0x8862d656, 0x3a76ef92,
    0xf362ee56, 0x2d296e46, 0xdebfd758, 0xeb3e4d5f, 0x18b7ddbc, 0x881a7d76, 0x6393bc9e, 0xf01df955,
    0x35a36482, 0xe3932700, 0x98129dbd, 0x53eae53e, 0xbb91bb42, 0xb5420443, 0x05faccbd, 0xae75c6fc,
    0x483881a6, 0xfed19182, 0x11ac9e86, 0x226ed648, 0x52da9fb4, 0xe0c078ff, 0x404b40da, 0x6baebcb7,
    0x86c2ca6b, 0x056e9c44, 0x5cd3af74, 0x19a33818, 0x08e80dd2, 0xa770ea29, 0xea190938, 0x543e3294,
    0xb1a69805, 0x66d9e502, 0xad85397e, 0xd73d16bf, 0x934b4434, 0xb892c5fd, 0x8f20ada8, 0xbae125b9,
    0x4aca2386, 0xe022e885, 0x40c691ec, 0xc3afe8c7, 0xf2f6830a, 0x8ff0f9ec, 0x9ed2ccb0, 0xd4c93746,
    0xd615a976, 0x15e34015, 0x3fd04828, 0xc8955a50, 0x1fed85b0, 0xb9b98945, 0x6ba3e8bd, 0x4bc1000b,
    0x699e4b71, 0xe4bb4e8b, 0xa5784763, 0x3cf6170c, 0x00038a07, 0x07b08bf3, 0xbc7432c9, 0xae53bb84,
    0xdf7ec4ba, 0x1be2e9bd, 0x5f550152, 0xbc35d227, 0xd88a4950, 0x9c79f72d, 0xd3dc2316, 0x02dec19f,
    0x7d08c98f, 0xa49933e9, 0xad30e356, 0xfc120342, 0x7b104ce1, 0x45f9579a, 0xa442c26b, 0x7028579c,
    0x679a9668, 0x2c487e88, 0xea6eb466, 0x63c7d9ca, 0xd3dc1215, 0xd60cb8f1, 0x722f7ed2, 0x3ca982d9,
    0x1d28cbe0, 0x8e8f1d4e, 0x6c6b3c92, 0xd7a5b4e4, 0x3584e916, 0x4ec17725, 0xe60566cb, 0x468a5079,
    0x07398d14, 0xab4e12f8, 0xcd481ee7, 0x35778f16, 0xa296426e, 0x6d02611b, 0xf4576800, 0x9bf01796,
    0x7db1a251, 0x4aa68106, 0x6568baf5, 0x346b8ad7, 0xb417108d, 0xfb6c4e9e, 0x13b5ae7c, 0x984547b4,
    0x75bf6403, 0x2e999e00, 0x9fa2a4a1, 0xa0a5f226, 0x135d8d50, 0x751247a9, 0x54f931bb, 0x66a5a717,
    0xc5787600, 0x47e426f0, 0xc9e83271, 0xd47abd3b, 0xea0f6597, 0x8e6d10ab, 0x4c4d76db, 0xb2fd1a73,
    0x1dcf6541, 0xeec0a471, 0xc5543d02, 0xbca85d5c, 0x11c879f9, 0xa45ef959, 0x50bb6fdb, 0xde64e1f3,
    0xbce9722c, 0x7689e095, 0x3366cdf2, 0x3b53c854, 0xb5aa6bc3, 0x536f8f22, 0x3c413ad9, 0xa1837593,
    0x57909a38, 0x89e64e94, 0x6f4ecdf3, 0xe4a432e7, 0x6a4235fe, 0x5c4a23c0, 0x1290f5ed, 0xa2f6258b,
    0x80e9d7ff, 0xe9e69535, 0xc31346b0, 0x16cece8e, 0xe8997cf3, 0x169a5ac4, 0x4b25c99e, 0x84c5fa94,
    0xb124dfc6, 0xb709c6ad, 0x710c50c3, 0xde4fb85b, 0xdcd5801d, 0x88a289e4, 0x26f5aa66, 0x17ad7fb8,
    0xc86b45fa, 0x032b5211, 0x77595a35, 0xeb5e1e70, 0x8c775c25, 0x2396501c, 0x05471ecc, 0xd1564098,
    0x352075ce, 0xe6a91e3e, 0xbdb7859c, 0x66f16870, 0xf94521e8, 0x0c8ac4d3, 0x5c26079b, 0xa6bbc2f3,
    0x47d2eda1, 0x179378c6, 0xe85cbd99, 0x4eddb567, 0x15642f97, 0xe6c9ad03, 0x2b7019ab, 0x343748ec,
    0xa2cded2f, 0x94e179d9, 0xa77e220f, 0xec3595a8, 0xe655a306, 0x56dd9639, 0x733e3a0c, 0x1a6f65dd,
    0xd4d32383, 0xc3a548a8, 0x83d34c7b, 0xff723756, 0x0db28454, 0xc4560a61, 0x9df87560, 0xfd51fc54,
    0xb310145b, 0x88ee427c, 0xf2d65082, 0xeca4357f, 0x6d49de22, 0xa81c9f3c, 0xa1ff5a19, 0x8f077f94,
    0x33eee389, 0x19153272, 0xb40fa365, 0xf5c7ad88, 0x9dbb7691, 0x16818228, 0x7ca29d54, 0x81a2f8af,
    0x108d89f0, 0xf023def8, 0x4099d59d, 0xcb1eb7c3, 0x6a0b7fa3, 0x29964339, 0x03aa52ea, 0xc0fd7fbb,
    0xb32b2e41, 0x616d2af7, 0x237a7bb3, 0xebd894f5, 0xd9c1e9dc, 0x5c44871d, 0x4c87e81c, 0x064bd1c4,
    0x407413f5, 0x10289209, 0x1bf797e0, 0x244914e0, 0x06ab7734, 0xe8bd4ce0, 0xcd5c1c2b, 0xf2118fe6,
    0x24a5697c, 0xfaec7faf, 0xcf959a29, 0xbcd70926, 0xc6c9ea5f, 0x32a4f242, 0xd1af7bc4, 0xd1495a89,
    0xfab1bb15, 0x21c25871, 0xd5c51916, 0x6d8066c4, 0xa6c20649, 0x81f67663, 0xe533da88, 0xd64d5283,
    0x97df9770, 0x6c7c9af9, 0x86316252, 0x8ae5ff2c, 0x3f394a81, 0x5713dfc3, 0xe441ca7d, 0x53c7d9db,
    0xa03aba62, 0x8b73b561, 0xc3d25987, 0xc2c97567, 0x93d5a3a7, 0xe165f395, 0xf12ce3ec, 0x094431f2,
    0x63af4ea7, 0xedbc6894, 0xff56e975, 0xb5c27ef7, 0x30279a3b, 0x84751e38, 0x08e6d029, 0x1d07d308,
    0x03f90398, 0x45daa09d, 0x522b4ac5, 0xe5ecc747, 0xa3359ea2, 0x07d5b1ab, 0x24c059a4, 0xebae0b1f,
    0x193e2870, 0xd0be27a5, 0x28cfef58, 0xfda2ce33, 0xc67808e9, 0x0c4420ec, 0x6f467d4c, 0xdd559637,
    0x5d15cc41, 0x58385aa9, 0x02aafe96, 0x4e4a6653, 0x47a005ee, 0x031c9c02, 0x8b43409f, 0x5ee912b1,
    0x181a90d1, 0xd8a9c370, 0x5a57f3d2, 0x8b7c4236, 0xec05ef36, 0xdfb2d3b1, 0xcb52a5ad, 0x64211157,
    0xfe05f502, 0x72f22250, 0x2e4f0fa3, 0x9f316e3d, 0x3b923fda, 0xbc5002a7, 0x1c2b8ebb, 0xdca43c3c,
    0xb4fbab0e, 0x34245245, 0x157f20a0, 0xd6a34925, 0x8cc2a0ca, 0xf544897b, 0x5999f1cf, 0x29479bf6,
    0xd5978c31, 0xa40f17b0, 0x1d875344, 0x4c323022, 0x25a5423a, 0xdb9f7719, 0x4720d724, 0x1d597af4,
    0x58274967, 0x569dc620, 0x14dd032d, 0x2a6e9a45, 0xb2a8be64, 0x5c11e5c5, 0x61720024, 0xadb9c92b,
    0xddbb6d82, 0x4969445f, 0xae00b139, 0x6c0b8831, 0x9d1af707, 0x9e7c843d, 0x68a467d7, 0x55e5a564,
    0xf09e2593, 0xa583041f, 0xfa3cd4c1, 0x9e160610, 0x7fcd2d22, 0x07d66d97, 0x2d1a5c9f, 0x8ad51344,
    0x0ff953ac, 0x1b4e4787, 0xd90926e3, 0xef6812ff, 0xd8e7d1e4, 0x45a440bb, 0xa3db2a6c, 0xb1778a59,
    0xb018a277, 0x264b3887, 0xa7397d43, 0xb886fe9d, 0x845b0354, 0x6be87312, 0x9e8997a3, 0x87007cba,
    0xe470da4b, 0x4f02a77c, 0xaf86624a, 0xa12cbf3b, 0x756ff79f, 0xe13d4af5, 0xeee65d71, 0xde0a6ecc,
    0xd836432e, 0xb03790b0, 0xec399848, 0xcc2aae79, 0x75ee7511, 0x3d723632, 0x74ed5af2, 0x7970e5fc,
    0xf4d67b8a, 0x918350b7, 0x53343d75, 0x3cdd7c10, 0xfb604e66, 0x7f00628e, 0x2178ffd6, 0xb8ea8438,
    0xf67c680a, 0xd0d0ff27, 0xf8ee88fe, 0x48af84a9, 0xd3e9f797, 0x7bbcb946, 0x9ea99306, 0x9e02efd1,
    0x846c2b0e, 0xeef3c4c6, 0x060a460e, 0xd3c56a1a, 0xfb26ac51, 0x167338ae, 0xa0df7180, 0x51a41500,
    0xfa959aae, 0xdf326416, 0x1d776ce7, 0x1db68405, 0xfc595337, 0x6f82a0e7, 0x1314a093, 0x3f941a6b,
    0x04c46270, 0x9abf0933, 0xb6e9c538, 0x44bfd903, 0x102584c0, 0x9193f895, 0xa308c9b1, 0x6361f4ba,
    0xafa00274, 0x1948bb8f, 0xc06aebd6, 0xe3f32e4e, 0x22063266, 0xeaf5fcc3, 0x809af938, 0x48beb66f,
    0x8b62c658, 0xd81a8569, 0xa3d933ae, 0x606e0234, 0xad6e2c5b, 0x7102ea3f, 0xad2e995b, 0xcbc4cea3,
    0xef5afac9, 0x22b4da46, 0x49911bc0, 0x10838a85, 0x664b7b96, 0x190ba552, 0x70b4c6ec, 0xd1bf1e38,
    0x7082449c, 0xd5f76584, 0x9493bc26, 0xe63e658e, 0x6b6adffa, 0x24e214c2, 0x3c42663d, 0x5f89a46e,
    0x1bf27151, 0x0a55a051, 0x808b4d3b, 0x11ff3c47, 0x7bd94acd, 0x9a6ab620, 0xb1d5122a, 0xb23a844d,
    0xa9074390, 0x24ea05ba, 0x58adb9a1, 0xd517bdec, 0x955153bd, 0xa3ebd076, 0xf733d24f, 0x04522d1f,
    0xf4f0f3ec, 0x5c4b001b, 0x52866173, 0xb669e616, 0xd1455f09, 0xcfd395fe, 0x03c0b406, 0xdc474ad5,
    0xa953f7c3, 0xa180830d, 0x7ed28798, 0x31e5a5b7, 0xcec2a253, 0x998f5a50, 0xb84032f2, 0x88574e38,
    0x5a5f756e, 0xf19c08ed, 0x34c29306, 0xfca6a527, 0x177696ff, 0xc1b113de, 0x04ffe941, 0xe4e6af77,
    0xe226e997, 0xa443ed06, 0x7713014f, 0x0bc1fab8, 0x19326217, 0x4216681b, 0xdefea599, 0x1a541263,
    0xf4355a40, 0x13b8cbb9, 0xbd93c12a, 0xe1613075, 0x3bc54613, 0xaae3b99a, 0x11be04cb, 0x36f9bdc7,
    0x9197dd2c, 0x52b47f7b, 0x6639186d, 0x2102f94d, 0x025fa7a0, 0xd7f08ee1, 0xf8d4f571, 0x619f6822,
    0x74ef9a74, 0x20ade6d0, 0x008e97b2, 0x2b11919b, 0x6d8aaf31, 0x09b0ee50, 0x50604131, 0x49ffdc20,
    0x10c79fa5, 0x408d446a, 0x0cf8fa13, 0xee1246bb, 0xc9e565be, 0xbe2cc2bd, 0x70b6aeba, 0x0d2ebb78,
    0x3e0c387b, 0x6dc27225, 0xe92ad146, 0xe823baf0, 0x85c4622e, 0xf460f58a, 0xed120f14, 0xa433f7b3,
    0x860bf676, 0x12f79101, 0xb7ee0623, 0x88a2d486, 0x1b1ac588, 0x7c496393, 0xaf45ac3a, 0x6dca1538,
    0xc5ca9e41, 0xe0a6753c, 0x36158a86, 0x6b3bda5f, 0xcd309c97, 0x86ad35bc, 0x625a897b, 0x58775f7c,
    0x217d6be7, 0x5b2d3cac, 0xd7859367, 0x29325bd5, 0x05ace5b4, 0x2f728130, 0xfc0e2503, 0xc5e08ffe,
    0xe7b18b8c, 0x204ed5ed, 0x569d7a33, 0x270ac14e, 0x23dcce15, 0xa71a02ee, 0x32d041c7, 0x6305efce,
    0x9575e729, 0x52df6ba5, 0x71a65657, 0x83255c2b, 0x3c337f39, 0x34e9f623, 0xe721ef19, 0xa8f1cdbf,
    0xc2281a5f, 0xeb6b1f9c, 0x88598fc4, 0xd153ec69, 0xb07a56a6, 0x3ad2a9a2, 0x6a7a0111, 0xce0d2e2a,
    0x88fd0f76, 0xba632250, 0xca3a555e, 0x9a802610, 0x3915b0cc, 0x56f1c934, 0x97d3b859, 0x39a65936,
    0x2ab9e5ad, 0x19942dd2, 0x72cb0474, 0x09d4bbd6, 0x2d2bd4aa, 0x7f0dfb9b, 0x8fe389ab, 0x6012b848,
    0x6a5ea0be, 0x156e73c6, 0x7dd8c6fa, 0xa466e203, 0x5fb3b517, 0x97411a55, 0xcdd13836, 0x00c386bd,
    0xed771f10, 0xd80abc71, 0xecbc29ec, 0x66688f2d, 0x9004c26f, 0x43aec072, 0xe19a3b03, 0x05575243,
    0x09aba70b, 0xeb13bf14, 0x0e049f72, 0xe8a26ce8, 0x8988fd44, 0x89764005, 0x2a3d9117, 0x90c0e06e,
    0xecaca076, 0x6455e2f0, 0x6df28a8b, 0x246dd086, 0xb0e6a34e, 0x1ba77b36, 0xa04b7d81, 0x2cc8c425,
    0x34674168, 0xadb66685, 0x6a9fa9e2, 0x1fb7056e, 0x107fa93a, 0x37bdae32, 0xfcd12de2, 0xd1f4c54c,
    0x4914a643, 0x952a0c2e, 0xcd00ad6e, 0xd4b09cc2, 0x09ba6a46, 0x837be363, 0xe5cbe314, 0xcdf81076,
    0x3f2a9b80, 0xdcc4991d, 0xf4469881, 0x96326781, 0x78467ad0, 0x0a01c11e, 0x98a757b1, 0xfcb0e543,
    0x10db4009, 0x0394e598, 0xe3d21d40, 0xfa1fa344, 0x27b4a2fa, 0x80aa4183, 0x507fea51, 0x9c2671d9,
    0x0574b12c, 0xf992f29d, 0x1adc0faa, 0xeedc3970, 0xc24a8677, 0xcdd4e32e, 0x57e4f076, 0xc7511ad6,
    0x84e5f674, 0xe88df801, 0x75dc563a, 0xcb45fe0b, 0x637472e6, 0xc8ccc509, 0x1291d4a3, 0x2b99680c,
    0xf78a9483, 0x452a7dea, 0x918f8120, 0x739dadee, 0x0cec85fa, 0x574df1ca, 0xf1059223, 0xcc995ac6,
    0x03aa6fe1, 0x08957005, 0xcc59ec12, 0x3731c687, 0x89377578, 0xce82c1b7, 0xda19584a, 0x8147a793,
    0x9c07bf83, 0x32af009c, 0xe84fc659, 0xc7e73cb1, 0xaabbfb33, 0xd1ee44a5, 0x78ba87a6, 0x703a77dc,
    0xa2384170, 0x08458f41, 0x276da94b, 0xa21f1e0e, 0xc0679da9, 0x2fbfda3c, 0x6172c504, 0x95cebe3c,
    0x2c469950, 0xc34fd628, 0xd0949f4e, 0x8439e904, 0x1e8daefa, 0xb888b2e2, 0x3aad56d7, 0x34c4eae1,
    0x14944116, 0x25d52040, 0xfe5858e3, 0xf87c53a0, 0x349c7b3d, 0xd4a19f80, 0x57bb41e8, 0x93bb70c2,
    0x8ebe737e, 0xab1164fd, 0x1ca73be8, 0x6b771253, 0x663696cc, 0x3f8b1133, 0x038a1ee3, 0x8a320f56,
    0x91dd868e, 0xbf1389cc, 0xd5457e93, 0x71257ae3, 0x6c4d66d5, 0x605aa792, 0xb8247975, 0x9210f917,
    0x37e63a1f, 0x10833f33, 0xabc533cb, 0x2a579f6d, 0x9291986f, 0x4c9b2799, 0xec4fcd7c, 0xcb824c38,
    0x1a3b797e, 0xbaf7ce96, 0xb674cbf1, 0x1919cdff, 0x25449241, 0xcd18294c, 0x6cfb200b, 0x31f11b7a,
    0x37f9355e, 0x146bd1c3, 0x529f8eda, 0x0848235d, 0xde20060a, 0xeaf48beb, 0xb89c9e9c, 0xac65e089,
    0xf1eae2c5, 0x9a3495ac, 0x734ded0c, 0xb65f0e8e, 0xe53bf240, 0x134ef292, 0x1d88b877, 0x2d443e5a,
    0xbf2a5154, 0xc2a22659, 0xc7e22c3f, 0xdf012622, 0xc329ae74, 0xdc91f776, 0x9c0c0103, 0xa91ade9d,
    0x802c8b2e, 0x20332491, 0x181fb1c2, 0x5219f8c9, 0x6bfce342, 0x5d43142b, 0x3ccb438d, 0xe314f202,
    0x8892475f, 0x5e1e8bed, 0x0c2de304, 0xbf999b2a, 0x30f02ec7, 0x49947055, 0xe0a05c1b, 0x2e1fad6d,
    0xaa15a81f, 0x8e5cca74, 0x936a8a5b, 0x311ca27e, 0x2bf7131c, 0x6eb2d473, 0x549b70f5, 0x79959d8f,
    0x6bad9e0c, 0x53081546, 0xcedfe2f8, 0xc13e979c, 0x65188dae, 0xa1ebdbcf, 0xa3c70fd3, 0x43f03568,
    0xf07aa2a2, 0x471a28cb, 0xf4b0e650, 0xb1da6dea, 0xa3be20e6, 0x5ea18550, 0x1a368722, 0x0f7b4e78,
    0xf5fa8795, 0x1c7d4f56, 0x32415102, 0x549ab0c1, 0x759fd154, 0x81a83c89, 0x43950a3a, 0x6e48e803,
    0x8a568e09, 0x8457a90e, 0x420df67f, 0xaf5d9ae0, 0xba6d1de5, 0xa242fcd5, 0xd234a52f, 0x1ca8b313,
    0x43ff3e48, 0xd972e874, 0x7e53cf67, 0x5d72596c, 0x9889dee6, 0xfc982cb7, 0xeaf9c9a6, 0x943dac7c,
    0xdb152580, 0x1f031283, 0x96e7a693, 0xdd8039a0, 0x835ec9da, 0xe8fec2ae, 0xd7100766, 0xa089b985,
    0xab9aaf2d, 0x3c331980, 0xaffe959f, 0x919506e4, 0xd726f66d, 0xb0f09174, 0x2668ddd8, 0x55cc42c6,
    0x90fc802c, 0x03892cc3, 0x66f45fb6, 0xf64f27af, 0x7a961df9, 0xb352b864, 0xf5809335, 0xd3c49d07,
    0x473771d0, 0x86af6a28, 0x99eeaa40, 0x12cf1aa9, 0x9938dfe3, 0x7d50e79b, 0x5472ad59, 0x4ff60e42,
    0x8c571a45, 0x3b69a06b, 0x9487b30e, 0xea07e0a0, 0x1cdf371f, 0xbad969bd, 0x8b57001b, 0x3d668d5b,
    0x1a27e5ac, 0x0ce4724a, 0xffa119b9, 0x8ea124b8, 0x18e13223, 0xa4243ca3, 0xb6d58c7b, 0x522b1e58,
    0x4da5cd00, 0x3331759d, 0xc9666362, 0xe5ce3fc2, 0xaacf404d, 0xe152c704, 0xc35bcc47, 0xa298a23c,
    0x5d48d60a, 0x460540ac, 0xbf859a85, 0x48a7212d, 0x5b01d5c6, 0x083702c1, 0x64ba918d, 0x32b7abc7,
    0xadf13b8c, 0x47e32250, 0x31e32e76, 0xc6f4380a, 0x30dd39df, 0x4db4de28, 0x1070fb32, 0x6e162e52,
    0xf8d17a87, 0x189eea6e, 0xf4703d04, 0xefd531d3, 0x325c3cd3, 0x067e5498, 0xbe95ca74, 0xe636553c,
    0x00656fa8, 0xf37d7c38, 0x2c4b0dd1, 0xea731aae, 0xef77e3ea, 0x62fd5beb, 0xd60decbc, 0xcaa694ad,
    0x0a185b7d, 0xfa8ad677, 0x64ccf113, 0xe093ce39, 0x51d85a4d, 0x21fe397c, 0xa5fe516d, 0x0c75d9c8,
    0xf0a3d46a, 0xda65805d, 0xfdd22fb2, 0xfa8ba940, 0x47bb4a89, 0x6ea16952, 0x667c6c4d, 0xd1123d85,
    0x0e6bc000, 0x1fc52017, 0x64bf7f05, 0x0c551b47, 0x93746f3c, 0x8bb19e86, 0x76c87eb0, 0x61ac6213,
    0x12f3a630, 0x7470e922, 0xe4f89ff4, 0xeeebc886, 0x9d98f391, 0x9816b586, 0x60db7f33, 0xfe4ab2f3,
    0x28f5740b, 0x43af423e, 0xd8aa49d8, 0x12719c82, 0x974ead05, 0x8595c8c9, 0x7d0955b5, 0x30d3db07,
    0xbe5cdb64, 0x68e619f8, 0x4879b676, 0x9e0f89bb, 0x7bbc29e9, 0xf81b464b, 0xf2c1abe9, 0xefac01c9,
    0xe796a0d9, 0x05135d7f, 0x4d846f0c, 0x8bc3b279, 0x469ef683, 0x40a8b4fb, 0x41f78b14, 0xd24024b0,
    0x486d1815, 0x6b786413, 0xf1fb1cf9, 0xbf3b340e, 0x319476c9, 0x593fadbe, 0x071375c6, 0xe07655c6,
    0x10b2635a, 0xb3926eaf, 0xd3858611, 0x5023c5d4, 0x98209f11, 0x591da76a, 0x202f8165, 0x615e4360,
    0x6e53727d, 0x3c3dbf5b, 0xcda99c15, 0xba150bf1, 0xd7f62dc3, 0xe5ef9e2b, 0x7dd3cb1e, 0x43f8db30,
    0xb1cfdfe9, 0xc1c0aaea, 0x458d4fea, 0xa061aff4, 0x71811490, 0xb2064f5e, 0xe87d60ad, 0xb8966e4f,
    0xc4e37f66, 0x4cd35905, 0x4311403a, 0x57ecec3b, 0x429630e8, 0x607f1aef, 0x05f695b2, 0x8f5898b5,
    0xdb4f5b1f, 0x03cfc298, 0xa08a12f5, 0x4a905f0c, 0x0a6f543d, 0xb74a26af, 0x27becbd1, 0x4e920790,
    0xe3eff565, 0x95d7470e, 0x270ab130, 0x393e1d59, 0x636982f0, 0xc4a37db3, 0x0c23d107, 0xed4b2510,
    0xc8dd57fe, 0x7229b3fd, 0x7570c13b, 0x9ca41945, 0x16eda678, 0x0f26829d, 0x2d1e33f5, 0xb40cdb11,
    0x0598756b, 0x88e2e1d3, 0xebd29377, 0xb671f430, 0x1de4d072, 0xc98697e9, 0xe2e103e8, 0x5305bc85,
    0xfb204c48, 0xc857c92b, 0xdac7dc94, 0x786d2d90, 0xc992a123, 0xc1c2a621, 0xebaced59, 0xf2a3435a,
    0x807fad8e, 0x58d97e63, 0xfb3728e0, 0x1b043ce1, 0x9935e935, 0x38d97e1e, 0x61a1372d, 0xb5a16b8d,
    0x2837aa3e, 0x25571679, 0x437f2de3, 0xa51ca2b6, 0x27fd2760, 0xd7522e59, 0x632971ac, 0xa954ef48,
    0x01cd69ed, 0x2ca1c71d, 0x98e2d62c, 0x462711d6, 0xca541267, 0xc46945ff, 0x8ce309fe, 0xf9106615,
    0xe0eb3712, 0x5a5c9b15, 0x84e30da1, 0x131be4c2, 0x0487f7a5, 0x587fc3ae, 0x4c38d9c9, 0x3473bdfb,
    0xffd2cf8e, 0x51395806, 0x3289c736, 0x84b24991, 0xaccb8794, 0x28c7e2e2, 0x64b7cd17, 0x565e7e81,
    0x8e2faa9b, 0x913a3cd3, 0x13273a4d, 0xc7b20c95, 0x23b15f1f, 0x5dd51265, 0x6657900a, 0xa089a900,
    0xae7b77cb, 0x3fd0dfdb, 0xc2470bc4, 0x10269b2b, 0x56d0ca69, 0xcfd871c5, 0x78a54930, 0x5da6ec6c,
    0x9a5e67db, 0xe4e68a06, 0x336a7e98, 0xa5445389, 0x73f8a39a, 0x40c23d43, 0x93cae9b2, 0x2c0f905c,
    0x8812726c, 0x32caf31e, 0x7846137c, 0xa5592430, 0xc8550615, 0x891f9e5a, 0xfec2c143, 0x6dfbfb52,
    0xcaa9024a, 0x869de67c, 0x6fc90910, 0xb70dc9f3, 0x85f80d9c, 0x01505a0e, 0x50b244a5, 0x6bc9e926,
    0x781da5e2, 0x9859856e, 0x56e5522a, 0xc3bca942, 0x605b4701, 0x7e4b44ab, 0x0dea0d2a, 0xb64a80be,
    0x8308ddc0, 0xa0fe1789, 0x8f7c67ab, 0x920b7faf, 0xd13c39a9, 0x01e83bea, 0x3e8ed7bc, 0x74559589,
    0xef9c6e41, 0xeac3cf33, 0xfdcaf255, 0xb5162e1a, 0xc1ef9f68, 0x6958b710, 0x89652610, 0xe1998800,
    0xca352b2b, 0xec6893c7, 0x7e32bb26, 0xe5385ee2, 0xa9c9f042, 0x6af20ef4, 0xf65d20d9, 0xd1473e14,
    0x931f8372, 0x470106c0, 0xab07a45d, 0x59ce018c, 0xa0fe7715, 0x47d51275, 0x9362c135, 0xe3464082,
    0xb5f4a328, 0x631e9ddc, 0x1310282d, 0xe33cad60, 0x53237fd1, 0x6284d45c, 0xc24d1845, 0x4f5784e7,
    0x0e394da1, 0x41996cdd, 0xcc3108ff, 0xe1b62ec6, 0x5542edf1, 0x1fc9dc4d, 0x842f38ee, 0x999302cd,
    0xdb0103b1, 0x6f61fbb7, 0xc0d7c477, 0xa9b35d2c, 0xebc95dbc, 0x68626ab1, 0x21cf6647, 0xf5088c8a,
    0x963c51a6, 0x97b14382, 0xa6d28997, 0xcbc79d0e, 0xfab0ede1, 0xf894dedb, 0x80427332, 0x88b11d5b,
    0x6e26d052, 0xe89ff87f, 0xd81d797c, 0xc7ebe525, 0xf69b186b, 0xea346d2f, 0x7a263c77, 0xb7ee89f4,
    0x0031c006, 0x4aac855e, 0x0ff2d8c4, 0x4ad6c81d, 0x02775aa9, 0xe86b9130, 0x3a63945a, 0xdaee0d50,
    0x41219a72, 0xd958ecf3, 0xe3ec1f8f, 0x4728ac73, 0xbead6648, 0x410ea171, 0x2ed03e35, 0x191d29ee,
    0xc384aef2, 0x67b5e98a, 0x6f107361, 0x557f7267, 0xff811b8f, 0xaa46584b, 0xe4651696, 0x3f91ff5b,
    0x206a249a, 0x93a7fdc5, 0xf30fde3a, 0xcd2a3c87, 0xd592dd14, 0xb0fe57ad, 0xa05f424a, 0xc99cf8eb,
    0x0a903973, 0x356b50d3, 0xb11aab0e, 0x7f2038ba, 0xc9f5e9a5, 0x346895a5, 0x2aca4834, 0x80090dbe,
    0x4ed30121, 0x48378e52, 0x8f924d96, 0xc8d016d6, 0x5626bb14, 0xd8636c6d, 0xeb20a537, 0x633b217f,
    0xe9ed5212, 0x51ca05de, 0x72c6704a, 0x96705328, 0xcede667f, 0x59a652f9, 0xc30112c1, 0x546aaf3b,
    0xb26c4950, 0x28ab90a9, 0xf6b5429a, 0xcbd2489e, 0x11d8314f, 0x4e3e126e, 0xdc308549, 0x97cb61dd,
    0x8d4231d6, 0xe61405d3, 0x79d7d82a, 0xc86b883c, 0x4aee281a, 0x9ca08c27, 0x4568d302, 0x0ca606dc,
    0x7f0d7337, 0x42ec2586, 0xc70f1720, 0xd3b8144a, 0x809058ff, 0xad9dbf6a, 0x61a4ae97, 0xe658d680,
    0xc0ea6862, 0x66a98abc, 0xec73ce78, 0x8d965fb1, 0x787081e7, 0xb3ede5e9, 0x204dbc79, 0xa6d47854,
    0x9633558f, 0x17f60dea, 0x43fa73e2, 0x6a310ca1, 0xee83e98e, 0x4547ec71, 0x1fae5d64, 0x737161d2,
    0x436d8c11, 0x63871520, 0x88fd1f88, 0x1d110b17, 0xb94a952e, 0x95f53936, 0xea4222a8, 0x2564531c,
    0x8ce24a1e, 0xb431ea4e, 0x04f5a391, 0x3c671463, 0xee35c1f4, 0x26a2960d, 0x88d0075f, 0xdea9476d,
    0x58dd0eec, 0x8cbc3c28, 0xf41f79a3, 0xe9312ee8, 0x39061be4, 0xaa42a7e0, 0x8a74a175, 0x0399fbf9,
    0x5237923d, 0x88ddb42c, 0xf9c029e2, 0xfb509fd2, 0x151278c8, 0xb8d1275d, 0x489fa2e1, 0xa6f73ac2,
    0xa54bf05b, 0xf200c32b, 0x336c7869, 0x4e4d97b2, 0xb62b1d4e, 0xf04eb8d4, 0x234a7589, 0xab6ab6a6,
    0x63a68589, 0x1757dfbb, 0x2003807f, 0xe860edc7, 0x83a0676d, 0x1b287922, 0x59302e7a, 0xc17ea080,
    0x16782dc3, 0xfd2a50b0, 0xaa791bd5, 0xf18e1828, 0x9292ac03, 0x46c61ac2, 0xdc3db7fa, 0xd688ba43,
    0xb00df0f0, 0xcc4280f8, 0xbe52d450, 0x7f3e9b3b, 0x1d3f9a83, 0xb4b49e9d, 0x0234af59, 0x06fc59bc,
    0x4409b1dc, 0x48c400bd, 0xaabad074, 0x3423dd1b, 0x7636bbc7, 0xb509e116, 0xcea2079f, 0x05fba769,
    0xaf612390, 0xf9916040, 0xcc204d1a, 0x7788dce2, 0x1ecf85a3, 0x7fdeedd4, 0x46dc96eb, 0xc882f353,
    0x6e59ec4f, 0xbd02f305, 0x4b0400cb, 0xfaef7919, 0x60bb4b71, 0xe3f385b4, 0x1fea9de8, 0xa6682f96,
    0xed99f241, 0x440974a4, 0xd8eda2ec, 0xf063e6ac, 0xe6930c75, 0x03ddff93, 0x0cb3e77d, 0xb58c5828,
    0x9cfc9d5d, 0xd65bc881, 0x6746b11a, 0x72b5c486, 0x16c80b1e, 0x9a116206, 0xba138772, 0x539ce39a,
    0xe4f86953, 0x1ea62536, 0x4f74ffdd, 0x5eaa14f3, 0x1de4a8d8, 0x5abbc324, 0x99116aea, 0x1ab08279,
    0xc9da311b, 0x0f9240f1, 0x57da96e9, 0x88a60bc7, 0x2e2c6d8b, 0x7ade4efc, 0x53c10201, 0x08ffa2c8,
    0x8f8b92b5, 0x3b14fe53, 0xf369fbf1, 0xe23e1382, 0x1923f7c5, 0xa02ea872, 0xfcd602cc, 0xacc342f9,
    0xcd3cfa86, 0x464fa94b, 0x4d69a877, 0x76df27b5, 0xe8498d9a, 0xb04fa6aa, 0xeeefa0e9, 0x43e7e0ec,
    0xb017dd30, 0x08acb21a, 0x0427d314, 0x4f08e7a5, 0x9054e1a7, 0x9f4160dc, 0xd0e63287, 0xeba31c7a,
    0x2bf1c49e, 0xa11fc0eb, 0x09c334c8, 0xbdf4f7bd, 0xbc89a531, 0xa19a32b5, 0x58b07bd6, 0x5e2c79f2,
    0x7b3106bf, 0x2143e0d9, 0xae641bab, 0xcabba46a, 0x08229b43, 0xbc9c451b, 0x7da89bab, 0xe2cf6d8f,
    0xd940ac0b, 0x9efbba9e, 0x6ebda081, 0x9d5e8b1f, 0x1837d7c2, 0x2ec6202f, 0xea73f7d1, 0x7c63da16,
    0xc2c8a59c, 0x95d6a609, 0x08c3766e, 0xb6a394dc, 0x2ae77abe, 0xde919245, 0x52905959, 0x1c2fd6f9,
    0xb03d3a5c, 0xcd11df05, 0x5667d610, 0x71b01238, 0x0abec90b, 0x5459aa33, 0xf4dfa42c, 0x4ac917c1,
    0x540b2736, 0xd8a8c861, 0xe9a5b5a0, 0xad2e2533, 0x86513d0c, 0x4b262dad, 0x3e0d2747, 0xa9b64bd2,
    0xafdbe351, 0xa2fd3312, 0xd44fdf56, 0xcf826cd8, 0xa287faf7, 0x1cd97a5c, 0x5c3d50b4, 0x54e15f6e,
    0x9e87edea, 0xe53fc675, 0x80f517c4, 0xff6ac1d4, 0xafc63535, 0xace9aec8, 0x36ed5e98, 0x78c8bece,
    0x66b1c924, 0xb9099088, 0xabdbc3f4, 0x183f34d2, 0x78cf3868, 0xe514c1d5, 0xf236738b, 0x47da64ed,
    0x3e89fcfa, 0x67950cfd, 0x98144146, 0x001f9a5e, 0xb93a76b2, 0x9382b426, 0xb036a1bd, 0xf7e7e9ce,
    0x15334525, 0x5f6e3b26, 0xf5522c91, 0x9e713df9, 0xe3569350, 0x9fd03e59, 0x309cdd7d, 0xd5000924,
    0x5bcd9bd0, 0xf4934e6a, 0xaedef066, 0x9ee5d1c7, 0x74cade16, 0xd4cc17bf, 0x79298f86, 0x44f9d0ae,
    0xfb6750c3, 0x99b692b7, 0x3228af33, 0x20bf6f1c, 0x63698395, 0x367606cf, 0x847461f7, 0xae13b51a,
    0x56bd25a0, 0x9364ba46, 0x3fdad049, 0xb51dd965, 0x458bb52c, 0x5b3ad13a, 0xf04694dd, 0x92255fb0,
    0xdb77bd3c, 0x418cb5cd, 0xc6700b0b, 0xc3428853, 0x8f4abcb3, 0xe75c5088, 0x3684f012, 0xc020abfa,
    0x3e93f9de, 0xc957573f, 0xdb22f966, 0xa43fbb9d, 0x612b9f35, 0x9b8790d9, 0x730cece3, 0x7d6c3a6f,
    0x171fb251, 0x2566e951, 0x35cc0a29, 0xf60e391e, 0x18ae53fe, 0x40dffb9f, 0xad33b508, 0xdcb2be48,
    0x8503963f, 0x69584427, 0x1c2a5be3, 0x68346a51, 0x0de183c5, 0xeb6ea0b6, 0x0b613246, 0x13a31238,
    0x0f553201, 0xf1a7ab9c, 0x47666e3e, 0x91bc7da8, 0xf24e3fa9, 0x972fa6ba, 0xa32004b1, 0x235d9b45,
    0x49e3440e, 0x3895981a, 0x22c3aa69, 0x22c1f56d, 0x5c9873af, 0xcb376197, 0xd4549844, 0xd33b377e,
    0xd2c2a221, 0xc9f3e987, 0x5bb82af8, 0x71c91c72, 0x6998e9c6, 0x8a2e81ed, 0x60fa71ff, 0xc90b6850,
    0xfc861a5a, 0x4eabf913, 0xce37bb2b, 0xf4aa89bc, 0x345fcfbf, 0xdc196837, 0xef9c768c, 0x96ebf845,
    0xe8683b46, 0x6211dd47, 0xea1f8895, 0x009da62d, 0x92821de9, 0xd53b0ca9, 0xc049d991, 0x9c9cd11f,
    0xe979b751, 0x54f054cb, 0x6fee24c4, 0xb85a65f2, 0xcafc5dec, 0x1afe16f4, 0x8c5e5591, 0xe3b18bb6,
    0xc6d52c4a, 0x489612bc, 0x536c5de0, 0xb39b7fec, 0x27fb983f, 0xe85e5601, 0x91655573, 0x7daccf25,
    0xb1df981c, 0x366772d2, 0x004e441c, 0x2f539817, 0x936c4776, 0x2ed3cdd0, 0x635674a8, 0xc1fd11b5,
    0x6b321cbe, 0xa3c036fb, 0x9a5a055d, 0x6cab9a12, 0x5332cd5e, 0x9ee6b267, 0x75c8d837, 0xbd719258,
    0x6f8f0147, 0xf411031d, 0xcfb54d4e, 0x36937db0, 0xedcab6fc, 0xd27879c5, 0xa796b1a0, 0x74c62dc9,
    0x5a2ea14a, 0x046ce755, 0xd93ff7c2, 0x34f02320, 0x6081099e, 0xee68be07, 0x5113004c, 0xdfe83d00,
    0x3d6f3ec3, 0x4dc8c575, 0x63232a96, 0xb6bb9957, 0x62fb0469, 0xc28b62f2, 0xbe28ac2e, 0xa2a55ad2,
    0x3b7432f3, 0xc0d61867, 0x6a713e15, 0xa44cacba, 0x8e6ee4ae, 0x43d11400, 0x20b99e7b, 0xefb2e03e,
    0xa02c0542, 0x81e61e4e, 0xb48f9fb1, 0xfd50b8fe, 0x45374c98, 0x9d5d6e9f, 0x03cb6cfc, 0x501cd531,
    0xc4ea0a00, 0xb9701506, 0xf33a685a, 0x62548908, 0xdbfd865a, 0x684ba1e8, 0x757d0a11, 0x42f09ebd,
    0xb11bfa56, 0xbd42dcb1, 0xdc2a043b, 0x4c4e92cf, 0xd614cff1, 0xb65d71a4, 0xacd7805e, 0xbeb93b5b,
    0x1cd41dce, 0xc09d4411, 0x74e5225f, 0x6c46142d, 0x312a8b00, 0x5296f734, 0x6709fffc, 0x00da0050,
    0xfddc5621, 0x2742844c, 0x230b9ab4, 0x909e9478, 0x2e972708, 0x048d1bf9, 0xc1f5c856, 0x7b2c831f,
    0xf7627356, 0x005577a8, 0xbed60d3f, 0x1ab1f983, 0x0e987a30, 0xdedfba84, 0xcba4fdbe, 0x804367cd,
    0xc9583084, 0x484876bc, 0x59b36214, 0x894fe869, 0xbf2edc0d, 0x8a830e45, 0x173766e8, 0x1e82739e,
    0x2424e471, 0x697dc2c3, 0x873250cc, 0x6a0fb132, 0x48b6c945, 0x5b2e2253, 0x882b31fe, 0x34faaa1d,
    0xbf4efa63, 0x0f1b5473, 0x1d937d4e, 0x45b32626, 0xb24c77b5, 0x1df97c92, 0xccba9297, 0x9ae4ace1,
    0x9b245b46, 0x7f0b742f, 0xdb2eecc9, 0xf1e415af, 0x2ab26ab0, 0x2cc1f45a, 0x7e7695bf, 0xdde32036,
    0x75c7afe4, 0x92b07501, 0x6e2da8df, 0x4134ed1c, 0x3dfcf9e3, 0x85c568de, 0x5f2b2775, 0xac844ba5,
    0x7656e330, 0x6f81274a, 0xcda7f76b, 0x104697ba, 0x31d99919, 0x56c4fb95, 0x6d7a58b1, 0xbee0ad2a,
    0xb16fd4ab, 0x176c7f9d, 0x046e358f, 0x8700f31e, 0xd28e64c2, 0xd826f4bd, 0xc37e0da3, 0x1edc5ab2,
    0x7d7bc4d0, 0x5a624bc7, 0xf40726bc, 0x8d7b8a89, 0xc60ef4b9, 0x82e58bb3, 0x44e56944, 0x1153ed89,
    0x533354ad, 0x0441aeb0, 0xf6d4d86f, 0xab7c29dd, 0xbcda470f, 0xc306522b, 0x552ffe84, 0xbbc4996f,
    0xb59ecd23, 0xef9aaa26, 0x874fb31d, 0x95e77a2b, 0xf5d195a7, 0xed93d962, 0xf427e7c4, 0xbf036840,
    0xb83cfb72, 0x057cc7b2, 0x760730bc, 0x82a172ad, 0x928ce77b, 0xb1a0466c, 0xe2702002, 0x13999aec,
    0xc4a28489, 0x66d089c7, 0x0245d42d, 0xb65ba4e4, 0x46338ec7, 0x10477d13, 0xecf9f237, 0xfca0f08d,
    0xccce3e03, 0x8ccf8747, 0xe844860a, 0x102703f7, 0x1ae3752b, 0xa611f357, 0x42a79bdf, 0xe62e275c,
];