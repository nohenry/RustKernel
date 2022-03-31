use crate::util;

pub struct SerialPort {
    address: u16,
    enabled: bool,
}

impl SerialPort {
    pub fn new(address: u16) -> Self {
        unsafe {
            // util::out8(address + 1, 0x00); // Disable interrupts
            // util::out8(address + 3, 0x80); // Enable DLAB
            // util::out8(address + 0, 0x01); // Set divisor to 3 (low byte) 38400 baud
            // util::out8(address + 1, 0x00); //                  (high byte) 38400 baud
            // util::out8(address + 3, 0x03); // 8 bits, no parity, one stop bit
            //                                // util::out8(address + 2, 0xC7); // Enable FIFO, clear them, with 14-byte threshold
            // util::out8(address + 4, 0x03); // IRQs enabled, RTS/DSR set
            //                                // util::out8(address + 4, 0x1E); // Set in loopback mode, test the serial chip
            //                                // core::ptr::write_volatile(0x14141414141414 as *mut u64, 0);
            //                                // util::out8(address + 0, 0x69); // Testing
            util::out8(address + 1, 0x00); // Disable interrupts
            util::out8(address + 3, 0x80); // Enable DLAB
            util::out8(address + 0, 0x01); // Set divisor to 3 (low byte) 38400 baud
            util::out8(address + 1, 0x00); //                  (high byte) 38400 baud
            util::out8(address + 3, 0x03); // 8 bits, no parity, one stop bit
                                           // util::out8(address + 2, 0xC7); // Enable FIFO, clear them, with 14-byte threshold
            util::out8(address + 2, 0xC7); // IRQs enabled, RTS/DSR set
            util::out8(address + 4, 0x0F); // IRQs enabled, RTS/DSR set
                                           // util::out8(address + 4, 0x1E); // Set in loopback mode, test the serial chip
                                           // core::ptr::write_volatile(0x14141414141414 as *mut u64, 0);
                                           // util::out8(address + 0, 0x69); // Testing
            util::out8(address + 1, 0x01); // Disable interrupts
            SerialPort {
                address,
                enabled: true,
            }
            // if util::in8(address) != 0x69 {
            //     SerialPort {
            //         address,
            //         enabled: false,
            //     }
            // } else {
            //     util::out8(address + 4, 0x0F); // disable loopback, IRQs, OUT 1 and 2
            //     SerialPort {
            //         address,
            //         enabled: true,
            //     }
            // }
        }
    }

    pub fn from(address: u16) -> Self {
        SerialPort {
            address,
            enabled: true,
        }
    }

    fn has_data(&self) -> bool {
        unsafe { (util::in8(self.address + 5) & 1) == 0 }
    }

    fn can_write(&self) -> bool {
        unsafe { (util::in8(self.address + 5) & 0x20) == 0 }
    }

    pub fn read_byte(&self) -> Option<u8> {
        unsafe {
            if self.has_data() && self.enabled {
                Some(util::in8(self.address))
            } else {
                None
            }
        }
    }

    pub fn write_byte(&self, value: u8) {
        unsafe {
            while self.can_write() {}
            match value {
                b'\n' => util::out8(self.address, b'\r'),
                _ => (),
            }
            util::out8(self.address, value);
        }
    }

    pub fn write(&self, value: &[u8]) {
        if self.enabled {
            for &b in value {
                self.write_byte(b);
            }
        }
    }
}

impl core::fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write(s.as_bytes());
        Ok(())
    }
}
