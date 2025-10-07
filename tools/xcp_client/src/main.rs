//-----------------------------------------------------------------------------
// xcp_client - XCP client example
// This tool demonstrates how to to use the xcp_client library to
//  connect to an XCP server
//  load or upload an A2L file
//  read and write calibration variables,
//  configure and aquire measurment variables.
//
// Run:
// cargo r -p xcp_client -- -h

use indexmap::IndexMap;
use parking_lot::Mutex;
use std::ffi::OsStr;
use std::net::Ipv4Addr;

use std::{error::Error, sync::Arc};
use xcp_lite::registry::{McAddress, McDimType, McEvent, McObjectType, McSupportData, McValueType, Registry};

mod xcp_client;
use xcp_client::*;

mod xcp_test_executor;
use xcp_test_executor::test_executor;

// This module contains code adapted from https://github.com/DanielT/a2ltool
// Original code licensed under MIT/Apache-2.0
// Copyright (c) DanielT
mod debuginfo;
use debuginfo::{DbgDataType, DebugData, TypeInfo};

//-----------------------------------------------------------------------------
// Command line arguments

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // -l --log-level
    /// Log level (Off=0, Error=1, Warn=2, Info=3, Debug=4, Trace=5)
    #[arg(short, long, default_value_t = 3)]
    log_level: u8,

    // -v --verbose
    /// Verbose output
    /// Enables additional output when reading ELF files and creating A2L files
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    // -d --dest_addr
    /// XCP server address
    #[arg(short, long, default_value = "127.0.0.1:5555")]
    dest_addr: String,

    // -p --port
    /// XCP server port number
    #[arg(short, long, default_value_t = 5555)]
    port: u16,

    // -b --bind-addr
    /// Bind address, master port number
    #[arg(short, long, default_value = "0.0.0.0:9999")]
    bind_addr: String,

    // --tcp
    /// Use TCP for XCP communication
    #[arg(long, default_value_t = false)]
    tcp: bool,
    // --udp
    /// Use UDP for XCP communication
    #[arg(long, default_value_t = false)]
    udp: bool,

    // -a, --a2l
    /// Specify and overide the name of the A2L file name
    /// If not specified, The A2L file name is read from the XCP server
    #[arg(short, long, default_value = "")]
    a2l: String,

    // --upload-a2l
    /// Upload A2L file from XCP server
    /// Requires that the XCP server supports GET_ID A2L upload
    #[arg(long, default_value_t = false)]
    upload_a2l: bool,

    // --create-a2l
    /// Build an A2L file template from XCP server information about events and memory segments
    /// Requires that the XCP server supports the GET_EVENT_INFO and GET_SEGMENT_INFO commands
    #[arg(long, default_value_t = false)]
    create_a2l: bool,

    // -e, --elf
    /// Specifiy the name of an ELF file, create an A2L file from ELF debug information
    /// If connected to a XCP server, events and memory segments will be extracted from the XCP server
    #[arg(short, long, default_value = "")]
    elf: String,

    // --list_mea
    /// Lists all specified measurement variables (regex) found in the A2L file
    #[clap(long, default_value = "")]
    list_mea: String,

    // --list-cal
    /// Lists all specified calibration variables (regex) found in the A2L file
    #[clap(long, default_value = "")]
    list_cal: String,

    // -m --mea
    /// Specify variable names for DAQ measurement (list), may be list of names separated by space or single regular expressions (e.g. ".*")
    #[arg(short, long, value_delimiter = ' ', num_args = 1..)]
    mea: Vec<String>,

    // --time-ms
    /// Limit measurement duration to n ms
    #[arg(long, default_value_t = 0)]
    time_ms: u64,
    // -t --time
    /// Limit measurement duration to n s
    #[arg(short, long, default_value_t = 0)]
    time: u64,

    // --cal
    /// Set calibration variable to a value (format: "variable_name value")
    #[clap(long, value_names = ["NAME", "VALUE"], num_args = 2)]
    cal: Vec<String>,

    /// --test
    /// Execute a test sequence on the XCP server
    #[arg(long, default_value_t = false)]
    test: bool,
}

//----------------------------------------------------------------------------------------------
// Logging

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

trait ToLogLevelFilter {
    fn to_log_level_filter(self) -> log::LevelFilter;
}

impl ToLogLevelFilter for u8 {
    fn to_log_level_filter(self) -> log::LevelFilter {
        match self {
            0 => log::LevelFilter::Off,
            1 => log::LevelFilter::Error,
            2 => log::LevelFilter::Warn,
            3 => log::LevelFilter::Info,
            4 => log::LevelFilter::Debug,
            5 => log::LevelFilter::Trace,
            _ => log::LevelFilter::Warn,
        }
    }
}

//-----------------------------------------------------------------------------
// Test (--test) settings

const TEST_CAL: xcp_test_executor::TestModeCal = xcp_test_executor::TestModeCal::Cal; // Execute calibration tests: Cal or None
const TEST_DAQ: xcp_test_executor::TestModeDaq = xcp_test_executor::TestModeDaq::Daq; // Execute measurement tests: Daq or None
const TEST_DURATION_MS: u64 = 5000;

//------------------------------------------------------------------------
// Handle incoming DAQ data
// Prints the decoded data to the console

const MAX_EVENT: usize = 64;

#[derive(Debug)]
struct DaqDecoder {
    daq_odt_entries: Option<Vec<Vec<OdtEntry>>>,
    timestamp_resolution: u64,
    daq_header_size: u8,
    event_count: usize,
    byte_count: usize,
    daq_timestamp: [u64; MAX_EVENT],
}

