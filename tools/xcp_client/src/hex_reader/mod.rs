use std::{error::Error, io::Write};

pub fn test_ihex() -> Result<(), Box<dyn Error>> {
    println!("create:");
    let ihex_records = &[
        ihex::Record::Data {
            offset: 0x0010,
            value: vec![11, 12, 13, 14, 15],
        },
        ihex::Record::Data {
            offset: 0x0020,
            value: vec![21, 22, 23, 24, 25],
        },
        ihex::Record::EndOfFile,
    ];

    let ihex_object = ihex::create_object_file_representation(ihex_records)?;
    println!("string:");
    println!("{}", ihex_object);

    // Write String object to file test.hex
    println!("write:");
    let mut file = std::fs::File::create("test.hex")?;
    file.write_all(ihex_object.as_bytes())?;

    // Reload from file and parse
    println!("read:");
    let file_content = std::fs::read_to_string("test1.hex")?;
    println!("string:");
    println!("{}", file_content);

    let ihex_reader = ihex::Reader::new(file_content.as_str());
    for record in ihex_reader {
        match record {
            Err(e) => {
                println!("Error parsing IHEX record: {}", e);
                continue;
            }
            Ok(record) => {
                println!("record: {:?}", record);
            }
        }
    }

    return Ok(());
}
