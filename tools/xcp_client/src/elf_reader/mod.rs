use indexmap::IndexMap;
use std::error::Error;
use std::ffi::OsStr;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use xcp_lite::registry::{McAddress, McDimType, McEvent, McObjectType, McSupportData, McValueType, Registry};

/*
Which information can be detected from ELF/DWARF:
- Events:
    name, compilation unit, function name and CFA offset, but index is unknown
- Memory segment name, type (naming convention name = reference page), address, length, but number is unknown
- Variables:
    variable name, typename, absolute address, frame offset, compilation unit, function name, namespace
    static variables in functions get the correct event
    local variables on stack get the correct CFA
    name, type, compilation unit, namespace, location (register or stack)
- Types:
    typedefs, structs, enums
    basic types: int8/16/32/64, uint8/16/32/64, float, double
    arrays 1D and 2D
    pointers (as ulong or ulonglong)

    Key benefits:
    - Instance names get prefixed with function name if local stack or static variables
    - All instances get the correct fixed event id, if there is one in their scope, default is event id 0
    - Event compilation unit, function and CFA is detected to enable local variable access

    Todo:
    - test arrays and nested structs

    - No DW_AT_location means optimized away

Detect TLS Variables:

TLS Variables:
Check for missing DW_AT_location + thread-local context
Look for variables referencing .tdata/.tbss sections
Parse DW_TAG_variable with TLS-specific location expressions
DW_OP_form_tls_address, etc





Tools:
dwarfdump --debug-info <filename>
dwarfdump --debug-info --name <varname> <filename>
objdump -h  <filename>
objdump --syms <filename>



Limitations:
- With -o1 most stack variables are in registers, have to be manually spilled to stack or captured
- Segment numbers and event index are not constant expressions, need to be read by XCP (current solution) or from the binary persistence file from the target

Possible future improvements:
- Thread load addressing mode
- C++ support,  this addressing support, namespaces
- Measurement of variables and function parameters in registers
- Just in time compilation of variable access expressions



*/

// Dwarf reader
// This module contains modified code adapted from https://github.com/DanielT/a2ltool
// Original code licensed under MIT/Apache-2.0
// Copyright (c) DanielT
mod debuginfo;
use debuginfo::{DbgDataType, DebugData, TypeInfo};

//------------------------------------------------------------------------
//  ELF reader and A2L creator

pub(crate) struct ElfReader {
    pub(crate) debug_data: DebugData,
}

