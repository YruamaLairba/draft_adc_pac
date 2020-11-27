#![no_std]
#![no_main]

// pick a panicking behavior
use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics

// use panic_abort as _; // requires nightly
// use panic_itm as _; // logs messages over ITM; requires ITM support
// use panic_semihosting as _; // logs messages to the host stderr; requires a debugger

use crate::hal::{pac, prelude::*, stm32};

use core::cell::Cell;
use core::ptr;
use cortex_m::interrupt::Mutex;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32f4::stm32f411::{interrupt, NVIC};
use stm32f4xx_hal as hal;

static G_ADC_DATA: Mutex<Cell<Option<u16>>> = Mutex::new(Cell::new(None));

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let device = stm32::Peripherals::take().unwrap();
    let gpioa = device.GPIOA.split();
    let rcc = device.RCC.constrain();
    let _clocks = rcc.cfgr.sysclk(16.mhz()).pclk2(1.mhz()).freeze();

    let _pa0 = gpioa.pa0.into_analog();
    //power up the adc
    unsafe {
        let rcc = &(*pac::RCC::ptr());
        rcc.apb2enr.modify(|r, w| w.bits(r.bits() | (1 << 8)));
    }
    //adc prescaler /8
    device
        .ADC_COMMON
        .ccr
        .modify(|_, w| w.adcpre().bits(0b0000_0011));
    let adc = device.ADC1;
    //continuous mode and enable adc
    adc.cr2.modify(|_, w| w.cont().continuous().adon().enabled());
    //sequence length = 1
    adc.sqr1.modify(|_, w| w.l().bits(0b0000_0000));
    //Use channel 0
    unsafe {
        adc.sqr3.modify(|_, w| w.sq1().bits(0b0000_0000));
    }
    //end of conversion interupt
    adc.cr1.modify(|_, w| w.eocie().bit(true));
    //reset end of conversion flag
    adc.cr2.modify(|_, w| w.eocs().bit(false));
    //start conversion
    adc.cr2.modify(|_, w| w.swstart().set_bit());

    unsafe { NVIC::unmask(stm32f4::stm32f411::Interrupt::ADC) };

    loop {
        let data = cortex_m::interrupt::free(|cs| G_ADC_DATA.borrow(cs).replace(None));
        if let Some(data) = data {
            rprintln!("{}", data);
            adc.cr2.write(|w| w.adon().disabled());
        }
    }
}

#[interrupt]
fn ADC() {
    let adc = unsafe { ptr::read_volatile(pac::ADC1::ptr()) };
    let sr_read = adc.sr.read();
    if sr_read.ovr().bit_is_set() {
        rprintln!("ADC OVR");
    }
    if sr_read.eoc().bit_is_set() {
        rprintln!("ADC EOC {}",adc.dr.read().bits() );
    }
    if sr_read.jeoc().bit_is_set() {
        rprintln!("ADC JEOC");
    }
    if sr_read.awd().bit_is_set() {
        rprintln!("ADC AWD");
    }
    adc.sr.write(|w| w.eoc().clear_bit());
    adc.sr.write(|w| w.strt().clear_bit());
    adc.cr2.write(|w| w.adon().disabled());
    adc.cr2.write(|w| w.swstart().clear_bit());

    cortex_m::interrupt::free(|cs| {
        G_ADC_DATA
            .borrow(cs)
            .replace(Some(adc.dr.read().bits() as _))
    });
}