impl DaqDecoder {
    pub fn new() -> DaqDecoder {
        DaqDecoder {
            daq_odt_entries: None,
            timestamp_resolution: 0,
            daq_header_size: 0,
            event_count: 0,
            byte_count: 0,
            daq_timestamp: [0; MAX_EVENT],
        }
    }
}

impl XcpDaqDecoder for DaqDecoder {
    // Set start time and init
    fn start(&mut self, daq_odt_entries: Vec<Vec<OdtEntry>>, timestamp: u64) {
        // Init
        self.daq_odt_entries = Some(daq_odt_entries);
        self.event_count = 0;
        self.byte_count = 0;
        for t in self.daq_timestamp.iter_mut() {
            *t = timestamp;
        }
    }

    fn stop(&mut self) {}

    // Set timestamp resolution
    fn set_daq_properties(&mut self, timestamp_resolution: u64, daq_header_size: u8) {
        self.daq_header_size = daq_header_size;
        self.timestamp_resolution = timestamp_resolution;
    }

    // Decode DAQ data
    fn decode(&mut self, lost: u32, buf: &[u8]) {
        let daq: u16;
        let odt: u8;
        let mut timestamp_raw: u32 = 0;
        let data: &[u8];

        // Decode header and raw timestamp
        if self.daq_header_size == 4 {
            daq = (buf[2] as u16) | ((buf[3] as u16) << 8);
            odt = buf[0];
            if odt == 0 {
                timestamp_raw = (buf[4] as u32) | ((buf[4 + 1] as u32) << 8) | ((buf[4 + 2] as u32) << 16) | ((buf[4 + 3] as u32) << 24);
                data = &buf[8..];
            } else {
                data = &buf[4..];
            }
        } else {
            daq = buf[1] as u16;
            odt = buf[0];
            if odt == 0 {
                timestamp_raw = (buf[2] as u32) | ((buf[2 + 1] as u32) << 8) | ((buf[2 + 2] as u32) << 16) | ((buf[2 + 3] as u32) << 24);
                data = &buf[6..];
            } else {
                data = &buf[2..];
            }
        }

        assert!(daq < MAX_EVENT as u16);
        assert!(odt == 0);

        // Decode full 64 bit daq timestamp
        let t_last = self.daq_timestamp[daq as usize];
        let t: u64 = if odt == 0 {
            let tl = (t_last & 0xFFFFFFFF) as u32;
            let mut th = (t_last >> 32) as u32;
            if timestamp_raw < tl {
                th += 1;
            }
            let t = (timestamp_raw as u64) | ((th as u64) << 32);
            if t < t_last {
                warn!("Timestamp of daq {} declining {} -> {}", daq, t_last, t);
            }
            self.daq_timestamp[daq as usize] = t;
            t
        } else {
            t_last
        };

        println!("DAQ: lost={}, daq={}, odt={}, t={}ns (+{}us)", lost, daq, odt, t, (t - t_last) / 1000);

        // Get daq list
        let daq_list = &self.daq_odt_entries.as_ref().unwrap()[daq as usize];

        // Decode all odt entries
        for odt_entry in daq_list.iter() {
            let value_size = odt_entry.a2l_type.size;
            let mut value_offset = odt_entry.offset as usize + value_size - 1;
            let mut value: u64 = 0;
            loop {
                value |= data[value_offset] as u64;
                if value_offset == odt_entry.offset as usize {
                    break;
                };
                value <<= 8;
                value_offset -= 1;
            }
            match odt_entry.a2l_type.encoding {
                A2lTypeEncoding::Signed => {
                    match value_size {
                        1 => {
                            let signed_value: i8 = value as u8 as i8;
                            println!(" {} = {}", odt_entry.name, signed_value);
                        }
                        2 => {
                            let signed_value: i16 = value as u16 as i16;
                            println!(" {} = {}", odt_entry.name, signed_value);
                        }
                        4 => {
                            let signed_value: i32 = value as u32 as i32;
                            println!(" {} = {}", odt_entry.name, signed_value);
                        }
                        8 => {
                            let signed_value: i64 = value as i64;
                            println!(" {} = {}", odt_entry.name, signed_value);
                        }
                        _ => {
                            warn!("Unsupported signed value size {}", value_size);
                        }
                    };
                }
                A2lTypeEncoding::Unsigned => {
                    println!(" {} = {}", odt_entry.name, value);
                }
                A2lTypeEncoding::Float => {
                    if odt_entry.a2l_type.size == 4 {
                        // #[allow(clippy::transmute_int_to_float)]
                        // let value: f32 = unsafe { std::mem::transmute(value as u32) };
                        let value: f32 = f32::from_bits(value as u32);

                        println!(" {} = {}", odt_entry.name, value);
                    } else {
                        // #[allow(clippy::transmute_int_to_float)]
                        // let value: f64 = unsafe { std::mem::transmute(value) };
                        let value: f64 = f64::from_bits(value);
                        println!(" {} = {}", odt_entry.name, value);
                    }
                }
                A2lTypeEncoding::Blob => {
                    panic!("Blob not supported");
                }
            }
        }

        self.byte_count += data.len(); // overall payload byte count
        self.event_count += 1; // overall event count
    }
}

//------------------------------------------------------------------------
// Handle incoming SERV_TEXT data
// Prints the text to the console

#[derive(Debug, Clone, Copy)]
struct ServTextDecoder;

impl ServTextDecoder {
    pub fn new() -> ServTextDecoder {
        ServTextDecoder {}
    }
}

