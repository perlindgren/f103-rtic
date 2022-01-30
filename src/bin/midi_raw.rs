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
                    // 0x01 /* output pin number number of the entitiy to which this pin is connected */,
                    0x00,
                ], /* unsused */
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
            .freeze(&mut flash.acr);

        assert!(clocks.usbclk_valid(), "usb clocks not valid");

        let mut gpioa = p.GPIOA.split();
        let mut gpioc = p.GPIOC.split();

        // Configure the on-board LED (PC13, green)
        let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
        led.set_low(); // Turn on

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
            .manufacturer("Fake company")
            .product("MIDI Device")
            .serial_number("TEST")
            .device_class(midi::USB_CLASS_AUDIO)
            .build();

        // usb_dev.force_reset().expect("reset failed");

        // let mut timer = Timer::syst(cp.SYST, 1000.hz(), clocks);

        let notes = &[60, 62, 64, 65, 67, 69, 71, 72];
        let mut note_iter = notes.iter().cycle();
        let mut note = *note_iter.next().unwrap();

        let mut ms = 0;
        loop {
            // defmt::trace!("poll");
            while usb_dev.poll(&mut [&mut midi]) {}

            if usb_dev.state() == UsbDeviceState::Configured {
                // Excuse the super crude sequencer

                if ms == 200 {
                    if midi.note_on(0, note, 64).is_ok() {
                        defmt::trace!("note on");
                        led.set_low();
                        ms += 1;
                    }
                } else if ms == 400 {
                    if midi.note_off(0, note, 0).is_ok() {
                        defmt::trace!("note off");
                        led.set_high();
                        ms = 0;
                        note = *note_iter.next().unwrap();
                    }
                } else {
                    ms += 1;
                }
                delay(100_000);
            }
        }

        (Shared {}, Local {}, init::Monotonics())
    }
}
