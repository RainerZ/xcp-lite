// Module mc_value
// Types:
//  McValueType Copy, Clone
//  McDimType Clone (which is a copy)
//  McValueTypeTrait

use serde::Deserialize;
use serde::Serialize;

use super::McIdentifier;
use super::McText;

/// Dimensional type with meta data
/// Used to describe the type of a variable and its meta data
/// May be a scalar, an array [x_dim] or a matrix [x_dim][y_dim] of its basic type
/// The basic type may be a scalar (u8,u16,...,f64), a binary block blob or a reference to a typedef
/// May have meta data for Measurement and Characteristic objects
/// May refer to axis objects
/// May have a conversion rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McDimType {
    pub value_type: McValueType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_dim: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_dim: Option<u16>,
}

impl McDimType {
    /// Type with meta data and dimensions
    pub fn new(value_type: McValueType, x_dim: u16, y_dim: u16) -> Self {
        McDimType {
            value_type,
            x_dim: if x_dim <= 1 { None } else { Some(x_dim) },
            y_dim: if y_dim <= 1 { None } else { Some(y_dim) },
        }
    }

    /// Categorize the value type
    pub fn is_basic_type(&self) -> bool {
        !matches!(self.value_type, McValueType::Blob(_) | McValueType::TypeDef(_))
    }

    /// Categorize the value type
    pub fn is_blob(&self) -> bool {
        if let McValueType::Blob(_) = self.value_type {
            return true;
        }
        false
    }

    /// Categorize the value type
    pub fn is_typedef(&self) -> bool {
        if let McValueType::TypeDef(_) = self.value_type {
            return true;
        }
        false
    }

    /// No dimension
    pub fn is_scalar(&self) -> bool {
        let x_dim = self.y_dim.unwrap_or(1);
        let y_dim = self.y_dim.unwrap_or(1);
        x_dim <= 1 && y_dim <= 1
    }
    /// One dimension
    pub fn is_array(&self) -> bool {
        let x_dim = self.y_dim.unwrap_or(1);
        let y_dim = self.y_dim.unwrap_or(1);
        x_dim > 1 && y_dim <= 1
    }
    /// Two dimensions
    pub fn is_matrix(&self) -> bool {
        let x_dim = self.y_dim.unwrap_or(1);
        let y_dim = self.y_dim.unwrap_or(1);
        x_dim > 1 && y_dim > 1
    }

    /// Dimension as 2 dimensional array
    /// If the dimension is not defined, it is set to 1
    /// If the dimension is variable, it is set to 0
    pub fn get_dim(&self) -> [u16; 2] {
        [self.x_dim.unwrap_or(1), self.y_dim.unwrap_or(1)]
    }

    /// Get memory size in bytes
    pub fn get_size(&self) -> usize {
        self.value_type.get_size() * self.get_dim()[0] as usize * self.get_dim()[1] as usize
    }
}

impl std::fmt::Display for McDimType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)?;
        Ok(())
    }
}

/// Basic array type
/// 2 dimensional array of a basic scalar type or a typedef
///
/// Basic value type
/// May be a scalar type or may be an instance of a typedef
/// Special case is Blob, which functionality might ve realized in some other way in future
/// McValueType is copy
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum McValueType {
    Unknown,
    Bool,
    Ubyte,
    Uword,
    Ulong,
    Ulonglong,
    Sbyte,
    Sword,
    Slong,
    Slonglong,
    Float32Ieee,
    Float64Ieee,
    Blob(McText),          // IDL for this type, type is !sized
    TypeDef(McIdentifier), // McIdentifier of a type definition in TypeDefList
}

impl McValueType {
    // McValueType::TypeDef
    pub fn new_typedef<T: Into<McIdentifier>>(name: T) -> Self {
        let name: McIdentifier = name.into();
        McValueType::TypeDef(name)
    }

    // McValueType::Blob
    pub fn new_blob<T: Into<McText>>(text: T) -> Self {
        let text: McText = text.into();
        McValueType::Blob(text)
    }

