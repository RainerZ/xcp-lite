//----------------------------------------------------------------------------------------------
// Module daq_event

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use crate::{reg::RegistryMeasurement, xcp::*, RegistryDataType};

//----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
// XcpEvent

impl Xcp {
    /// Create a measurement event and a measurement variable directly associated to the event with memory offset 0
    pub fn create_measurement_object(&self, name: &'static str, data_type: RegistryDataType, x_dim: u16, y_dim: u16, comment: &'static str) -> XcpEvent {
        let event = self.create_event(name);
        if self
            .get_registry()
            .lock()
            .add_measurement(RegistryMeasurement::new(
                name, data_type, x_dim, y_dim, event, 0, // byte_offset
                0, 1.0, // factor
                0.0, // offset
                comment, "", // unit
                None,
            ))
            .is_err()
        {
            error!("Error: Measurement {} already exists", name);
        }
        event
    }
}

/// Create a single instance XCP event and register the given variable once, trigger the event
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_event_ref {

    ( $id:expr, $data_type: expr, $x_dim: expr, $comment:expr ) => {{
        lazy_static::lazy_static! {
            static ref XCP_EVENT__: XcpEvent = Xcp::get().create_measurement_object(stringify!($id), $data_type, $x_dim, 1, $comment);
        }
        XCP_EVENT__.trigger(&(*$id) as *const _ as *const u8, 0 );
    }};
    ( $id:expr, $data_type: expr, $x_dim: expr, $y_dim: expr, $comment:expr ) => {{
        lazy_static::lazy_static! {
            static ref XCP_EVENT__: XcpEvent = Xcp::get().create_measurement_object(stringify!($id), $data_type, $x_dim, $y_dim, $comment);
        }
        // @@@@ Unsafe - C library call which will dereference the raw pointer base
        unsafe { XCP_EVENT__.trigger_ext(&(*$id) as *const _ as *const u8); }
    }};
}

//----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
// DaqEvent

/// DaqEvent is a wrapper for XcpEvent which adds the capability to read variables from stack or adds an optional capture buffer to capture variable values
#[derive(Debug)]
pub struct DaqEvent<const N: usize> {
    event: XcpEvent,
    buffer_len: usize,
    pub buffer: [u8; N],
}

impl PartialEq for DaqEvent<0> {
    fn eq(&self, other: &Self) -> bool {
        self.event == other.event
    }
}

impl<const N: usize> DaqEvent<N> {
    /// Create a new DaqEvent with a given name and optional capture buffer
    pub fn new(name: &'static str) -> DaqEvent<N> {
        let xcp = Xcp::get();
        DaqEvent {
            event: xcp.create_event_ext(name, false, 0),
            buffer_len: 0,
            buffer: [0; N],
        }
    }

    /// Create a new DaqEvent from an existing XcpEvent
    pub fn new_from(xcp_event: &XcpEvent) -> DaqEvent<N> {
        DaqEvent {
            event: *xcp_event,
            buffer_len: 0,
            buffer: [0; N],
        }
    }

    fn get_xcp_event(&self) -> XcpEvent {
        self.event
    }

    /// Get the capacity of the capture buffer
    #[allow(clippy::unused_self)]
    pub fn get_capacity(&self) -> usize {
        N
    }

    /// Allocate space in the capture buffer
    pub fn allocate(&mut self, size: usize) -> i16 {
        trace!("Allocate DAQ buffer, size={}, len={}", size, self.buffer_len);
        let offset = self.buffer_len;
        assert!(offset + size <= self.buffer.len(), "DAQ buffer overflow");
        self.buffer_len += size;
        offset.try_into().expect("offset out of range")
    }

    /// Copy to the capture buffer     
    pub fn capture(&mut self, data: &[u8], offset: i16) {
        let offset = offset.try_into().expect("offset negative");
        self.buffer[offset..offset + data.len()].copy_from_slice(data);
    }