impl XcpTextDecoder for ServTextDecoder {
    // Handle incomming text data from XCP server
    fn decode(&self, data: &[u8]) {
        print!("[SERV_TEXT] ");
        let mut j = 0;
        while j < data.len() {
            print!("{}", data[j] as char);
            j += 1;
        }
    }
}

//------------------------------------------------------------------------
//  ELF reader and A2L creator

fn print_debug_stats(debug_data: &DebugData) {
    println!("Debug information summary:");
    println!("  Compilation units: {} units", debug_data.unit_names.len());
    println!("  Sections: {} sections", debug_data.sections.len());
    println!("  Type names: {} named types", debug_data.typenames.len());
    println!("  Types: {} total types", debug_data.types.len());
    println!("  Demangled names: {} entries", debug_data.demangled_names.len());

    let mut variable_count = 0;
    for (name, var_infos) in &debug_data.variables {
        variable_count += var_infos.len();
    }
    println!("  Variables {} with {} unique names", variable_count, debug_data.variables.len());
}

fn printf_debug_info(debug_data: &DebugData) {
    //Print compilation units
    println!("\nCompilation Units (debug_data.unit_names)");
    for (idx, unit_name) in debug_data.unit_names.iter().enumerate() {
        println!("  Unit {}: {:?}", idx, unit_name);
    }

    //Print sections information
    println!("\nMemory Sections (debug_data.sections)");
    for (name, (addr, size)) in &debug_data.sections {
        println!("  Section '{}': address=0x{:08x}, size=0x{:x} ({} bytes)", name, addr, size, size);
    }

    //Print type names
    println!("\nType Names (debug_data.typenames)");
    for (type_name, type_refs) in &debug_data.typenames {
        println!("Type name '{}': {} references", type_name, type_refs.len());
        for type_ref in type_refs {
            if let Some(type_info) = debug_data.types.get(type_ref) {
                println!("  -> type_ref={}, size={} bytes, unit={}", type_ref, type_info.get_size(), type_info.unit_idx);
            }
        }
    }

    // Print types
    println!("\nTypes:");
    for (type_ref, type_info) in &debug_data.types {
        let type_name = if let Some(name) = &type_info.name { name } else { "" };
        let unit_name: &Option<String> = if let Some(unit_name) = debug_data.unit_names.get(type_info.unit_idx) {
            unit_name
        } else {
            &None
        };
        println!(
            "TypeRef {}: name = '{}', size = {} bytes, unit = {} ({:?})",
            type_ref,
            type_name,
            type_info.get_size(),
            type_info.unit_idx,
            unit_name
        );
        print_type_info(type_info);
    }

    // Print demangled names
    println!("\nDemangled Names");
    for (mangled_name, demangled_name) in &debug_data.demangled_names {
        println!("  '{}' -> '{}'", mangled_name, demangled_name);
    }

    // Print variables
    println!("\nVariables:");
    for (var_name, var_info) in &debug_data.variables {
        println!("Variable '{}': {:?}", var_name, var_info);
    }
    println!();
}

fn print_type_info(type_info: &TypeInfo) {
    let type_name = if let Some(name) = &type_info.name { name } else { "" };
    let type_size = type_info.get_size();

    print!("    TypeInfo: {}", type_name);
    // print!(" (unit_idx = {}, dbginfo_offset = {})",type_info.unit_idx, type_info.dbginfo_offset);

    match &type_info.datatype {
        DbgDataType::Uint8 | DbgDataType::Uint16 | DbgDataType::Uint32 | DbgDataType::Uint64 => {
            println!(" Integer: {} byte unsigned", type_size);
        }
        DbgDataType::Sint8 | DbgDataType::Sint16 | DbgDataType::Sint32 | DbgDataType::Sint64 => {
            println!(" Integer: {} byte signed", type_size);
        }
        DbgDataType::Float | DbgDataType::Double => {
            println!(" Floating point: {} byte", type_size);
        }

        DbgDataType::Pointer(typeref, size) => {
            println!(" Pointer: typeref = {}, size = {} ", typeref, size);
        }
        DbgDataType::Array { arraytype, dim, stride, size } => {
            println!(" Array: typeref = {}, dim = {:?}, stride = {} bytes, size = {} bytes", arraytype, dim, stride, size);
        }
        DbgDataType::Struct { size, members } => {
            println!(" Struct: {} fields, size = {}", members.len(), size);
            for (name, (type_info, member_offset)) in members {
                let member_size = type_info.get_size();
                println!("      Field '{}': size = {} bytes, offset = {} bytes", name, member_size, member_offset);
            }
        }
        DbgDataType::Union { members, size } => {
            println!(" Union: {} members, size = {} bytes", members.len(), size);
        }
        DbgDataType::Enum { size, signed, enumerators } => {
            println!(" Enum: {} variants, size = {} bytes", enumerators.len(), size);
            for (name, value) in enumerators {
                println!("      Variant '{}': value={}", name, value);
            }
        }
        DbgDataType::Bitfield { basetype, bit_offset, bit_size } => {
            println!(" Bitfield: base type = {:?}, offset = {} bits, size = {} bits", basetype.datatype, bit_offset, bit_size);
        }
        DbgDataType::Class { size, inheritance, members } => {
            println!(" Class: {} members, size = {} bytes", members.len(), size);
        }
        DbgDataType::FuncPtr(size) => {
            println!(" Function pointer: size = {} bytes", size);
        }
        DbgDataType::TypeRef(typeref, size) => {
            println!(" TypeRef: typeref = {}, size = {} bytes", typeref, size);
        }
        _ => {
            println!(" Other type: {:?}", &type_info.datatype);
        }
    }
}

