// > DEFMT_LOG=info cargo rb hello_world
// ...
// 0 INFO  init
// └─ hello_world::app::init @ src/bin/hello_world.rs:33
// 1 INFO  idle
// └─ hello_world::app::idle @ src/bin/hello_world.rs:40
// or, using the most detailed "trace" log-level.
// > DEFMT_LOG=trace cargo rb hello_world
// ...
// 0 INFO  init
// └─ hello_world::app::init @ src/bin/hello_world.rs:33
// 1 TRACE init
// └─ hello_world::app::init @ src/bin/hello_world.rs:34
// 2 INFO  idle
// └─ hello_world::app::idle @ src/bin/hello_world.rs:40

#![no_main]
#![no_std]

use f103_rtic as _; // global logger + panicking-behavior + memory layout

#[rtic::app(device = stm32f1xx_hal::pac, peripherals = true, dispatchers = [EXTI1])]
mod app {

    #[shared]
    struct Shared {}

    #[local]
    struct Local {}

    #[init]
    fn init(_ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        defmt::info!("init");
        defmt::trace!("init");
        (Shared {}, Local {}, init::Monotonics())
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        defmt::info!("idle");
        // panic!("here");

        loop {}
    }
}

// You can uncomment the `panic` on line 41, to observe the tracing of
// panic messages.