    /// Trigger for stack or capture buffer measurement with base pointer relative addressing
    pub fn trigger(&self) {
        let base: *const u8 = &self.buffer as *const u8;
        // @@@@ Unsafe - C library call which will dereference the raw pointer base
        unsafe {
            self.event.trigger_ext(base);
        }
    }

    /// Trigger for stack measurement with absolute addressing
    pub fn trigger_abs(&self) {
        self.event.trigger_abs();
    }

    /// Associate a variable to this DaqEvent, allocate space in the capture buffer and register it
    #[allow(clippy::too_many_arguments)]
    pub fn add_capture(
        &mut self,
        name: &'static str,
        size: usize,
        datatype: RegistryDataType,
        x_dim: u16,
        y_dim: u16,
        factor: f64,
        offset: f64,
        unit: &'static str,
        comment: &'static str,
        annotation: Option<String>,
    ) -> i16 {
        let event_offset: i16 = self.allocate(size); // Address offset (signed) relative to event memory context (XCP_ADDR_EXT_DYN)
        trace!("Allocate DAQ buffer for {}, TLS OFFSET = {} {:?} and register measurement", name, event_offset, datatype);
        let event = self.get_xcp_event();
        if Xcp::get()
            .get_registry()
            .lock()
            .add_measurement(RegistryMeasurement::new(
                name,
                datatype,
                x_dim,
                y_dim,
                event,
                event_offset,
                0u64,
                factor,
                offset,
                comment,
                unit,
                annotation,
            ))
            .is_err()
        {
            error!("Error: Measurement {} already exists", name);
        }
        event_offset
    }

    /// Associate a variable on stack to this DaqEvent and register it
    #[allow(clippy::too_many_arguments)]
    pub fn add_stack(
        &self,
        name: &'static str,
        ptr: *const u8,
        datatype: RegistryDataType,
        x_dim: u16,
        y_dim: u16,
        factor: f64,
        offset: f64,
        unit: &'static str,
        comment: &'static str,
    ) {
        let p = ptr as usize; // variable address
        let b = &self.buffer as *const _ as usize; // base address
        let o: i64 = p as i64 - b as i64; // variable - base address
        let event_offset: i16 = o.try_into().expect("memory offset out of rang");
        trace!(
            "add_stack: {} {:?} ptr={:p} base={:p} event_offset={}",
            name,
            datatype,
            ptr,
            &self.buffer as *const _,
            event_offset
        );
        if Xcp::get()
            .get_registry()
            .lock()
            .add_measurement(RegistryMeasurement::new(
                name,
                datatype,
                x_dim,
                y_dim,
                self.event,
                event_offset,
                0u64,
                factor,
                offset,
                comment,
                unit,
                None,
            ))
            .is_err()
        {
            println!("Error: Measurement {} already exists", name);
        }
    }

    /// Associate a variable on stack to this DaqEvent and register it
    #[allow(clippy::too_many_arguments)]
    pub fn add_heap(
        &self,
        name: &'static str,
        ptr: *const u8,
        datatype: RegistryDataType,
        x_dim: u16,
        y_dim: u16,
        factor: f64,
        offset: f64,
        unit: &'static str,
        comment: &'static str,
    ) {
        debug!("add_heap: {} {:?} ptr={:p} ", name, datatype, ptr,);

        if Xcp::get()
            .get_registry()
            .lock()
            .add_measurement(RegistryMeasurement::new(
                name, datatype, x_dim, y_dim, self.event, 0i16, ptr as u64, factor, offset, comment, unit, None,
            ))
            .is_err()
        {
            error!("Error: Measurement {} already exists", name);
        }
    }
}

//----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
// Macros to create and register DAQ events and variables

//-----------------------------------------------------------------------------
// Single global instances
//-----------------------------------------------------------------------------