struct ElfReader {
    debug_data: DebugData,
}

impl ElfReader {
    fn new(file_name: &str) -> Option<ElfReader> {
        // Load debug information from the ELF file
        info!("Loading debug information from ELF file: {}", file_name);
        let debug_data = DebugData::load_dwarf(OsStr::new(file_name), true);
        match debug_data {
            Ok(debug_data) => Some(ElfReader { debug_data }),
            Err(e) => {
                error!("Failed to load debug info from '{}': {}", file_name, e);
                None
            }
        }
    }

    fn get_value_type(&self, reg: &mut Registry, type_info: &TypeInfo, object_type: McObjectType) -> McValueType {
        let type_size = type_info.get_size();
        match &type_info.datatype {
            DbgDataType::Uint8 => McValueType::Ubyte,
            DbgDataType::Uint16 => McValueType::Uword,
            DbgDataType::Uint32 => McValueType::Ulong,
            DbgDataType::Uint64 => McValueType::Ulonglong,
            DbgDataType::Sint8 => McValueType::Sbyte,
            DbgDataType::Sint16 => McValueType::Sword,
            DbgDataType::Sint32 => McValueType::Slong,
            DbgDataType::Sint64 => McValueType::Slonglong,
            DbgDataType::Float => McValueType::Float32Ieee,
            DbgDataType::Double => McValueType::Float64Ieee,
            DbgDataType::Struct { size, members } => {
                if let Some(type_name) = &type_info.name {
                    // Register the typedef struct for the value type typedef
                    if let Some(name) = type_info.name.as_ref() {
                        let _ = self.register_struct(reg, object_type, name.clone(), *size as usize, members);
                    }
                    McValueType::new_typedef(type_name.clone())
                } else {
                    warn!("Struct type without name in get_field_type");
                    McValueType::Ubyte
                }
            }
            DbgDataType::Enum { size, signed, enumerators } => McValueType::from_integer_size(*size as usize, *signed),

            DbgDataType::TypeRef(typeref, size) => {
                if let Some(typeinfo) = self.debug_data.types.get(typeref) {
                    self.get_value_type(reg, typeinfo, object_type)
                } else {
                    error!("TypeRef {} to unknown in get_field_type", typeref);
                    McValueType::Ubyte
                }
            }

            DbgDataType::Pointer(pointee, size) => {
                if *size == 4 {
                    McValueType::Ulong
                } else if *size == 8 {
                    McValueType::Ulonglong
                } else {
                    warn!("Unsupported pointer size {} in get_field_type", size);
                    McValueType::Ulonglong
                }
            }

            // These type are not a supported value type
            // DbgDataType::Bitfield | DbgDataType::Pointer | DbgDataType::FuncPtr | DbgDataType::Class | DbgDataType::Union | DbgDataType::Enum  | DbgDataType::Other =>
            _ => {
                error!("Unsupported type in get_field_type: {:?}", &type_info.datatype);
                assert!(false, "Unsupported type in get_field_type: {:?}", &type_info.datatype);
                McValueType::Ubyte
            }
        }
    }

    fn get_dim_type(&self, reg: &mut Registry, type_info: &TypeInfo, object_type: McObjectType) -> McDimType {
        let type_size = type_info.get_size();
        match &type_info.datatype {
            DbgDataType::Array { arraytype, dim, stride, size } => {
                assert!(dim.len() != 0);
                let elem_type = self.get_value_type(reg, arraytype, object_type);
                if dim.len() > 2 {
                    warn!("Only 1D and 2D arrays supported, got {}D", dim.len());
                    McDimType::new(McValueType::Ubyte, 1, 1)
                } else if dim.len() == 1 {
                    McDimType::new(elem_type, dim[0] as u16, 1)
                } else {
                    McDimType::new(elem_type, dim[0] as u16, dim[1] as u16)
                }
            }
            _ => McDimType::new(self.get_value_type(reg, type_info, object_type), 1, 1),
        }
    }

    fn register_struct(
        &self,
        reg: &mut Registry,
        object_type: McObjectType,
        type_name: String,
        size: usize,
        members: &IndexMap<String, (TypeInfo, u64)>,
    ) -> Result<(), Box<dyn Error>> {
        let typedef = reg.add_typedef(type_name.clone(), size)?;
        for (field_name, (type_info, field_offset)) in members {
            let field_dim_type = self.get_dim_type(reg, type_info, object_type);
            let field_mc_support_data = McSupportData::new(object_type);
            reg.add_typedef_field(&type_name, field_name.clone(), field_dim_type, field_mc_support_data, (*field_offset).try_into().unwrap())?;
        }
        Ok(())
    }

