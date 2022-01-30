// $ cargo rb serial
// Serial rx/tx using DMA
#![no_main]
#![no_std]

use f103_rtic as _; // global logger + panicking-behavior + memory layout

#[rtic::app(device = stm32f1xx_hal::pac, peripherals = true, dispatchers = [EXTI1])]
mod app {
    use stm32f1xx_hal::{
        adc,
        pac,
        prelude::*,
        // dma::{
        //     dma1::{C4, C5},
        //     Event, RxDma, Transfer, TxDma, R, W,
        // },
        // pac::USART1,
        // prelude::*,
        // serial::{Config, Rx, Serial, Tx},
    };

    #[shared]
    struct Shared {}

    #[local]
    struct Local {}

    #[init()]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let p = ctx.device;
        let rcc = p.RCC.constrain();
        let mut flash = p.FLASH.constrain();

        // Configure ADC clocks
        // Default value is the slowest possible ADC clock: PCLK2 / 8. Meanwhile ADC
        // clock is configurable. So its frequency may be tweaked to meet certain
        // practical needs. User specified value is be approximated using supported
        // prescaler values 2/4/6/8.
        let clocks = rcc.cfgr.adcclk(2.mhz()).freeze(&mut flash.acr);
        defmt::info!("adc freq: {}", clocks.adcclk().0);

        // Setup ADC
        let mut adc1 = adc::Adc::adc1(p.ADC1, clocks);

        let mut adc2 = adc::Adc::adc2(p.ADC2, clocks);

        // Setup GPIOB
        let mut gpiob = p.GPIOB.split();

        // Configure pb0, pb1 as an analog input
        let mut ch0 = gpiob.pb0.into_analog(&mut gpiob.crl);

        let mut ch1 = gpiob.pb1.into_analog(&mut gpiob.crl);

        loop {
            let data: u16 = adc1.read(&mut ch0).unwrap();
            defmt::info!("adc1: {}", data);

            // let data1: u16 = adc2.read(&mut ch1).unwrap();
            // defmt::info!("adc2: {}", data1);
            cortex_m::asm::delay(1_000_000);
        }

        (Shared {}, Local {}, init::Monotonics())
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {}
    }
}