/// Create a DAQ event with unique name and global scope and lifetime
/// This creates a single instance of this DAQ event once or returns the DAQ event if it already exists by using a lazy static
/// The DAQ event may have an optional capture buffer with the given capacity
/// Multiple concurrently running instances of a task or thread may safely trigger this DAQ event
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_create_event {
    // With capture buffer and cycle time
    // Value may be moved, variable addresses is capture buffer offset
    ( $name:expr, $capacity: expr, $cycle_time_ns: expr ) => {{
        // Scope for lazy static XCP_EVENT__, create the XCP event only once
        lazy_static::lazy_static! {
            static ref XCP_EVENT__: XcpEvent = Xcp::get().create_event_ext($name,false,$cycle_time_ns);
        }
        // Create the DAQ event every time the thread is running through this code
        DaqEvent::<{ $capacity }>::new_from(&XCP_EVENT__)
    }};
    // With capture buffer
    // Value may be moved, variable addresses is capture buffer offset
    ( $name:expr, $capacity: expr ) => {{
        // Scope for lazy static XCP_EVENT__, create the XCP event only once
        lazy_static::lazy_static! {
            static ref XCP_EVENT__: XcpEvent = Xcp::get().create_event($name);
        }
        // Create the DAQ event every time the thread is running through this code
        DaqEvent::<{ $capacity }>::new_from(&XCP_EVENT__)
    }};
    // Without capture buffer
    // Addresses are stack frame offsets between the variable and the event
    ( $name:expr ) => {{
        lazy_static::lazy_static! {
            static ref XCP_EVENT__: XcpEvent = Xcp::get().create_event($name);
        }
        DaqEvent::<0>::new_from(&XCP_EVENT__)
    }};
}

/// Capture the value of a variable with basic type into the the capture buffer of the given daq event
/// Register the given variable metadata once
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_capture {
    // name, event, comment, unit, factor,offset
    ( $id:ident, $daq_event:expr, $comment:expr, $unit:expr, $factor:expr, $offset:expr ) => {{
        static DAQ_OFFSET__: std::sync::atomic::AtomicI16 = std::sync::atomic::AtomicI16::new(-32768);
        let byte_offset;
        match DAQ_OFFSET__.compare_exchange(-32768, 0, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => {
                byte_offset = $daq_event.add_capture(
                    stringify!($id),
                    std::mem::size_of_val(&$id),
                    $id.get_type(),
                    1, // x_dim
                    1, // y_dim
                    $factor,
                    $offset,
                    $unit,
                    $comment,
                    None,
                );
                DAQ_OFFSET__.store(byte_offset, std::sync::atomic::Ordering::Relaxed);
            }
            Err(offset) => byte_offset = offset,
        };

        $daq_event.capture(&($id.to_le_bytes()), byte_offset);
    }};

    // name, event, comment, unit
    ( $id:ident, $daq_event:expr, $comment:expr, $unit:expr ) => {{
        static DAQ_OFFSET__: std::sync::atomic::AtomicI16 = std::sync::atomic::AtomicI16::new(-32768);
        let byte_offset;
        match DAQ_OFFSET__.compare_exchange(-32768, 0, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => {
                byte_offset = $daq_event.add_capture(
                    stringify!($id),
                    std::mem::size_of_val(&$id),
                    $id.get_type(),
                    1, // x_dim
                    1, // y_dim
                    1.0,
                    0.0,
                    $unit,
                    $comment,
                    None,
                );
                DAQ_OFFSET__.store(byte_offset, std::sync::atomic::Ordering::Relaxed);
            }
            Err(offset) => byte_offset = offset,
        };
        $daq_event.capture(&($id.to_le_bytes()), byte_offset);
    }};

    // name, event
    ( $id:ident, $daq_event:expr ) => {{
        static DAQ_OFFSET__: std::sync::atomic::AtomicI16 = std::sync::atomic::AtomicI16::new(-32768);
        let byte_offset;
        match DAQ_OFFSET__.compare_exchange(-32768, 0, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => {
                byte_offset = $daq_event.add_capture(
                    stringify!($id),
                    std::mem::size_of_val(&$id),
                    $id.get_type(),
                    1, // x_dim
                    1, // y_dim
                    1.0,
                    0.0,
                    "",
                    "",
                    None,
                );
                DAQ_OFFSET__.store(byte_offset, std::sync::atomic::Ordering::Relaxed);
            }
            Err(offset) => byte_offset = offset,
        };
        $daq_event.capture(&($id.to_le_bytes()), byte_offset);
    }};
}