impl ElfReader {
    pub fn new(file_name: &str, verbose: usize, unit_idx_limit: usize) -> Option<ElfReader> {
        // Load debug information from the ELF file
        info!("Loading debug information from ELF file: {}", file_name);
        let debug_data = DebugData::load_dwarf(OsStr::new(file_name), verbose, unit_idx_limit);
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

            // From CalSegCreate macro
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

                // remove the "cal__" prefix to get the segment name
                let seg_name = var_name.strip_prefix("cal__").unwrap_or(var_name);
                info!(
                    "Calibration segment definition marker variable 'cal__{}' for segment '{}' found in {}:'{}'",
                    seg_name, seg_name, unit_name, function_name
                );

                // Lookup the reference page variable information to determine addr
                let seg_var_info = if let Some(x) = self.debug_data.variables.get(seg_name) {
                    if x.len() != 1 {
                        error!("Calibration segment reference page variable '{}' has {} definitions, expected 1", seg_name, x.len());
                        continue;
                    }
                    &x[0]
                } else {
                    error!("Could not find calibration segment reference page variable '{}'", seg_name);
                    continue;
                };
                let addr: u32 = seg_var_info.address.1.try_into().unwrap(); // @@@@ TODO: Handle 64 bit addresses and signed relative 
                let addr_ext: u8 = seg_var_info.address.0;
                info!("  Segment '{}' default page variable '()' found in debug data:", seg_name);
                info!("    Address = {}:{:#x}", addr_ext, addr);

                // Lookup the reference page variable type to determine segment length
                let length = if let Some(var_info) = self.debug_data.variables.get(seg_name) {
                    if let Some(type_info) = self.debug_data.types.get(&var_info[0].typeref) {
                        info!(
                            "  Segment '{}' type information found, type={}, length = {}",
                            seg_name,
                            type_info.name.as_ref().map_or("<unnamed>", |s| s.as_str()),
                            type_info.get_size()
                        );
                        if verbose {
                            self.debug_data.print_type_info(type_info);
                        }
                        type_info.get_size()
                    } else {
                        warn!("Could not determine length type for segment {}", seg_name);
                        0
                    }
                } else {
                    warn!("Could not find calibration segment reference page variable {}", seg_name);
                    0
                };
                info!("    Length = {:#x}", length);

                // Check for valid address and length
                if length > 0 && addr > 0 && addr_ext == 0 {
                } else {
                    error!(
                        "Calibration segment from cal_<name> '{}' not found, has invalid address {:#x} or length {:#x}, skipped",
                        seg_name, addr, length
                    );
                    continue; // skip this variable
                }

                // Find the segment in the registry
                if let Some(reg_seg) = reg.cal_seg_list.find_cal_seg(seg_name) {
                    // Check if address and length match
                    info!("Calibration segment '{}' already exists in registry, checking length and address", seg_name);
                    if reg_seg.addr == addr && reg_seg.size == length as u32 {
                        info!("Calibration segment '{}' matches existing registry entry", seg_name);
                    } else {
                        warn!("Calibration segment '{}' does not match existing registry entry, registry information updated", seg_name);
                        unimplemented!();
                    }
                    continue; // segment already exists, leave it as it is
                }
                // If not create it
                else {
                    reg.cal_seg_list
                        .add_cal_seg_by_addr(seg_name.to_string(), next_segment_number, addr_ext, addr, length as u32)
                        .unwrap();
                    info!(
                        "Not yet defined segment '{}':  Created with number={}, addr = {:#x}, length = {:#x}",
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
                let evt_mode = parts.next().unwrap_or("");
                let evt_name = parts.next().unwrap_or("");

                let evt_unit_idx = var_infos[0].unit_idx;
                let evt_unit_name = if let Some(name) = debuginfo::make_simple_unit_name(&self.debug_data, evt_unit_idx) {
                    name
                } else {
                    format!("{evt_unit_idx}")
                };
                let evt_function = if let Some(f) = var_info.function.as_ref() { f.as_str() } else { "" };
                info!("Event {} trigger found in {}:{}, address resolver mode {}", evt_name, evt_unit_name, evt_function, evt_mode);

                // Find the event in the registry
                if let Some(_evt) = reg.event_list.find_event(evt_name, 0) {
                    // Try to lookup the canonical stack frame address offset from the function name
                    let mut evt_cfa: i32 = 0;
                    for cfa_info in self.debug_data.cfa_info.iter() {
                        if cfa_info.unit_idx == evt_unit_idx && cfa_info.function == evt_function {
                            if let Some(x) = cfa_info.cfa_offset {
                                evt_cfa = x as i32;
                            } else {
                                warn!("Could not determine CFA offset for function '{}'", evt_function);
                            }
                            break;
                        }
                    }

                    if verbose {
                        info!("  Event '{}' trigger in function '{}', cfa = {}", evt_name, evt_function, evt_cfa);
                    }

                    // Store the unit and function name and canonical stack frame address offset for this event trigger

                    match reg.event_list.set_event_location(evt_name, evt_unit_idx, evt_function, evt_cfa) {
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

    pub fn register_variables(&self, reg: &mut Registry, verbose: bool, unit_idx_limit: usize) -> Result<(), Box<dyn Error>> {
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

            // Count variables with this name in compilation unit 0
            let count = var_infos.iter().filter(|v| v.unit_idx <= unit_idx_limit).count();

            // Process all variable with this name in different scopes and namesspaces
            for var_info in var_infos {
                // @@@@ TODO: Create only variables from specified compilation unit
                if var_info.unit_idx > unit_idx_limit {
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
                        // find an event triggered in this function
                        if let Some(event) = reg.event_list.find_event_by_location(var_info.unit_idx, var_function) {
                            xcp_event_id = event.id;
                            info!("Variable '{}' is local to function '{}', using event id = {}", var_name, var_function, xcp_event_id);
                        } else {
                            debug!("Variable '{}' is local to function '{}', but no event found", var_name, var_function);
                        }
                        // multiple variables with this name, prefix with function name
                        if count > 1 {
                            a2l_name = format!("{}.{}", var_function, var_name);
                        }
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
                        let cfa: i64 = event.cfa as i64;
                        a2l_name = format!("{}.{}", var_function, var_name);
                        debug!(
                            "Variable '{}' is local to function '{}', using event id = {}, dwarf_offset = {} cfa = {}",
                            var_name,
                            var_function,
                            xcp_event_id,
                            (var_info.address.1 as i64 - 0x80000000) as i64,
                            cfa
                        );
                        // Encode dyn addressing mode from signed offset and event id
                        let offset: i16 = (var_info.address.1 as i64 - 0x80000000 + cfa).try_into().unwrap();
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
                            info!(
                                "Add {} instance for {}: addr = {}:0x{:08x}",
                                if object_type == McObjectType::Characteristic { "characteristic" } else { "measurement" },
                                a2l_name,
                                a2l_addr_ext,
                                a2l_addr
                            );
                            if verbose {
                                self.debug_data.print_type_info(type_info);
                            }
                            let dim_type = self.get_dim_type(reg, type_info, object_type);
                            let res = reg.instance_list.add_instance(a2l_name.clone(), dim_type, McSupportData::new(object_type), mc_addr);
                            match res {
                                Ok(_) => {
                                    if verbose {
                                        info!(
                                            "  Registered variable '{}' with type '{}', size = {}, event id = {}",
                                            a2l_name,
                                            type_name.as_ref().unwrap_or(&"<unnamed>".to_string()),
                                            type_size,
                                            xcp_event_id
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to register variable '{}': {}", a2l_name, e);
                                }
                            }
                        }
                        _ => {
                            warn!("Variable '{}' has unsupported type: {:?}", var_name, &type_info.datatype);
                            self.debug_data.print_type_info(type_info);
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