    fn register(&self, reg: &mut Registry, verbose: bool) -> Result<(), Box<dyn Error>> {
        // Load debug information from the ELF file
        info!("Registering debug information");

        let mut next_event_id: u16 = 0;
        let mut next_segment_number: u16 = 0;

        // Iterate over variables
        for (var_name, var_infos) in &self.debug_data.variables {
            // Skip standard library variables and system/compiler internals (__<name>)s
            // Skip global XCP variables (gXCP.. and gA2L..)
            if var_name.starts_with("__") || var_name.starts_with("gXcp") || var_name.starts_with("gA2l") {
                continue;
            }

            // cal__<name> (local scope static, name is calibration segment name and type name)
            // Calibration segment definition
            if var_name.starts_with("cal__") {
                assert!(var_infos.len() == 1); // Only one definition allowed
                let var_info = &var_infos[0];
                let function_name = if let Some(f) = var_info.function.as_ref() { f.as_str() } else { "" };
                let unit_idx = var_info.unit_idx;

                // remove the "cal__" prefix
                let seg_name = var_name.strip_prefix("cal__").unwrap_or(var_name);
                info!("Calibration segment definition '{}' found in function {}:{}", seg_name, unit_idx, function_name);
                // Find the segment in the registry
                if let Some(_seg) = reg.cal_seg_list.find_cal_seg(seg_name) {
                } else {
                    // length will be determined from variable 'seg_name' which is the default page
                    let length = if let Some(var_info) = self.debug_data.variables.get(seg_name) {
                        if let Some(type_info) = self.debug_data.types.get(&var_info[0].typeref) {
                            type_info.get_size()
                        } else {
                            warn!("Could not determine calibration segment length ");
                            0
                        }
                    } else {
                        warn!("Could not find calibration segment reference page {}", seg_name);
                        0
                    };
                    let addr = var_info.address.try_into().unwrap(); // @@@@ TODO: Handle 64 bit addresses
                    let addr_ext = 0; // Absolute addressing
                    reg.cal_seg_list
                        .add_cal_seg_by_addr(seg_name.to_string(), next_segment_number, addr_ext, addr, length as u32)
                        .unwrap();
                    error!(
                        "Unknown calibration segment '{}':  Created with number={}, addr = {:#x}, length = {:#x}",
                        seg_name, next_segment_number, addr, length
                    );
                    next_segment_number += 1;
                    continue; // skip this variable
                }
            }

            // evt__<name> (thread local static, name is event name)
            // Event definitions (thread local static variaables)
            if var_name.starts_with("evt__") {
                // remove the "evt__" prefix
                let evt_name = var_name.strip_prefix("evt__").unwrap_or("unnamed");
                info!("Event definition for event '{}' found", evt_name);
                // Find the event in the registry
                if let Some(_evt) = reg.event_list.find_event(evt_name, 0) {
                } else {
                    // @@@@ TODO: Event number unknown !!!!!!!!!!!!!!!
                    reg.event_list.add_event(McEvent::new(evt_name.to_string(), 0, next_event_id, 0)).unwrap();
                    error!("Unknown event '{}': Created with event id = {}", evt_name, next_event_id);
                    next_event_id += 1;
                    continue; // skip this variable
                }
            }

            // trg__<event_name> (thread local static, name is event name)
            // Event definitions (thread local static variables)
            if var_name.starts_with("trg__") {
                assert!(var_infos.len() == 1); // Only one definition allowed
                let var_info = &var_infos[0];
                let function_name = if let Some(f) = var_info.function.as_ref() { f.as_str() } else { "" };

                // remove the "trg__" prefix
                let evt_name = var_name.strip_prefix("trg__").unwrap_or(var_name);
                info!("Event trigger found in function {}, event name = {}", function_name, evt_name);
                continue; // skip this variable
            }

            // daq__<event_name>__<var_name> (local scope static variables)
            // Check for captured variables with format "daq__<event_name>__<var_name>"
            let mut a2l_name = var_name.to_string();
            let mut xcp_event_id = 0u16;
            if var_name.starts_with("daq__") {
                // remove the "daq__" prefix
                let new_name = var_name.strip_prefix("daq__").unwrap_or(var_name);
                // get event name and variable name
                let mut parts = new_name.split("__");
                let event_name = parts.next().unwrap_or("");
                let var_name = parts.next().unwrap_or("");
                // Find the event in the registry
                if let Some(id) = reg.event_list.find_event(event_name, 0) {
                    xcp_event_id = id.id;
                    a2l_name = format!("{}.{}", event_name, var_name);
                } else {
                    warn!("Event '{}' for captured variable '{}' not found in registry", event_name, var_name);
                    continue; // skip this variable
                }
            }

            // Process all variable infos (same name, different types or addresses)
            if var_infos.is_empty() {
                warn!("Variable '{}' has no variable info", var_name);
            }
            for var_info in var_infos {
                // Register only global variables
                if var_info.address == 0 {
                    continue;
                }

                let a2l_name = a2l_name.clone();
                let a2l_addr = var_info.address.try_into().unwrap(); // @@@@ TODO: Handle 64 bit addresses
                let a2l_addr_ext = 0;

                // Check if the address is in a calibration segment
                let (object_type, mc_addr) = if reg.cal_seg_list.find_cal_seg_by_address(a2l_addr).is_some() {
                    (McObjectType::Characteristic, McAddress::new_a2l(a2l_addr, a2l_addr_ext))
                } else {
                    (McObjectType::Measurement, McAddress::new_a2l_with_event(xcp_event_id, a2l_addr, a2l_addr_ext))
                };

                // Register measurement variable if possible
                if let Some(type_info) = self.debug_data.types.get(&var_info.typeref) {
                    // Print variable info
                    if verbose {
                        println!("  {}: addr = 0x{:08x}", a2l_name, var_info.address);
                        //println!("  {}: addr = 0x{:08x}, typeref = {}, unit = {}",a2l_name, var_info.address, var_info.typeref, var_info.unit_idx);
                    }

                    // Print type info
                    if verbose {
                        print_type_info(type_info);
                    }

                    // Register supported variable types in the registry
                    let type_size = type_info.get_size();
                    let type_name = &type_info.name;
                    match &type_info.datatype {
                        DbgDataType::Uint8
                        | DbgDataType::Uint16
                        | DbgDataType::Uint32
                        | DbgDataType::Uint64
                        | DbgDataType::Sint8
                        | DbgDataType::Sint16
                        | DbgDataType::Sint32
                        | DbgDataType::Sint64
                        | DbgDataType::Float
                        | DbgDataType::Double
                        | DbgDataType::Array { .. }
                        | DbgDataType::Struct { .. } => {
                            let dim_type = self.get_dim_type(reg, type_info, object_type);
                            let _ = reg.instance_list.add_instance(a2l_name, dim_type, McSupportData::new(object_type), mc_addr);
                        }
                        _ => {
                            warn!("Variable '{}' has unsupported type: {:?}", var_name, &type_info.datatype);
                        }
                    }
                } else {
                    warn!("TypeRef {} of variable '{}' not found in debug info", var_info.typeref, var_name);
                }
            }
        } // var_infos
        Ok(())
    }
}