/// Register a local variable with basic type for the given daq event
/// Address format and addressing mode will be relative to the stack frame position of the variable holding the event
/// No capture buffer required
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_register {
    // name, event, comment, unit, factor, offset
    ( $id:ident, $daq_event:expr, $comment:expr, $unit:expr, $factor:expr, $offset:expr ) => {{
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            //assert!($daq_event.get_capacity() == 0, "DAQ event with capture buffer");
            $daq_event.add_stack(stringify!($id), &$id as *const _ as *const u8, $id.get_type(), 1, 1, $factor, $offset, $unit, $comment);
        });
    }};
    // name, event, comment, unit
    ( $id:ident, $daq_event:expr, $comment:expr, $unit:expr ) => {{
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            //assert!($daq_event.get_capacity() == 0, "DAQ event with capture buffer");
            $daq_event.add_stack(stringify!($id), &$id as *const _ as *const u8, $id.get_type(), 1, 1, 1.0, 0.0, $unit, $comment);
        });
    }};
    // name, event
    ( $id:ident, $daq_event:expr ) => {{
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            //assert!($daq_event.get_capacity() == 0, "DAQ event with capture buffer");
            $daq_event.add_stack(stringify!($id), &$id as *const _ as *const u8, $id.get_type(), 1, 1, 1.0, 0.0, "", "");
        });
    }};
}

/// Register a local variable with type array of basic type for the given daq event
/// Address format and addressing mode will be relative to the stack frame position of the variable holding the event
/// No capture buffer required
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_register_array {
    // name, event
    ( $id:ident, $daq_event:expr ) => {{
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let dim = (std::mem::size_of_val(&$id) / std::mem::size_of_val(&$id[0])).try_into().expect("dim too large");
            $daq_event.add_stack(stringify!($id), &$id as *const _ as *const u8, ($id[0]).get_type(), dim, 1, 1.0, 0.0, "", "");
        });
    }};
}

/// Register a local variable which is a reference to heap with basic type for the given daq event
/// Address format and addressing mode will be absolute addressing mode
/// Assuming that the memory location is reachable in absolute addressing mode, otherwise panic
/// No capture buffer required
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_register_ref {
    // name, event
    ( $id:ident, $daq_event:expr ) => {{
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            $daq_event.add_heap(stringify!($id), &(*$id) as *const _ as *const u8, (*$id).get_type(), 1, 1, 1.0, 0.0, "", "", None);
        });
    }};
}

/// Capture the CDR serialized value of a variable into the capture buffer of the given daq event
/// Register the given metadata once
/// This includes the serialization schema as annotation text of the variable (Vector VLSD, variable length signal description)
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_serialize {
    // name, event, comment
    ( $id:ident, $daq_event:expr, $comment:expr) => {{
        static DAQ_OFFSET__: std::sync::atomic::AtomicI16 = std::sync::atomic::AtomicI16::new(-32768);
        let byte_offset;
        match DAQ_OFFSET__.compare_exchange(-32768, 0, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => {
                // @@@@ Experimental: Hard coded type here for point_cloud demo
                let annotation = GeneratorCollection::generate(&IDL::CDR, &$id.description()).unwrap();
                byte_offset = $daq_event.add_capture(
                    stringify!($id),
                    std::mem::size_of_val(&$id),
                    RegistryDataType::Blob,
                    $daq_event.buffer.len().try_into().expect("buffer too large"), // x_dim is buffer size in bytes
                    1,                                                             // y_dim
                    1.0,
                    0.0,
                    "",
                    $comment,
                    Some(annotation),
                );
                DAQ_OFFSET__.store(byte_offset, std::sync::atomic::Ordering::Relaxed);
            }
            Err(offset) => byte_offset = offset,
        };
        let v = cdr::serialize::<_, _, cdr::CdrBe>(&$id, cdr::Infinite).unwrap();
        $daq_event.capture(&v, byte_offset);
    }};
}

