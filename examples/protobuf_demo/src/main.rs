// protobuf_demo
// @@@@ Work in progress

use anyhow::Result;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{thread, time::Duration};

use xcp::*;

use prost::Message;
//use prost_types::{DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet};

/*


// Define your Rust struct
#[derive(Clone, PartialEq, Message)]
pub struct TestStruct {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(fixed32, tag = "2")] // use fixed32, varint zizag encoding not recomended
    pub counter: u32,
    #[prost(double, tag = "3")]
    pub signal: f64,
}

fn test() {
    let data = TestStruct {
        name: "RainerZ".to_string(),
        counter: 0x01020304,
        signal: 0.123456789,
    };

    // Serialize  to a byte buffer
    let mut buf = Vec::new();
    data.encode(&mut buf).unwrap();
    println!("Serialized data: {:#?}", buf);

    // Write the serialized data to a file
    let mut file = File::create("data.bin").unwrap();
    file.write_all(&buf).unwrap();

    // Obtain the file descriptor set for the struct definition
    let descriptor_set = create_file_descriptor_set();
    println!("Descriptor set: {:#?}", descriptor_set);

    // Serialize the file descriptor set to a file
    let mut desc_file = File::create("descriptor_file.desc").unwrap();
    desc_file.write_all(&descriptor_set.encode_to_vec()).unwrap();
}

// Manually create a FileDescriptorSet that describes the struct
fn create_file_descriptor_set() -> FileDescriptorSet {
    // Create a field descriptor for each field in the struct
    let fields = vec![
        FieldDescriptorProto {
            name: Some("name".to_string()),
            number: Some(1),
            label: Some(prost_types::field_descriptor_proto::Label::Optional as i32),
            r#type: Some(prost_types::field_descriptor_proto::Type::String as i32),
            ..Default::default()
        },
        FieldDescriptorProto {
            name: Some("counter".to_string()),
            number: Some(2),
            label: Some(prost_types::field_descriptor_proto::Label::Optional as i32),
            r#type: Some(prost_types::field_descriptor_proto::Type::Fixed32 as i32),
            ..Default::default()
        },
        FieldDescriptorProto {
            name: Some("signal".to_string()),
            number: Some(3),
            label: Some(prost_types::field_descriptor_proto::Label::Optional as i32),
            r#type: Some(prost_types::field_descriptor_proto::Type::Double as i32),
            ..Default::default()
        },
    ];

    // Create a descriptor for the Person message
    let descriptor = DescriptorProto {
        name: Some("TestStruct".to_string()),
        field: fields,
        ..Default::default()
    };

    // Create a file descriptor that holds the message descriptor
    let file_descriptor_proto = FileDescriptorProto {
        name: Some("test_struct.proto".to_string()),
        package: Some("example".to_string()),
        message_type: vec![descriptor],
        syntax: Some("proto3".to_string()),
        ..Default::default()
    };

    // Create a FileDescriptorSet containing the file descriptor
    let mut file_descriptor_set = FileDescriptorSet::default();
    file_descriptor_set.file.push(file_descriptor_proto);

    file_descriptor_set
}
*/

/*
Explanation of the Code:

    1.	Define the Rust Struct:
    •	The Person struct is defined using prost annotations. Each field is annotated with #[prost(...)], which specifies the type and the tag number.
    2.	Serialize the Struct:
    •	The Person instance is serialized into a byte buffer using prost’s encode method. This buffer can then be written to a file.
    3.	Create a File Descriptor Set:
    •	The create_file_descriptor_set() function manually creates a FileDescriptorSet to describe the Person message.
    •	This includes creating FieldDescriptorProto for each field and DescriptorProto for the message itself.
    •	Finally, a FileDescriptorProto is created to hold the message descriptor, and it’s added to a FileDescriptorSet.
    4.	Write the Descriptor Set:
    •	The descriptor set is serialized and written to a file (descriptor_file.desc), which can be used for inspection or further processing.

Step 2: Compile and Run

To compile and run the program:

cargo run

This will generate two files:

    •	person.bin: The binary-encoded Person message.
    •	descriptor_file.desc: The serialized file descriptor set that describes the Person struct.

Summary:

    •	No .proto File: This approach allows you to work entirely in Rust without the need for a .proto file.
    •	Manual Descriptor Creation: We manually create the FileDescriptorSet to describe the Rust struct, which can be useful for generating or inspecting .proto files from Rust code directly.
    •	Serialization: The Person struct is serialized using prost, and the resulting binary data can be used just like any other protobuf-encoded message.

This method is useful when you want to work purely in Rust and still leverage Protocol Buffers for serialization, while also having the ability to generate or inspect descriptor sets programmatically.


*/

//-----------------------------------------------------------------------------

#[derive(Clone, PartialEq, Message)]
pub struct TestData {
    #[prost(fixed32, tag = "1")] // use fixed32, varint zizag encoding not recomended
    pub counter: u32,
    #[prost(double, tag = "2")]
    pub signal: f64,
}

fn main() -> Result<()> {
    println!("protobuf demo");

    env_logger::Builder::new().target(env_logger::Target::Stdout).filter_level(log::LevelFilter::Info).init();

    let xcp = XcpBuilder::new("xcp_demo")
        .set_log_level(XcpLogLevel::Debug)
        .set_epk("EPK_")
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555)?;

    // Data struct to be measured
    let mut test_data = TestData { counter: 0, signal: 0.0 };

    /*
    r#"/begin ANNOTATION ANNOTATION_LABEL "ObjectDescription" ANNOTATION_ORIGIN "application/dynamic-object-package" /begin ANNOTATION_TEXT
    "<DynamicObject>"
    "<Package> {filename}.do.zip< /Package>"
    "<RootType> {name} </RootType>"
    "<RootFile> {file} </RootFile>"
    "</DynamicObject>"
    "/end ANNOTATION_TEXT /end ANNOTATION "
     */

    // Create a proto description for the data struct
    let annotation = r#"/begin ANNOTATION ANNOTATION_LABEL "ObjectDescription" ANNOTATION_ORIGIN "application/protobuf"
    /begin ANNOTATION_TEXT
        "<DynamicObject>"
        "<RootType>TestData</RootType>"
        "</DynamicObject>"
        "message TestData {"
        "  fixed32 counter = 1;"
        "  double signal = 2;"
        "}"
    /end ANNOTATION_TEXT
/end ANNOTATION"#
        .to_string();

    // Register the data struct and create a buffer

    let mut buf = Vec::new();
    let event = xcp.create_event("test_data");
    xcp.get_registry()
        .lock()
        .add_measurement(RegistryMeasurement::new(
            "test_data",
            RegistryDataType::Blob,
            1,
            1,
            event,
            0,
            0u64,
            1.0,
            0.0,
            "proto serialized test data",
            "",
            Some(annotation),
        ))
        .expect("Duplicate");

    // Loop
    loop {
        test_data.counter += 1;
        test_data.signal += 0.1;

        // Serialize data and trigger measurememt event
        buf.clear();
        test_data.encode(&mut buf).unwrap();
        println!("Capacity: {}, Data: {:?}", buf.capacity(), buf);
        unsafe {
            event.trigger_ext(buf.as_ptr());
        }

        thread::sleep(Duration::from_micros(1000000));

        xcp.write_a2l().unwrap(); // @@@@ Remove: force A2L write
    }
    // Ok(())
}
