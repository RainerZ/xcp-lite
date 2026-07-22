// Module mc_typedef
// Types:
//  McTypeDef, McTypeDefField

use serde::Deserialize;
use serde::Serialize;

use super::McDimType;
use super::McIdentifier;
use super::McSupportData;
use super::McValueType;
use super::RegistryError;

//-------------------------------------------------------------------------------------------------
// McTypeDef

// Type definition for McValueType::TypeDef(type_name)
#[derive(Debug, Serialize, Deserialize)]
pub struct McTypeDef {
    pub name: McIdentifier,
    pub fields: McTypeDefFieldList, // Fields of the struct type_name
    pub size: usize,                // Size of the struct type_name in bytes
}

impl McTypeDef {
    pub fn new<T: Into<McIdentifier>>(name: T, size: usize) -> McTypeDef {
        let name: McIdentifier = name.into();
        McTypeDef {
            name,
            fields: McTypeDefFieldList::new(),
            size,
        }
    }

    pub fn get_name(&self) -> &'static str {
        self.name.as_str()
    }

    pub fn find_field(&self, name: &str) -> Option<&McTypeDefField> {
        self.fields.into_iter().find(|field| field.name == name)
    }

    pub fn find_field_mut(&mut self, name: &str) -> Option<&mut McTypeDefField> {
        self.fields.0.iter_mut().find(|f| f.name == name)
    }

    pub fn add_field<T: Into<McIdentifier>>(&mut self, name: T, dim_type: McDimType, mc_support_data: McSupportData, offset: u16) -> Result<(), RegistryError> {
        let name: McIdentifier = name.into();

        // Error if duplicate field name
        if self.find_field(name.as_str()).is_some() {
            return Err(RegistryError::Duplicate(name.to_string()));
        }

        // Add field
        self.fields.push(McTypeDefField::new(name, dim_type, mc_support_data, offset));
        Ok(())
    }
}

//----------------------------------------------------------------------------------------------
// McTypeDefFieldList

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct McTypeDefList(Vec<McTypeDef>);

impl std::ops::Deref for McTypeDefList {
    type Target = [McTypeDef];
    fn deref(&self) -> &[McTypeDef] {
        &self.0
    }
}

impl std::ops::DerefMut for McTypeDefList {
    fn deref_mut(&mut self) -> &mut [McTypeDef] {
        &mut self.0
    }
}

impl McTypeDefList {
    pub fn new() -> Self {
        McTypeDefList(Vec::with_capacity(16))
    }

    pub fn get_mut(&mut self, index: usize) -> &mut McTypeDef {
        &mut self.0[index]
    }
    pub fn push(&mut self, object: McTypeDef) {
        self.0.push(object);
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
    pub fn find_typedef_mut(&mut self, name: &str) -> Option<&mut McTypeDef> {
        self.0.iter_mut().find(|i| i.name == name)
    }
    pub fn find_typedef(&self, name: &str) -> Option<&McTypeDef> {
        self.0.iter().find(|i| i.name == name)
    }

    pub fn sort_by_name(&mut self) {
        self.0.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

impl<'a> IntoIterator for &'a McTypeDefList {
    type Item = &'a McTypeDef;
    type IntoIter = std::slice::Iter<'a, McTypeDef>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

//-------------------------------------------------------------------------------------------------
// McTypeDefField

#[derive(Debug, Serialize, Deserialize)]
pub struct McTypeDefField {
    pub name: McIdentifier,
    pub dim_type: McDimType,            // Type name and matrix dimensions, recursion here if McValueType::TypeDef
    pub mc_support_data: McSupportData, // Metadata for the field
    pub offset: u16,                    // Offset of the field in the struct ABI
}

impl McTypeDefField {
    pub fn new<T: Into<McIdentifier>>(field_name: T, dim_type: McDimType, mc_support_data: McSupportData, offset: u16) -> McTypeDefField {
        McTypeDefField {
            name: field_name.into(),
            dim_type,
            mc_support_data,
            offset,
        }
    }

    pub fn get_name(&self) -> &'static str {
        self.name.as_str()
    }

    /// Check if the value type is a typedef and return the typedef name if it is
    pub fn get_typedef_name(&self) -> Option<&'static str> {
        match self.dim_type.value_type {
            McValueType::TypeDef(typedef_name) => Some(typedef_name.as_str()),
            _ => None,
        }
    }

    /// Get the offset of the field in the struct ABI
    pub fn get_offset(&self) -> u16 {
        self.offset
    }

    /// Get type
    pub fn get_dim_type(&self) -> &McDimType {
        &self.dim_type
    }

    /// Get metadata
    pub fn get_mc_support_data(&self) -> &McSupportData {
        &self.mc_support_data
    }
}

//----------------------------------------------------------------------------------------------
// McTypeDefFieldList

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct McTypeDefFieldList(Vec<McTypeDefField>);

impl std::ops::Deref for McTypeDefFieldList {
    type Target = [McTypeDefField];
    fn deref(&self) -> &[McTypeDefField] {
        &self.0
    }
}

impl std::ops::DerefMut for McTypeDefFieldList {
    fn deref_mut(&mut self) -> &mut [McTypeDefField] {
        &mut self.0
    }
}

impl McTypeDefFieldList {
    pub fn new() -> Self {
        McTypeDefFieldList(Vec::with_capacity(8))
    }

    pub fn push(&mut self, object: McTypeDefField) {
        self.0.push(object);
    }

    pub fn find_typedef_field(&self, name: &str) -> Option<&McTypeDefField> {
        self.0.iter().find(|i| i.name == name)
    }
}

impl<'a> IntoIterator for &'a McTypeDefFieldList {
    type Item = &'a McTypeDefField;
    type IntoIter = std::slice::Iter<'a, McTypeDefField>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