//----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// Thread local instances (tli)
//-----------------------------------------------------------------------------

/// Create a multi instance task DAQ event or return the DAQ event if it already exists in this thread
/// The DAQ event instance lives in thread local storage (TLS)
/// When the macro is called multiple times, the DAQ event is created once for each thread
/// This is thread safe, there is no potential race with other threads
/// Multiple concurrently running instances of a task use the DAQ event associated to their thread
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_create_event_tli {
    ( $name:expr, $capacity: literal ) => {{
        thread_local! {
            static XCP_EVENT__: std::cell::Cell<XcpEvent> = const { std::cell::Cell::new(XcpEvent::XCP_UNDEFINED_EVENT) }
        }
        if XCP_EVENT__.get() == XcpEvent::XCP_UNDEFINED_EVENT {
            XCP_EVENT__.set(Xcp::get().create_event_ext($name, true, 0));
        }
        DaqEvent::<$capacity>::new_from(&XCP_EVENT__.get())
    }};
    ( $name:expr ) => {{
        thread_local! {
            static XCP_EVENT__: std::cell::Cell<XcpEvent> = const { std::cell::Cell::new(XcpEvent::XCP_UNDEFINED_EVENT) }
        }
        if XCP_EVENT__.get() == XcpEvent::XCP_UNDEFINED_EVENT {
            XCP_EVENT__.set(Xcp::get().create_event_ext($name, true, 0));
        }
        DaqEvent::<0>::new_from(&XCP_EVENT__.get())
    }};
}

/// Capture the value of a variable with basic type for the given multi instance daq event
/// Register the given meta data once for each event instance
/// The events index number will be appended to the variable name
/// Append an index to the variable name to distinguish between different threads
// @@@@ The offset does not need to be stored in thread local storage, static would be sufficient, as it is the same for all instances of a task
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_capture_tli {
    // name, event, comment, unit, factor, offset
    ( $id:ident, $daq_event:expr, $comment:expr, $unit:expr, $factor:expr, $offset:expr ) => {{
        thread_local! {
            static DAQ_OFFSET__: std::cell::Cell<i16> = const { std::cell::Cell::new(-32768) }
        }
        let mut offset = DAQ_OFFSET__.get();
        if offset == -32768 {
            offset = $daq_event.add_capture(
                stringify!($id),
                std::mem::size_of_val(&$id),
                $id.get_type(),
                1, // x_dim
                1, // y_dim
                $factor,
                $offset,
                $unit,
                $comment,
                None,
            );
            DAQ_OFFSET__.set(offset)
        };
        $daq_event.capture(&($id.to_le_bytes()), offset);
    }};

    // name, event, comment, unit
    ( $id:ident, $daq_event:expr, $comment:expr, $unit:expr ) => {{
        thread_local! {
            static DAQ_OFFSET__: std::cell::Cell<i16> = const { std::cell::Cell::new(-32768) }
        }
        let mut offset = DAQ_OFFSET__.get();
        if offset == -32768 {
            offset = $daq_event.add_capture(
                stringify!($id),
                std::mem::size_of_val(&$id),
                $id.get_type(),
                1, // x_dim
                1, // y_dim
                1.0,
                0.0,
                $unit,
                $comment,
                None,
            );
            DAQ_OFFSET__.set(offset)
        };
        $daq_event.capture(&($id.to_le_bytes()), offset);
    }};

    // name, event
    ( $id:ident, $daq_event:expr ) => {{
        thread_local! {
            static DAQ_OFFSET__: std::cell::Cell<i16> = const { std::cell::Cell::new(-32768) }
        }
        let mut offset = DAQ_OFFSET__.get();
        if offset == -32768 {
            offset = $daq_event.add_capture(
                stringify!($id),
                std::mem::size_of_val(&$id),
                $id.get_type(),
                1, // x_dim
                1, // y_dim
                1.0,
                0.0,
                "",
                "",
                None,
            );
            DAQ_OFFSET__.set(offset)
        };
        $daq_event.capture(&($id.to_le_bytes()), offset);
    }};
}

