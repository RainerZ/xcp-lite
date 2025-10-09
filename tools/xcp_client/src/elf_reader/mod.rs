use indexmap::IndexMap;
use std::error::Error;
use std::ffi::OsStr;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use xcp_lite::registry::{McAddress, McDimType, McEvent, McObjectType, McSupportData, McValueType, Registry};

// Dwarf reader
// This module contains code adapted from https://github.com/DanielT/a2ltool
// Original code licensed under MIT/Apache-2.0
// Copyright (c) DanielT
mod debuginfo;
use debuginfo::{DbgDataType, DebugData, TypeInfo};
mod csa;

/*

Function #1: main
  Compilation Unit: 0
  Address Range: 0x00002054 - 0x00002460 (size: 1036 bytes)
  CFA Offset: 96 (0x60)
  Local variables are likely at: CFA + 96 + variable_offset

Function #2: foo
  Compilation Unit: 0
  Address Range: 0x00001e5c - 0x00002054 (size: 504 bytes)
  CFA Offset: 128 (0x80)
  Local variables are likely at: CFA + 128 + variable_offset

Function #3: task
  Compilation Unit: 0
  Address Range: 0x00001c74 - 0x00001e5c (size: 488 bytes)
  CFA Offset: 80 (0x50)
  Local variables are likely at: CFA + 80 + variable_offset

*/

//------------------------------------------------------------------------
//  ELF reader and A2L creator