//------------------------------------------------------------------------
//  XCP client

async fn xcp_client(
    verbose: bool,
    tcp: bool,
    udp: bool,
    dest_addr: std::net::SocketAddr,
    local_addr: std::net::SocketAddr,
    a2l_name: String,
    upload_a2l: bool,
    create_a2l: bool,
    elf_name: String,
    list_cal: String,
    list_mea: String,
    measurement_list: Vec<String>,
    measurement_time_ms: u64,
    cal_args: Vec<String>,
) -> Result<(), Box<dyn Error>> {
    // Create xcp_client
    let mut xcp_client = XcpClient::new(tcp, dest_addr, local_addr);

    // Target ECU name
    let mut ecu_name = String::new();

    // A2L default name
    let a2l_default_name = "xcp_client".to_string();

    // Connect the XCP server if required
    let go_online = tcp || udp || upload_a2l || !measurement_list.is_empty() || !cal_args.is_empty();
    if go_online {
        // Connect to the XCP server
        info!("XCP Connect using {}", if tcp { "TCP" } else { "UDP" });
        let daq_decoder = Arc::new(Mutex::new(DaqDecoder::new()));
        xcp_client.connect(Arc::clone(&daq_decoder), ServTextDecoder::new()).await?;
        info!("XCP MAX_CTO = {}", xcp_client.max_cto_size);
        info!("XCP MAX_DTO = {}", xcp_client.max_dto_size);
        info!(
            "XCP RESOURCES = 0x{:02X} {} {} {} {}",
            xcp_client.resources,
            if (xcp_client.resources & 0x01) != 0 { "CAL" } else { "" },
            if (xcp_client.resources & 0x04) != 0 { "DAQ" } else { "" },
            if (xcp_client.resources & 0x10) != 0 { "PGM" } else { "" },
            if (xcp_client.resources & 0x40) != 0 { "STM" } else { "" }
        );
        info!("XCP COMM_MODE_BASIC = 0x{:02X}", xcp_client.comm_mode_basic);
        assert!((xcp_client.comm_mode_basic & 0x07) == 0); // Address granularity != 1 and motorola format not supported
        info!("XCP PROTOCOL_VERSION = 0x{:04X}", xcp_client.protocol_version);
        info!("XCP TRANSPORT_LAYER_VERSION = 0x{:04X}", xcp_client.transport_layer_version);
        info!("XCP DRIVER_VERSION = 0x{:02X}", xcp_client.driver_version);
        info!("XCP MAX_SEGMENTS = {}", xcp_client.max_segments);
        info!("XCP FREEZE_SUPPORTED = {}", xcp_client.freeze_supported);
        info!("XCP MAX_EVENTS = {}", xcp_client.max_events);

        // Get target name
        let res = xcp_client.get_id(XCP_IDT_ASCII).await;
        ecu_name = match res {
            Ok((_, Some(id))) => id,
            Err(e) => {
                panic!("GET_ID failed, Error: {}", e);
            }
            _ => {
                panic!("Empty string");
            }
        };
        info!("GET_ID XCP_IDT_ASCII = {}", ecu_name);

        // Get EPK
        let res = xcp_client.get_id(XCP_IDT_ASAM_EPK).await;
        let _ecu_epk = match res {
            Ok((_, Some(id))) => {
                info!("GET_ID IDT_EPK = {}", id);
                id
            }
            Err(e) => {
                warn!("GET_ID XCP_IDT_ASAM_EPK failed, Error: {}", e);
                "".into()
            }
            _ => {
                panic!("Empty string");
            }
        };
    } // go online

    // Create a new empty A2L registry
    let mut reg = xcp_lite::registry::Registry::new();
    if !ecu_name.is_empty() {
        reg.set_app_info(ecu_name.clone(), "-", 0);
    }

    // Set A2L default file path to given command line argument 'a2l' or target name 'ecu_name' if available
    let mut a2l_path = std::path::Path::new(if !a2l_name.is_empty() {
        &a2l_name
    } else if !ecu_name.is_empty() {
        &ecu_name
    } else {
        &a2l_default_name
    })
    .with_extension("a2l");

    // Upload A2L the file from XCP server and load it into the registry
    if upload_a2l {
        info!("Upload A2L file from XCP server");

        // Get A2L name from XCP server and adjust a2l_path construct from command line argument
        // If upload A2L is supported, ecu should provide the ASAM name with GET_ID XCP_IDT_ASAM_NAME command
        if a2l_name.is_empty() {
            let res = xcp_client.get_id(XCP_IDT_ASAM_NAME).await;
            match res {
                Ok((_, Some(id))) => {
                    info!("GET_ID XCP_IDT_ASAM_NAME = {}", id);
                    a2l_path = std::path::Path::new(&id).with_extension("a2l");
                }
                Err(e) => {
                    warn!("GET_ID XCP_IDT_ASAM_NAME failed, Error: {}", e);
                }
                _ => {
                    warn!("GET_ID XCP_IDT_ASAM_NAME returned empty string, using A2L name: {}", a2l_path.display());
                }
            };
        }

        // Upload A2L file
        info!("Uploading A2L file: {}", a2l_path.display());
        let res = xcp_client.upload_a2l(&a2l_path).await;
        if let Err(e) = res {
            error!("A2L upload failed, Error: {}", e);
            return Err("A2L upload failed".into());
        }
        info!("Uploaded A2L file: {}", a2l_path.display());

        // Read the A2L file into a registry
        // @@@@ TODO xcp_client does not support arrays, instances and typedefs yet, flatten the registry and mangle the names
        reg.load_a2l(&a2l_path, true, true, true, true)?;

    // If an ELF file is specified create an A2L file from the XCP server information and the ELF file
    // If option create-a2l is specified and no ELF file, create a A2L template from XCP server information
    // Read segment and event information obtained from the XCP server into registry
    // Add measurement and calibration variables from ELF file if specified
    } else if create_a2l || !elf_name.is_empty() {
        info!("Generate A2L file {} from XCP server event and segment information", a2l_path.display());

        reg.set_vector_xcp_mode(false); // Don't activate standard xcp-lite addressing modes and EPK segment
        //reg.set_app_version(epk, 0x80000000); // @@@@ TODO

        // Set registry XCP default transport layer informations
        let protocol = if tcp { "TCP" } else { "UDP" };
        let addr = dest_addr.ip();
        let ipv4_addr = match addr {
            std::net::IpAddr::V4(v4) => v4,
            std::net::IpAddr::V6(_) => Ipv4Addr::new(127, 0, 0, 1),
        };
        let port: u16 = dest_addr.port();
        reg.set_xcp_params(protocol, ipv4_addr, port);

        // Get event information
        for i in 0..xcp_client.max_events {
            let name = xcp_client.get_daq_event_info(i).await?;
            info!("Event {}: {}", i, name);
            reg.event_list.add_event(McEvent::new(name, 0, i, 0)).unwrap();
        }

        // Get segment and page information
        for i in 0..xcp_client.max_segments {
            let (addr_ext, addr, length, name) = xcp_client.get_segment_info(i).await?;
            info!("Segment {}: {} addr={}:0x{:08X} length={} ", i, name, addr_ext, addr, length);
            // Segment relative addressing
            // reg.cal_seg_list.add_cal_seg(name, i as u16, length as u32).unwrap();
            // Absolute addressing
            reg.cal_seg_list.add_cal_seg_by_addr(name, i as u16, addr_ext, addr, length as u32).unwrap();
        }

        // Read binary file if specified and create calibration variables in segments and all global measurement variables
        if !elf_name.is_empty() {
            info!("Reading ELF file: {}", elf_name);
            let elf_reader = ElfReader::new(&elf_name).ok_or(format!("Failed to read ELF file '{}'", elf_name))?;
            if verbose {
                printf_debug_info(&elf_reader.debug_data);
            } else {
                print_debug_stats(&elf_reader.debug_data);
            }
            elf_reader.register(&mut reg, verbose)?;
        }

        // Write the generated A2L file
        if !a2l_path.as_os_str().is_empty() {
            if a2l_path.exists() {
                warn!("Overwriting existing A2L file: {}", a2l_path.display());
            }
            if ecu_name.is_empty() {
                ecu_name = "_".into();
            }
            reg.write_a2l(&a2l_path, &ecu_name, "created by xcp_client", &ecu_name, "_", true).unwrap();
            info!("Created A2L file: {}", a2l_path.display());
        }
    }
    // Load existing A2L file into registry
    else {
        info!("Using existing A2L file: {} ({})", a2l_name, a2l_path.display());

        // @@@@ TODO xcp_client does not support arrays, instances and typedefs yet, flatten the registry and mangle the names
        reg.load_a2l(&a2l_path, true, true, true, true)?;
    }

    // Assign registry to xcp_client
    xcp_client.registry = Some(reg);

    // Print all known calibration objects and get their current value
    if !list_cal.is_empty() {
        println!();
        let cal_objects = xcp_client.find_characteristics(list_cal.as_str());
        println!("Calibration variables:");
        if !cal_objects.is_empty() {
            for name in &cal_objects {
                {
                    let h: XcpCalibrationObjectHandle = xcp_client.create_calibration_object(name).await?;
                    match xcp_client.get_calibration_object(h).get_a2l_type().encoding {
                        A2lTypeEncoding::Signed => {
                            let o = xcp_client.get_calibration_object(h);
                            print!(" {} {}:{:08X}", o.get_name(), o.get_a2l_addr().ext, o.get_a2l_addr().addr);
                            if xcp_client.is_connected() {
                                let v = xcp_client.get_value_i64(h);
                                print!(" ={}", v);
                            }
                        }
                        A2lTypeEncoding::Unsigned => {
                            let o = xcp_client.get_calibration_object(h);
                            print!(" {} {}:{:08X} ", o.get_name(), o.get_a2l_addr().ext, o.get_a2l_addr().addr);
                            if xcp_client.is_connected() {
                                let v = xcp_client.get_value_u64(h);
                                print!(" = {}", v);
                            }
                        }
                        A2lTypeEncoding::Float => {
                            let o = xcp_client.get_calibration_object(h);
                            print!(" {} {}:{:08X}", o.get_name(), o.get_a2l_addr().ext, o.get_a2l_addr().addr);
                            if xcp_client.is_connected() {
                                let v = xcp_client.get_value_f64(h);
                                print!(" = {}", v);
                            }
                        }
                        A2lTypeEncoding::Blob => {
                            print!(" {} = [...]", name);
                        }
                    }
                }
            }
            println!();
        } else {
            println!(" None");
        }
    }

    // Set calibration variable
    if xcp_client.is_connected() && !cal_args.is_empty() {
        if cal_args.len() != 2 {
            return Err("Calibration command requires exactly 2 arguments: variable name and value".into());
        }

        let var_name = &cal_args[0];
        let value_str = &cal_args[1];

        // Parse the value as a double
        let value: f64 = value_str.parse().map_err(|_| format!("Failed to parse '{}' as a double value", value_str))?;

        info!("Setting calibration variable '{}' to {}", var_name, value);

        // Create calibration object
        let handle = xcp_client
            .create_calibration_object(var_name)
            .await
            .map_err(|e| format!("Failed to create calibration object for '{}': {}", var_name, e))?;

        // Set the value using f64 (most calibration tools can handle type conversion)
        xcp_client
            .set_value_f64(handle, value)
            .await
            .map_err(|e| format!("Failed to set value for '{}': {}", var_name, e))?;

        info!("Successfully set '{}' = {}", var_name, value);
        println!("Ok");
    }

    // Print all known measurement objects
    if !list_mea.is_empty() {
        println!();
        let mea_objects = xcp_client.find_measurements(&list_mea);
        println!("Measurement variables:");
        if !mea_objects.is_empty() {
            for name in &mea_objects {
                if let Some(h) = xcp_client.create_measurement_object(name) {
                    let o = xcp_client.get_measurement_object(h);
                    println!(" {} {} {}", o.get_name(), o.get_a2l_addr(), o.get_a2l_type());
                }
            }
            println!();
        } else {
            println!(" None");
        }
    }

    // Measurement
    if xcp_client.is_connected() && !measurement_list.is_empty() {
        // Create list of measurement variable names
        let list = if measurement_list.len() == 1 {
            // Regular expression
            xcp_client.find_measurements(measurement_list[0].as_str())
        } else {
            // Just a list of names given on the command line
            measurement_list
        };
        if list.is_empty() {
            warn!("No measurement variables found");
        }
        // Start measurement
        else {
            // Create measurement objects for all names in the list
            // Multi dimensional objects not supported yet
            info!("Measurement list:");
            for name in &list {
                if let Some(o) = xcp_client.create_measurement_object(name) {
                    info!(r#"  {}: {}"#, o.0, name);
                }
            }

            // Measure for n seconds
            // 32 bit DAQ timestamp will overflow after 4.2s
            let start_time = tokio::time::Instant::now();
            xcp_client.start_measurement().await?;
            tokio::time::sleep(std::time::Duration::from_millis(measurement_time_ms)).await;
            xcp_client.stop_measurement().await?;
            let elapsed_time = start_time.elapsed().as_micros();

            // Print statistics from DAQ decoder
            {
                let daq_decoder = xcp_client.get_daq_decoder();
                if let Some(daq_decoder) = daq_decoder {
                    let daq_decoder = daq_decoder.lock();
                    let event_count = daq_decoder.get_event_count();
                    let byte_count = daq_decoder.get_byte_count();
                    info!(
                        "Measurement done, {} events, {:.0} event/s, {:.3} Mbytes/s",
                        event_count,
                        event_count as f64 * 1_000_000.0 / elapsed_time as f64,
                        byte_count as f64 / elapsed_time as f64
                    );
                }
            }
        }
    }

    // Disconnect
    if xcp_client.is_connected() {
        xcp_client.disconnect().await?;
        info!("XCP Disconnected");
    }

    Ok(())
}

//------------------------------------------------------------------------
// Main function

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    info!("xcp_client");

    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    let log_level = args.log_level.to_log_level_filter();
    env_logger::Builder::new()
        .target(env_logger::Target::Stdout)
        .filter_level(log_level)
        .format_timestamp(None)
        .format_module_path(false)
        .format_target(false)
        .init();

    // Parse IP addresses
    let dest_addr: std::net::SocketAddr = args.dest_addr.parse().map_err(|e| format!("{}", e))?;
    let local_addr: std::net::SocketAddr = args.bind_addr.parse().map_err(|e| format!("{}", e))?;
    info!("XCP server dest addr: {}", dest_addr);
    info!("XCP client local bind addr: {}", local_addr);

    // Run the test executor if --test is specified
    if args.test {
        test_executor(args.tcp, dest_addr, local_addr, TEST_CAL, TEST_DAQ, TEST_DURATION_MS).await
    }
    // Run the XCP client
    else {
        let res = xcp_client(
            args.verbose,
            args.tcp,
            args.udp,
            dest_addr,
            local_addr,
            args.a2l,
            args.upload_a2l,
            args.create_a2l,
            args.elf,
            args.list_cal,
            args.list_mea,
            args.mea,
            if args.time_ms > 0 { args.time_ms } else { args.time * 1000 },
            args.cal,
        )
        .await;
        if let Err(e) = res {
            error!("XCP client failed, Error: {}", e);
        }
    }

    Ok(())
}