/// Register a local variable with basic type once for the given multi instance daq event
/// Address will be relative to the stack frame position of event
/// No capture buffer required
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_register_tli {
    // name, event
    ( $id:ident, $daq_event:expr ) => {{
        thread_local! {
            static DAQ_REGISTERED__: std::cell::Cell<i16> = const { std::cell::Cell::new(0) }
        }
        if DAQ_REGISTERED__.get() == 0 {
            DAQ_REGISTERED__.set(1);
            //assert!($daq_event.get_capacity() == 0, "DAQ event with capture buffer");
            $daq_event.add_stack(stringify!($id), &$id as *const _ as *const u8, $id.get_type(), 1, 1, 1.0, 0.0, "", "");
        };
    }};
    // name, event, comment, unit
    ( $id:ident, $daq_event:expr, $comment:expr, $unit:expr ) => {{
        thread_local! {
            static DAQ_REGISTERED__: std::cell::Cell<i16> = const { std::cell::Cell::new(0) }
        }
        if DAQ_REGISTERED__.get() == 0 {
            DAQ_REGISTERED__.set(1);
            //assert!($daq_event.get_capacity() == 0, "DAQ event with capture buffer");
            $daq_event.add_stack(stringify!($id), &$id as *const _ as *const u8, $id.get_type(), 1, 1, 1.0, 0.0, $unit, $comment);
        };
    }};
}

//----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// multi instance, user defined instances
//-----------------------------------------------------------------------------

/// Create a multi instance task DAQ event
/// Each call will create a new instance of an event named "<name>_<instance_index>""
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_create_event_instance {
    ( $name:expr ) => {{
        DaqEvent::<0>::new_from(&Xcp::get().create_event_ext($name, true, 0))
    }};
}

/// Register a local variable with basic type for the given daq event once for each event instance
/// The events index number will be appended to the variable name
/// May be executed only once, there is no check the instance already exists
/// Address will be relative to the stack frame position of event
/// No capture buffer required
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_register_instance {
    // name, event
    ( $id:ident, $daq_event:expr ) => {{
        //assert!($daq_event.get_capacity() == 0, "DAQ event with capture buffer");
        $daq_event.add_stack(stringify!($id), &$id as *const _ as *const u8, $id.get_type(), 1, 1, 1.0, 0.0, "", "");
    }};
}

//-----------------------------------------------------------------------------
// Test
// Tests for the daq types
//-----------------------------------------------------------------------------

#[cfg(test)]
mod daq_tests {

    #![allow(dead_code)]
    #![allow(unused_imports)]

    use super::*;

    use crate::reg::*;
    use crate::xcp::*;

    //-----------------------------------------------------------------------------
    // Test local variable register
    #[test]
    fn daq_register() {
        xcp_test::test_setup(log::LevelFilter::Info);
        let xcp = Xcp::get();

        let event = daq_create_event!("TestEvent1");
        let mut counter1: u16 = 0;
        daq_register!(counter1, event);
        loop {
            counter1 += 1;
            {
                let mut counter2: u8 = 0;
                daq_register!(counter2, event);
                counter2 += 1;

                {
                    let mut counter3: u32 = 0;
                    daq_register!(counter3, event);
                    counter3 += 1;
                    {
                        let mut counter4: u64 = 0;
                        daq_register!(counter4, event);
                        counter4 += 1;

                        trace!("counter1={}", counter1);
                        trace!("counter2={}", counter2);
                        trace!("counter3={}", counter3);
                        trace!("counter4={}", counter4);
                    }
                }
            }
            event.trigger();
            if counter1 == 3 {
                break;
            }
        }
        xcp.write_a2l().unwrap(); // @@@@ Remove: force A2L write
    }

