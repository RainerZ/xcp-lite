// Regression test for xcp-lite issue #51: `#[derive(McRegisterType)]` must accept a field type
// produced by macro_rules! substitution. When a captured `:ty` fragment is emitted as part of a
// macro-generated struct, the compiler wraps it in a hygiene-preserving `syn::Type::Group`
// instead of a bare `syn::Type::Path`; the derive's type parser (`src/ty.rs`) must unwrap it.

use xcp_registry::McRegisterType;

macro_rules! define_struct {
    ($name:ident, $field:ident : $t:ty) => {
        #[derive(xcp_register_type_derive::McRegisterType, Copy, Clone, Debug)]
        struct $name {
            $field: $t,
        }
    };
}

#[derive(xcp_register_type_derive::McRegisterType, Copy, Clone, Debug)]
struct Inner {
    a: u8,
}

// Group-wrapped scalar field type (the exact case reported in issue #51).
define_struct!(GroupScalar, value: f32);

// Group-wrapped user-defined (typedef-recursing) field type.
define_struct!(GroupUser, inner: Inner);

#[test]
fn group_wrapped_scalar_field_registers() {
    let _ = GroupScalar { value: 1.0 };
    assert_eq!(<GroupScalar as McRegisterType>::mc_type_name(), "GroupScalar");
}

#[test]
fn group_wrapped_user_type_field_registers() {
    let _ = GroupUser { inner: Inner { a: 0 } };
    assert_eq!(<GroupUser as McRegisterType>::mc_type_name(), "GroupUser");
}
