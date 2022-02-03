// DEFMT_LOG=info cargo rrb midi_raw

#![no_std]
#![no_main]

use f103_rtic as _; // global logger + panicking-behavior + memory layout

mod midi {
    use usb_device::class_prelude::*;
    use usb_device::Result;

    pub const USB_CLASS_AUDIO: u8 = 0x01;

    pub struct MidiClass<'a, B: UsbBus> {
        audio_if: InterfaceNumber,
        midi_if: InterfaceNumber,
        out_ep: EndpointOut<'a, B>,
        in_ep: EndpointIn<'a, B>,
    }

    impl<B: UsbBus> MidiClass<'_, B> {
        pub fn new(alloc: &UsbBusAllocator<B>) -> MidiClass<'_, B> {
            MidiClass {
                audio_if: alloc.interface(),
                midi_if: alloc.interface(),
                out_ep: alloc.bulk(64),
                in_ep: alloc.bulk(64),
            }
        }

        pub fn note_off(&self, chan: u8, key: u8, vel: u8) -> Result<usize> {
            // I have no idea why the "virtual cable" must be number 0 and not one of the jack IDs
            // but only 0 seemed to work
            self.in_ep
                .write(&[0x08, 0x80 | (chan & 0x0f), key & 0x7f, vel & 0x7f])
        }

        pub fn note_on(&self, chan: u8, key: u8, vel: u8) -> Result<usize> {
            self.in_ep
                .write(&[0x09, 0x90 | (chan & 0x0f), key & 0x7f, vel & 0x7f])
        }

        pub fn ctrl(&self, chan: u8, ctrl_nr: u8, ctrl_data: u8) -> Result<usize> {
            self.in_ep
                .write(&[0x0b, 0xb0 | (chan & 0x0f), ctrl_nr & 0x7f, ctrl_data & 0x7f])
        }
    }

    impl<B: UsbBus> UsbClass<B> for MidiClass<'_, B> {
        fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> Result<()> {
            writer.interface(self.audio_if, 0x01, 0x01, 0x00)?; // Interface 0
            writer.write(
                0x24, /* interface decscriptor */
                &[0x01, 0x00, 0x01, 0x09, 0x00, 0x01, 0x01],
            )?; // CS Interface (audio)

            writer.interface(self.midi_if, 0x01, 0x03, 0x00)?; // Interface 1
            writer.write(0x24, &[0x01, 0x00, 0x01, 0x2e, 0x00])?; // CS Interface (midi)

            writer.write(
                0x24, /* Descriptor */
                &[
                    0x02, /* midi jack in */
                    0x01, /* embedded */
                    0x01, /* id */
                    0x00, /* unused */
                ],
            )?; // IN Jack 1 (emb)
            writer.write(
                0x24, /* Descriptor */
                &[
                    0x03, /* midi jack out */
                    // 0x01 /* embedded */,
                    0x02, /* external */
                    0x02, /* id */
                    // 0x01 /* nr of input pins */,
                    0x00, /* nr of input pins */
                    // 0x01 /* id of entity this this pin is connected to */,
                    // 0x01 /* output pin number number of the entity to which this pin is connected */,
                    0x00,
                ], /* unused */
            )?; // OUT Jack 2 (emb)

            writer.endpoint(&self.out_ep)?;
            writer.write(0x25, &[0x01, 0x01, 0x01])?; // CS EP IN Jack

            writer.endpoint(&self.in_ep)?;
            writer.write(0x25, &[0x01, 0x01, 0x02])?; // CS EP OUT Jack

            Ok(())
        }
    }
}

#[rtic::app(device = stm32f1xx_hal::pac, peripherals = true, dispatchers = [EXTI1])]
mod app {
    use cortex_m::asm::delay;
    // use nb::block;
    //  use stm32f103xx_usb::UsbBus;
    use super::midi;
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
    use stm32f1xx_hal::{prelude::*, stm32, timer::Timer};
    use usb_device::prelude::*;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {}

    #[init()]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        defmt::info!("init");

        let p = ctx.device;

        let rcc = p.RCC.constrain();
        let mut flash = p.FLASH.constrain();

        let clocks = rcc
            .cfgr
            .use_hse(8.mhz())
            .sysclk(48.mhz())
            .pclk1(24.mhz())
            .adcclk(16.mhz())
            .freeze(&mut flash.acr);

        assert!(clocks.usbclk_valid(), "usb clocks not valid");

        // Setup ADC
        let mut adc1 = adc::Adc::adc1(p.ADC1, clocks);

        // Setup GPIOB
        let mut gpiob = p.GPIOB.split();

        // Configure pb0, pb1 as an analog input
        let mut ch0 = gpiob.pb0.into_analog(&mut gpiob.crl);

        // Setup LED
        let mut gpioc = p.GPIOC.split();
        // Configure the on-board LED (PC13, green)
        let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
        led.set_low(); // Turn on

        // Setup USB
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

        let mut midi = midi::MidiClass::new(&usb_bus);

        let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27de))
            .manufacturer("Wha wha wha")
            .product("MIDI Wha")
            .serial_number("0.1.0")
            .device_class(midi::USB_CLASS_AUDIO)
            .build();

        let mut old_data_msb = 0;

        let mut send = false;

        const NR_SAMPLES: u32 = 4;
        loop {
            // defmt::trace!("poll");
            while usb_dev.poll(&mut [&mut midi]) {}

            if usb_dev.state() == UsbDeviceState::Configured {
                let mut data_acc: u32 = 0;

                // take a sequence of samples and compute the average (adc noise)
                for _ in 0..NR_SAMPLES {
                    let sample: u16 = adc1.read(&mut ch0).unwrap();
                    data_acc += sample as u32;
                }
                let data_raw = data_acc / NR_SAMPLES;

                let data_msb: u8 = (data_raw >> 5) as u8;

                // defmt::trace!("raw: {}, msb {}", data_raw, data_msb);

                // we initiate `send` mode only if new data differs by 2
                // we stay in `send` = true as long as new data differs
                let diff: i8 = old_data_msb as i8 - data_msb as i8;
                if diff.abs() > 1 || send {
                    defmt::debug!("old msb {}, msb {}", old_data_msb, data_msb);
                    send = old_data_msb != data_msb;

                    old_data_msb = data_msb;
                    match midi.ctrl(0, 1, data_msb) {
                        Ok(_) => {}
                        Err(UsbError::BufferOverflow) => {
                            defmt::info!("overflow");
                        }
                        Err(UsbError::WouldBlock) => {
                            // skipping
                            defmt::info!("busy");
                        }
                        _ => {
                            defmt::info!("other error");
                        }
                    }
                }
            }
        }

        (Shared {}, Local {}, init::Monotonics())
    }
}