    /// Get minimum value for data type
    /// Used by the register macros
    pub fn get_min(&self) -> Option<f64> {
        match self {
            McValueType::Bool => Some(0.0),
            McValueType::Sbyte => Some(i8::MIN as f64),
            McValueType::Sword => Some(i16::MIN as f64),
            McValueType::Slong => Some(i32::MIN as f64),
            McValueType::Slonglong => Some(i64::MIN as f64),
            McValueType::Float32Ieee | McValueType::Float64Ieee => Some(-1E32),
            McValueType::Ubyte => Some(0.0),
            McValueType::Uword => Some(0.0),
            McValueType::Ulong => Some(0.0),
            McValueType::Ulonglong => Some(0.0),
            _ => {
                //log::warn!("get_min: Unsupported data type {:?}", self);
                None
            }
        }
    }

    /// Get maximum value for data type
    /// Used by the register macros
    pub fn get_max(&self) -> Option<f64> {
        match self {
            McValueType::Ubyte => Some(u8::MAX as f64),
            McValueType::Sbyte => Some(i8::MAX as f64),
            McValueType::Uword => Some(u16::MAX as f64),
            McValueType::Sword => Some(i16::MAX as f64),
            McValueType::Ulong => Some(u32::MAX as f64),
            McValueType::Slong => Some(i32::MAX as f64),
            McValueType::Ulonglong => Some(u64::MAX as f64), // converting u64::MAX to f64 results in a loss of precision, and the resulting f64 value is slightly higher than the original u64 value
            McValueType::Slonglong => Some(i64::MAX as f64),
            McValueType::Float32Ieee => Some(1E32),
            McValueType::Float64Ieee => Some(1E32),
            McValueType::Bool => Some(1.0),
            _ => {
                //log::warn!("get_max: Unsupported data type {:?}", self);
                None
            }
        }
    }

    // Get data type size
    // Used by the register macros
    pub fn get_size(&self) -> usize {
        match self {
            McValueType::Ubyte | McValueType::Sbyte | McValueType::Bool => 1,
            McValueType::Uword | McValueType::Sword => 2,
            McValueType::Ulong | McValueType::Slong | McValueType::Float32Ieee => 4,
            McValueType::Ulonglong | McValueType::Slonglong | McValueType::Float64Ieee => 8,
            McValueType::Blob(_) => panic!("get_size: Unknown blob size"),
            McValueType::TypeDef(_) => panic!("get_size: Unknown instance size"),
            _ => panic!("get_size: Unsupported data type"),
        }
    }

    // Convert from Rust basic type as str
    // Used by the register macros
    fn from_rust_basic_type(s: &'static str) -> McValueType {
        match s {
            "bool" => McValueType::Bool,
            "u8" => McValueType::Ubyte,
            "i8" => McValueType::Sbyte,
            "u16" => McValueType::Uword,
            "i16" => McValueType::Sword,
            "u32" => McValueType::Ulong,
            "i32" => McValueType::Slong,
            "u64" | "usize" => McValueType::Ulonglong,
            "i64" | "isize" => McValueType::Slonglong,
            "f32" => McValueType::Float32Ieee,
            "f64" => McValueType::Float64Ieee,
            _ => McValueType::Unknown,
        }
    }

    /// Convert from Rust type as str
    /// May be u8, u16, u32, u64, i8, i16, i32, i74, f32, f64, bool, InnerStruct, [InnerStruct; x_dim], [[InnerStruct; x_dim]; y_dim]
    /// // Used by the register macros
    pub fn from_rust_type(s: &'static str) -> McValueType {
        let t = McValueType::from_rust_basic_type(s);
        if t != McValueType::Unknown {
            t
        } else {
            // Trim leading and trailing whitespace and brackets
            let array_type = s.trim_start_matches('[').trim_end_matches(']');

            // Find the first ';' to handle multi-dimensional arrays
            let first_semicolon_index = array_type.find(';').unwrap_or(array_type.len());

            // Extract the substring from the start to the first ';'
            let inner_type = &array_type[..first_semicolon_index].trim();

            // If there are inner brackets, remove them to get the base type
            let base_type = inner_type.trim_start_matches('[').trim_end_matches(']');

            // If the array type is not a basic type, return an McValueType::TypeDef(type_name)
            let t = McValueType::from_rust_basic_type(base_type);
            if t == McValueType::Unknown { McValueType::new_typedef(base_type) } else { t }
        }
    }
}