fn print_debug_stats(debug_data: &DebugData) {
    println!("\nDebug information summary:");
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

    //Print compilation units
    println!("\nCompilation Units:");
    for (idx, unit_name) in debug_data.unit_names.iter().enumerate() {
        let unit_name = debuginfo::make_simple_unit_name(debug_data, idx);
        if unit_name.is_none() {
            println!("  Unit {}: <unnamed>", idx);
        } else {
            println!("  Unit {}: {}", idx, unit_name.as_ref().unwrap());
        }
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

pub struct ElfReader {
    debug_data: DebugData,
}

impl ElfReader {
    pub fn new(file_name: &str) -> Option<ElfReader> {
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

    pub fn printf_debug_info(&self, verbose: bool) {
        print_debug_stats(&self.debug_data);

        //Print sections information
        println!("\nMemory Sections in debug_data:");
        for (name, (addr, size)) in &self.debug_data.sections {
            println!("  '{}': 0x{:08x}, {} bytes", name, addr, size);
        }

        if verbose {
            //Print type names
            println!("\nType Names (debug_data.typenames)");
            for (type_name, type_refs) in &self.debug_data.typenames {
                println!("Type name '{}': {} references", type_name, type_refs.len());
                for type_ref in type_refs {
                    if let Some(type_info) = self.debug_data.types.get(type_ref) {
                        println!("  -> type_ref={}, size={} bytes, unit={}", type_ref, type_info.get_size(), type_info.unit_idx);
                    }
                }
            }

            // Print types
            println!("\nTypes:");
            for (type_ref, type_info) in &self.debug_data.types {
                let type_name = if let Some(name) = &type_info.name { name } else { "" };
                println!(
                    "TypeRef {}: name = '{}', size = {} bytes, unit = {}",
                    type_ref,
                    type_name,
                    type_info.get_size(),
                    type_info.unit_idx
                );
                print_type_info(type_info);
            }

            // Print demangled names
            println!("\nDemangled Names");
            for (mangled_name, demangled_name) in &self.debug_data.demangled_names {
                println!("  '{}' -> '{}'", mangled_name, demangled_name);
            }
        }

        // Print variables
        let unit_idx = 0; // print only variables <= compilation unit 0
        println!("\nVariables in compilation unit 0..{unit_idx}:");
        for (var_name, var_info) in &self.debug_data.variables {
            // Count all variable in unit_idx
            let count = var_info.iter().filter(|v| v.unit_idx <= unit_idx).count();
            if count > 1 {
                println!("{} : ", var_name);
            }
            // Iterate over all variable infos for this variable name in unit_idx
            for var in var_info {
                if var.unit_idx > unit_idx {
                    continue; // print only variables from compilation unit 0..=unit_idx
                }
                if count <= 1 {
                    print!("{} : ", var_name);
                }

                let unit_name = if let Some(name) = debuginfo::make_simple_unit_name(&self.debug_data, var.unit_idx) {
                    name
                } else {
                    "<unnamed>".to_string()
                };
                let function_name = if let Some(name) = &var.function { name } else { "<global>" };
                let name_space = if var.namespaces.len() > 0 { var.namespaces.join("::") } else { "".to_string() };
                print!(" {}:'{}' {}: addr={}:0x{:08X}", unit_name, function_name, name_space, var.address.0, var.address.1);
                if let Some(type_info) = self.debug_data.types.get(&var.typeref) {
                    let type_name = if let Some(name) = &type_info.name { name } else { "" };
                    print!(", type='{}', size={}", type_name, type_info.get_size());
                    // print_type_info(type_info);
                }
                println!();
            }
        }

        println!();
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

    pub fn register_segments_and_events(&self, reg: &mut Registry, verbose: bool) -> Result<(), Box<dyn Error>> {
        info!("Registering segment and event information");

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
                assert!(var_infos.len() == 1); // @@@@ Only one definition allowed
                let var_info = &var_infos[0];
                let function_name = if let Some(f) = var_info.function.as_ref() { f.as_str() } else { "" };
                let unit_idx = var_info.unit_idx;
                let unit_name = if let Some(name) = debuginfo::make_simple_unit_name(&self.debug_data, unit_idx) {
                    name
                } else {
                    format!("{unit_idx}")
                };

                // remove the "cal__" prefix
                let seg_name = var_name.strip_prefix("cal__").unwrap_or(var_name);
                info!("Calibration segment definition '{}' found in {}:{}", seg_name, unit_name, function_name);
                // Find the segment in the registry
                if let Some(_seg) = reg.cal_seg_list.find_cal_seg(seg_name) {
                    continue; // segment already exists
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
                    let addr: u32 = var_info.address.1.try_into().unwrap(); // @@@@ TODO: Handle 64 bit addresses and signed relative 
                    let addr_ext: u8 = var_info.address.0;
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
                let evt_unit_idx = var_infos[0].unit_idx;
                let evt_unit_name = if let Some(name) = debuginfo::make_simple_unit_name(&self.debug_data, evt_unit_idx) {
                    name
                } else {
                    format!("{evt_unit_idx}")
                };
                let evt_function = if let Some(f) = var_infos[0].function.as_ref() { f.as_str() } else { "" };
                info!("Event definition for event '{}' found in {}:{}", evt_name, evt_unit_name, evt_function);
                // Find the event in the registry
                if let Some(_evt) = reg.event_list.find_event(evt_name, 0) {
                    continue; // event already exists
                } else {
                    // @@@@ TODO: Event number unknown !!!!!!!!!!!!!!!
                    reg.event_list.add_event(McEvent::new(evt_name.to_string(), 0, next_event_id, 0)).unwrap();
                    error!("Unknown event '{}': Created with event id = {}", evt_name, next_event_id);
                    next_event_id += 1;
                    continue; // skip this variable
                }
            }
        }
        Ok(())
    }

    pub fn register_event_locations(&self, reg: &mut Registry, verbose: bool) -> Result<(), Box<dyn Error>> {
        info!("Registering event locations");

        // Iterate over variables
        for (var_name, var_infos) in &self.debug_data.variables {
            // Skip standard library variables and system/compiler internals (__<name>)s
            // Skip global XCP variables (gXCP.. and gA2L..)
            if var_name.starts_with("__") || var_name.starts_with("gXcp") || var_name.starts_with("gA2l") {
                continue;
            }

            // trg__<event_name> (thread local static, name is event name)
            // Event definitions (thread local static variables)
            if var_name.starts_with("trg__") {
                assert!(var_infos.len() == 1); // Only one definition allowed
                let var_info = &var_infos[0];

                // Get the event name from format  "trg__<tag>__<eventname>" prefix
                let s = var_name.strip_prefix("trg__").unwrap_or("unnamed");
                let mut parts = s.split("__");
                let evt_tag = parts.next().unwrap_or("");
                let evt_name = parts.next().unwrap_or("");

                let evt_unit_idx = var_infos[0].unit_idx;
                let evt_unit_name = if let Some(name) = debuginfo::make_simple_unit_name(&self.debug_data, evt_unit_idx) {
                    name
                } else {
                    format!("{evt_unit_idx}")
                };
                let evt_function = if let Some(f) = var_info.function.as_ref() { f.as_str() } else { "" };
                info!("Event {} trigger found in {}:{}", evt_name, evt_unit_name, evt_function);

                // Find the event in the registry
                if let Some(_evt) = reg.event_list.find_event(evt_name, 0) {
                    // Store the unit and function name and cananical stack frame address offset for this event trigger
                    let evt_csa: i32 = 0; // @@@@ TODO: Get from variable info ????
                    match reg.event_list.set_event_location(evt_name, evt_unit_idx, evt_function, evt_csa) {
                        Ok(_) => {}
                        Err(e) => {
                            error!("Failed to set event location for event '{}': {}", evt_name, e);
                        }
                    }
                } else {
                    error!("Event '{}' for trigger not found in registry", evt_name);
                }
                continue; // skip this variable
            }
        }
        Ok(())
    }

    pub fn register_variables(&self, reg: &mut Registry, verbose: bool) -> Result<(), Box<dyn Error>> {
        // Load debug information from the ELF file
        info!("Registering variables");

        // Iterate over variables
        for (var_name, var_infos) in &self.debug_data.variables {
            // Skip standard library variables and system/compiler internals (__<name>)s
            // Skip global XCP variables (gXCP.. and gA2L..) and special marker variables (cal__, evt__, trg__)
            if var_name.starts_with("__")
                || var_name.starts_with("gXcp")
                || var_name.starts_with("gA2l")
                || var_name.starts_with("cal__")
                || var_name.starts_with("evt__")
                || var_name.starts_with("trg__")
            {
                continue;
            }

            if var_infos.is_empty() {
                warn!("Variable '{}' has no variable info", var_name);
            }

            let mut a2l_name = var_name.to_string();
            let mut xcp_event_id = 0; // default event id is 0, async event in transmit thread

            // daq__<event_name>__<var_name> (local scope static variables)
            // Check for captured variables with format "daq__<event_name>__<var_name>"
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

            // Process all variable with this name in different scopes and namesspaces
            for var_info in var_infos {
                // @@@@ TODO: Create only variables from specified compilation unit
                if var_info.unit_idx != 0 {
                    continue;
                }

                // @@@@ Create an option for this
                // Register only global variables
                // if var_info.address.0 != 0 || var_info.address.1 == 0 {
                //     continue;
                // }

                let var_function = if let Some(f) = var_info.function.as_ref() { f.as_str() } else { "" };

                // Address encoder
                let a2l_addr_ext: u8 = var_info.address.0;
                let a2l_addr: u32 = if a2l_addr_ext == 0 {
                    // Encode absolute addressing mode
                    if var_info.address.1 == 0 {
                        error!("Variable '{}' in function '{}' skipped, no address", var_name, var_function);
                        continue; // skip this variable
                    } else if var_info.address.1 >= 0xFFFFFFFF {
                        error!(
                            "Variable '{}' skipped, has 64 bit address {:#x}, which does not fit the 32 bit XCP address range",
                            var_name, var_info.address.1
                        );
                        continue; // skip this variable
                    } else {
                        var_info.address.1 as u32
                    }
                }
                // Encode relative addressing mode
                else if a2l_addr_ext == 2 {
                    // Find an event id for this local variable
                    if let Some(event) = reg.event_list.find_event_by_location(var_info.unit_idx, var_function) {
                        // Set the event id for this function
                        // Prefix the variable with the function name
                        xcp_event_id = event.id;
                        let csa: i64 = event.csa as i64;
                        a2l_name = format!("{}.{}", var_function, var_name);
                        debug!(
                            "Variable '{}' is local to function '{}', using event id = {}, dwarf_offset = {} csa = {}",
                            var_name,
                            var_function,
                            xcp_event_id,
                            (var_info.address.1 as i64 - 0x80000000) as i64,
                            csa
                        );
                        // Encode dyn addressing mode from signed offset and event id
                        let offset: i16 = (var_info.address.1 as i64 - 0x80000000 + csa).try_into().unwrap();
                        ((offset as u32) & 0xFFFF) | ((event.id as u32) << 16)
                    } else {
                        error!("Variable '{}' skipped, could not find event for dyn addressing mode", var_name);
                        continue;
                    }
                }
                // @@@@ TODO: Handle other address extensions
                else {
                    error!("Variable '{}' skipped, has unsupported address extension {:#x}", var_name, a2l_addr_ext);
                    continue; // skip this variable
                };

                // Check if the absolute address is in a calibration segment
                // Create a McAddress with or without event id
                // @@@@ TODO event id ?????
                let (object_type, mc_addr) = if reg.cal_seg_list.find_cal_seg_by_address(a2l_addr).is_some() {
                    (McObjectType::Characteristic, McAddress::new_a2l(a2l_addr, a2l_addr_ext))
                } else {
                    (McObjectType::Measurement, McAddress::new_a2l_with_event(xcp_event_id, a2l_addr, a2l_addr_ext))
                };

                // Register measurement variable if possible
                if let Some(type_info) = self.debug_data.types.get(&var_info.typeref) {
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
                            info!("Add instance for {}: addr = {}:0x{:08x}", a2l_name, a2l_addr_ext, a2l_addr);
                            if verbose {
                                print_type_info(type_info);
                            }
                            let dim_type = self.get_dim_type(reg, type_info, object_type);
                            let _ = reg.instance_list.add_instance(a2l_name.clone(), dim_type, McSupportData::new(object_type), mc_addr);
                        }
                        _ => {
                            warn!("Variable '{}' has unsupported type: {:?}", var_name, &type_info.datatype);
                            print_type_info(type_info);
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
