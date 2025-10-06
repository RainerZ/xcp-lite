// Taken from Github repository a2ltool by DanielT

use indexmap::IndexMap;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::Display;

mod dwarf;

#[derive(Debug)]
pub(crate) struct VarInfo {
    pub(crate) address: u64,
    pub(crate) typeref: usize,
    pub(crate) unit_idx: usize,
    pub(crate) function: Option<String>,
    pub(crate) namespaces: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct TypeInfo {
    pub(crate) name: Option<String>, // not all types have a name
    pub(crate) unit_idx: usize,
    pub(crate) datatype: DbgDataType,
    pub(crate) dbginfo_offset: usize,
}

#[derive(Debug, Clone)]
pub(crate) enum DbgDataType {
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Sint8,
    Sint16,
    Sint32,
    Sint64,
    Float,
    Double,
    Bitfield {
        basetype: Box<TypeInfo>,
        bit_offset: u16,
        bit_size: u16,
    },
    Pointer(u64, usize),
    Struct {
        size: u64,
        members: IndexMap<String, (TypeInfo, u64)>,
    },
    Class {
        size: u64,
        inheritance: IndexMap<String, (TypeInfo, u64)>,
        members: IndexMap<String, (TypeInfo, u64)>,
    },
    Union {
        size: u64,
        members: IndexMap<String, (TypeInfo, u64)>,
    },
    Enum {
        size: u64,
        signed: bool,
        enumerators: Vec<(String, i64)>,
    },
    Array {
        size: u64,
        dim: Vec<u64>,
        stride: u64,
        arraytype: Box<TypeInfo>,
    },
    TypeRef(usize, u64), // dbginfo_offset of the referenced type
    FuncPtr(u64),
    Other(u64),
}

#[derive(Debug)]
pub(crate) struct DebugData {
    pub(crate) variables: IndexMap<String, Vec<VarInfo>>,
    pub(crate) types: HashMap<usize, TypeInfo>,
    pub(crate) typenames: HashMap<String, Vec<usize>>,
    pub(crate) demangled_names: HashMap<String, String>,
    pub(crate) unit_names: Vec<Option<String>>,
    pub(crate) sections: HashMap<String, (u64, u64)>,
}

impl DebugData {
    // load the debug info from an elf file
    pub(crate) fn load_dwarf(filename: &OsStr, verbose: bool) -> Result<Self, String> {
        dwarf::load_dwarf(filename, verbose)
    }
}

impl TypeInfo {
    //const MAX_RECURSION_DEPTH: usize = 5;

    pub(crate) fn get_size(&self) -> u64 {
        match &self.datatype {
            DbgDataType::Uint8 => 1,
            DbgDataType::Uint16 => 2,
            DbgDataType::Uint32 => 4,
            DbgDataType::Uint64 => 8,
            DbgDataType::Sint8 => 1,
            DbgDataType::Sint16 => 2,
            DbgDataType::Sint32 => 4,
            DbgDataType::Sint64 => 8,
            DbgDataType::Float => 4,
            DbgDataType::Double => 8,
            DbgDataType::Bitfield { basetype, .. } => basetype.get_size(),
            DbgDataType::Pointer(size, _)
            | DbgDataType::Other(size)
            | DbgDataType::Struct { size, .. }
            | DbgDataType::Class { size, .. }
            | DbgDataType::Union { size, .. }
            | DbgDataType::Enum { size, .. }
            | DbgDataType::Array { size, .. }
            | DbgDataType::FuncPtr(size)
            | DbgDataType::TypeRef(_, size) => *size,
        }
    }
}

impl Display for TypeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.datatype {
            DbgDataType::Uint8 => f.write_str("Uint8"),
            DbgDataType::Uint16 => f.write_str("Uint16"),
            DbgDataType::Uint32 => f.write_str("Uint32"),
            DbgDataType::Uint64 => f.write_str("Uint64"),
            DbgDataType::Sint8 => f.write_str("Sint8"),
            DbgDataType::Sint16 => f.write_str("Sint16"),
            DbgDataType::Sint32 => f.write_str("Sint32"),
            DbgDataType::Sint64 => f.write_str("Sint64"),
            DbgDataType::Float => f.write_str("Float"),
            DbgDataType::Double => f.write_str("Double"),
            DbgDataType::Bitfield { .. } => f.write_str("Bitfield"),
            DbgDataType::Pointer(_, _) => write!(f, "Pointer(...)"),
            DbgDataType::Other(osize) => write!(f, "Other({osize})"),
            DbgDataType::FuncPtr(osize) => write!(f, "function pointer({osize})"),
            DbgDataType::Struct { members, .. } => {
                if let Some(name) = &self.name {
                    write!(f, "Struct {name}({} members)", members.len())
                } else {
                    write!(f, "Struct <anonymous>({} members)", members.len())
                }
            }
            DbgDataType::Class { members, .. } => {
                if let Some(name) = &self.name {
                    write!(f, "Class {name}({} members)", members.len())
                } else {
                    write!(f, "Class <anonymous>({} members)", members.len())
                }
            }
            DbgDataType::Union { members, .. } => {
                if let Some(name) = &self.name {
                    write!(f, "Union {name}({} members)", members.len())
                } else {
                    write!(f, "Union <anonymous>({} members)", members.len())
                }
            }
            DbgDataType::Enum { enumerators, .. } => {
                if let Some(name) = &self.name {
                    write!(f, "Enum {name}({} enumerators)", enumerators.len())
                } else {
                    write!(f, "Enum <anonymous>({} enumerators)", enumerators.len())
                }
            }
            DbgDataType::Array { dim, arraytype, .. } => {
                write!(f, "Array({dim:?} x {arraytype})")
            }
            DbgDataType::TypeRef(t_ref, _) => write!(f, "TypeRef({t_ref})"),
        }
    }
}

#[cfg(test)]
mod test {}