//-------------------------------------------------------------------------------------------------
// McValueType from rust variables

/// Get RegDataType for a Rust basic type  
/// Glue used by the register_xxx macros
pub trait McValueTypeTrait {
    /// Get RegDataType for a Rust basic type
    fn get_type(&self) -> McValueType;
}

impl<T> McValueTypeTrait for std::num::Wrapping<T>
where
    T: McValueTypeTrait,
{
    fn get_type(&self) -> McValueType {
        self.0.get_type()
    }
}
impl<T> McValueTypeTrait for Option<T>
where
    T: McValueTypeTrait + std::default::Default,
{
    fn get_type(&self) -> McValueType {
        let x: T = T::default();
        x.get_type()
    }
}
impl McValueTypeTrait for bool {
    fn get_type(&self) -> McValueType {
        McValueType::Bool
    }
}
impl McValueTypeTrait for i8 {
    fn get_type(&self) -> McValueType {
        McValueType::Sbyte
    }
}
impl McValueTypeTrait for i16 {
    fn get_type(&self) -> McValueType {
        McValueType::Sword
    }
}
impl McValueTypeTrait for i32 {
    fn get_type(&self) -> McValueType {
        McValueType::Slong
    }
}
impl McValueTypeTrait for i64 {
    fn get_type(&self) -> McValueType {
        McValueType::Slonglong
    }
}
impl McValueTypeTrait for isize {
    fn get_type(&self) -> McValueType {
        McValueType::Slonglong
    }
}
impl McValueTypeTrait for u8 {
    fn get_type(&self) -> McValueType {
        McValueType::Ubyte
    }
}
impl McValueTypeTrait for u16 {
    fn get_type(&self) -> McValueType {
        McValueType::Uword
    }
}
impl McValueTypeTrait for u32 {
    fn get_type(&self) -> McValueType {
        McValueType::Ulong
    }
}
impl McValueTypeTrait for u64 {
    fn get_type(&self) -> McValueType {
        McValueType::Ulonglong
    }
}
impl McValueTypeTrait for usize {
    fn get_type(&self) -> McValueType {
        McValueType::Ulonglong
    }
}
impl McValueTypeTrait for f32 {
    fn get_type(&self) -> McValueType {
        McValueType::Float32Ieee
    }
}
impl McValueTypeTrait for f64 {
    fn get_type(&self) -> McValueType {
        McValueType::Float64Ieee
    }
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
// Test module

#[cfg(test)]
mod mc_type_tests {

    use crate::xcp::xcp_test::test_setup;

    use super::*;

    fn is_copy<T: Sized + Copy>() {}
    fn is_send<T: Sized + Send>() {}
    fn is_sync<T: Sized + Sync>() {}
    fn is_clone<T: Sized + Clone>() {}

    #[test]
    fn test_mc_type() {
        let _ = test_setup();

        // Check markers
        is_sync::<McValueType>();
        is_copy::<McValueType>();
        is_send::<McValueType>();
        is_clone::<McValueType>();

        let t1 = McValueType::Sbyte;
        assert_eq!(t1.get_min(), Some(-128.0));
        assert_eq!(t1.get_max(), Some(127.0));
        assert_eq!(t1.get_size(), 1);

        let byte: u8 = 0;
        let t2 = byte.get_type();
        assert_eq!(t2, McValueType::Ubyte);

        let t2 = McValueType::from_rust_type("u8");
        assert_eq!(t2, McValueType::Ubyte);

        let t3 = McValueType::from_rust_type("[[f64; 3]; 4]");
        assert_eq!(t3, McValueType::Float64Ieee);

        let t4 = McValueType::from_rust_type("MyType");
        assert_eq!(t4, McValueType::TypeDef(McIdentifier::new("MyType")));
        // Should panic
        // let result = std::panic::catch_unwind(|| {
        //     t4.get_size();
        // });
        // assert!(result.is_err());
    }
}
