// $ cargo rb serial
// Serial rx/tx using DMA
#![no_main]
#![no_std]

use f103_rtic as _; // global logger + panicking-behavior + memory layout

#[rtic::app(device = stm32f1xx_hal::pac, peripherals = true, dispatchers = [EXTI1])]
mod app {
    use cortex_m::asm::delay;

    use stm32f1xx_hal::{
        adc,
        pac,
        prelude::*,
        usb::{Peripheral, UsbBus},
        // dma::{
        //     dma1::{C4, C5},
        //     Event, RxDma, Transfer, TxDma, R, W,
        // },
        // pac::USART1,
        // prelude::*,
        // serial::{Config, Rx, Serial, Tx}
    };
    use usb_device::prelude::*;
    use usbd_serial::{SerialPort, USB_CLASS_CDC};

    #[shared]
    struct Shared {}

    #[local]
    struct Local {}

    #[init()]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let p = ctx.device;
        let rcc = p.RCC.constrain();
        let mut flash = p.FLASH.constrain();

        let clocks = rcc
            .cfgr
            .use_hse(8.mhz())
            .sysclk(48.mhz())
            .pclk1(24.mhz())
            .adcclk(2.mhz())
            .freeze(&mut flash.acr);

        assert!(clocks.usbclk_valid());

        // Configure ADC clocks
        // Default value is the slowest possible ADC clock: PCLK2 / 8. Meanwhile ADC
        // clock is configurable. So its frequency may be tweaked to meet certain
        // practical needs. User specified value is be approximated using supported
        // prescaler values 2/4/6/8.
        defmt::info!("adc freq: {}", clocks.adcclk().0);

        // Setup ADC
        let mut adc1 = adc::Adc::adc1(p.ADC1, clocks);

        // Setup GPIOB
        let mut gpiob = p.GPIOB.split();

        // Configure pb0, pb1 as an analog input
        let mut ch0 = gpiob.pb0.into_analog(&mut gpiob.crl);

        // Configure the on-board LED (PC13, green)
        let mut gpioc = p.GPIOC.split();
        let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
        led.set_high(); // Turn off

        let mut gpioa = p.GPIOA.split();

        // BluePill board has a pull-up resistor on the D+ line.
        // Pull the D+ pin down to send a RESET condition to the USB bus.
        // This forced reset is needed only for development, without it host
        // will not reset your device when you upload new firmware.
        let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.crh);
        usb_dp.set_low();
        delay(clocks.sysclk().0 / 100);

        let usb = Peripheral {
            usb: p.USB,
            pin_dm: gpioa.pa11,
            pin_dp: usb_dp.into_floating_input(&mut gpioa.crh),
        };

        let usb_bus = UsbBus::new(usb);

        let mut serial = SerialPort::new(&usb_bus);

        let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
            .manufacturer("Fake company")
            .product("Serial port")
            .serial_number("TEST")
            .device_class(USB_CLASS_CDC)
            .build();

        // loop {
        //     let data: u16 = adc1.read(&mut ch0).unwrap();
        //     defmt::info!("adc1: {}", data);

        //     // let data1: u16 = adc2.read(&mut ch1).unwrap();
        //     // defmt::info!("adc2: {}", data1);
        //     cortex_m::asm::delay(1_000_000);
        // }

        loop {
            if !usb_dev.poll(&mut [&mut serial]) {
                continue;
            }

            let mut buf = [0u8; 64];

            match serial.read(&mut buf) {
                Ok(count) if count > 0 => {
                    led.set_low(); // Turn on

                    // Echo back in upper case
                    for c in buf[0..count].iter_mut() {
                        if 0x61 <= *c && *c <= 0x7a {
                            *c &= !0x20;
                        }
                    }

                    let mut write_offset = 0;
                    while write_offset < count {
                        match serial.write(&buf[write_offset..count]) {
                            Ok(len) if len > 0 => {
                                write_offset += len;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }

            led.set_high(); // Turn off
        }

        (Shared {}, Local {}, init::Monotonics())
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {}
    }
}