    //-----------------------------------------------------------------------------
    // Test local variable capture
    #[test]
    fn daq_capture() {
        xcp_test::test_setup(log::LevelFilter::Info);
        let xcp = Xcp::get();

        let mut event = daq_create_event!("TestEvent1", 15);
        let mut counter1: u16 = 0;
        loop {
            counter1 += 1;
            {
                let mut counter2: u8 = 0;
                counter2 += 1;
                {
                    let mut counter3: u32 = 0;
                    counter3 += 1;
                    {
                        let mut counter4: u64 = 0;
                        counter4 += 1;
                        daq_capture!(counter3, event);
                        daq_capture!(counter4, event);
                    }
                }
                daq_capture!(counter2, event);
            }
            daq_capture!(counter1, event);
            event.trigger();
            if counter1 == 3 {
                break;
            }
        }
        xcp.write_a2l().unwrap(); // @@@@ Remove: force A2L write
    }

    //-----------------------------------------------------------------------------
    // Test A2L file generation for local variables
    #[test]
    fn test_a2l_local_variables_capture() {
        xcp_test::test_setup(log::LevelFilter::Info);
        let xcp = Xcp::get();

        let mut event1: DaqEvent<0> = DaqEvent::new_from(&XcpEvent::XCP_UNDEFINED_EVENT);
        let mut event1_2: DaqEvent<0> = DaqEvent::new_from(&XcpEvent::XCP_UNDEFINED_EVENT);
        for i in 0..2 {
            let event = daq_create_event!("event");
            if i == 0 {
                event1 = event;
            } else {
                event1_2 = event;
            }
        }
        assert!(event1.get_xcp_event().get_channel() == event1_2.get_xcp_event().get_channel());

        // let event1 = daq_create_event!("event"); // panic: duplicate event

        let mut event2_1 = daq_create_event_tli!("ev_tli", 256); // -> event name: ev_tli_1
        let mut event2_2 = daq_create_event_tli!("ev_tli", 256); // -> event name: ev_tli_2
        let mut event2_3 = daq_create_event_tli!("ev_tli", 256); // -> event name: ev_tli_3
        let event3_1 = daq_create_event_instance!("ev_instance"); // -> event name: ev_instance_1
        let event3_2 = daq_create_event_instance!("ev_instance"); // -> event name: ev_instance_2
        let event3_3 = daq_create_event_instance!("ev_instance"); // -> event name: ev_instance_3
        let channel1: f64 = 1.0;
        let channel2: f64 = 2.0;
        let channel3: f64 = 3.0;
        let channel4: f64 = 4.0;
        let channel5: f64 = 5.0;
        let channel6: f64 = 6.0;
        let channel7: f64 = 7.0;
        let channel8: f64 = 8.0;
        let channel9: f64 = 9.0;

        daq_register!(channel1, event1, "comment", "unit", 2.0, 5.0); // -> variable channel1
        daq_register!(channel2, event1, "comment", "unit", 2.0, 5.0); // -> variable channel2

        daq_capture_tli!(channel3, event2_1, "", "Volt"); // -> variable channel3_1
        daq_capture_tli!(channel3, event2_2, "", "Volt"); // -> variable channel3_2
        daq_capture_tli!(channel3, event2_3, "", "Volt"); // -> variable channel3_3

        daq_capture_tli!(channel4, event2_2, "comment", "unit"); // -> variable channel4_2
        daq_capture_tli!(channel5, event2_3, "comment", "unit", 2.0, 5.0); // -> variable channel5_3

        daq_register_instance!(channel6, event3_1); // -> variable channel6_1
        daq_register_instance!(channel6, event3_2); // -> variable channel6_2
        daq_register_instance!(channel6, event3_3); // -> variable channel6_3
        daq_register_instance!(channel7, event3_1); // -> variable channel7_1
        daq_register_instance!(channel8, event3_1); // -> variable channel8_1
        daq_register_instance!(channel9, event3_1); // -> variable channel9_1

        // daq_register_instance!(channel6, event5); // panic: duplicate measurement

        xcp.write_a2l().unwrap(); // @@@@ Remove: force A2L write
    }
}
