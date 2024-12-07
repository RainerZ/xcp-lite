//-----------------------------------------------------------------------------
// Module registry

//-----------------------------------------------------------------------------
// Submodules

// Registry and A2l writer
mod registry;
pub use registry::*;

//-------------------------------------------------------------------------------------------------
// Test module

#[cfg(test)]
mod registry_tests {

    use super::*;
    use std::net::Ipv4Addr;

    use xcp_type_description::prelude::*;

    //-----------------------------------------------------------------------------
    // Test registry and A2L writer
    #[test]
    fn test_registry_2() {
        let mut reg = Registry::new();
        reg.set_name("test_registry_2");
        reg.set_epk("TEST_EPK", 0x80000000);
        reg.set_tl_params("UDP", Ipv4Addr::new(127, 0, 0, 1), 5555);

        reg.add_cal_seg("test_cal_seg_1", 0, 4);
        reg.add_cal_seg("test_cal_seg_2", 1, 4);

        let event1_1 = crate::XcpEvent::new(0, 1);
        reg.add_event("event1", event1_1, 0);
        let event1_2 = crate::XcpEvent::new(1, 2);
        reg.add_event("event1", event1_2, 0);
        let event2 = crate::XcpEvent::new(2, 0);
        reg.add_event("event2", event2, 0);

        reg.add_characteristic(RegistryCharacteristic::new(
            Some("test_cal_seg_1"),
            "test_characteristic_1",
            crate::RegistryDataType::Sbyte,
            "comment",
            -128.0,
            127.0,
            "",
            1,
            1,
            0,
        ))
        .unwrap();
        reg.add_characteristic(RegistryCharacteristic::new(
            Some("test_cal_seg_1"),
            "test_characteristic_2",
            crate::RegistryDataType::Sbyte,
            "comment",
            -128.0,
            127.0,
            "",
            1,
            1,
            1,
        ))
        .unwrap();

        reg.add_measurement(RegistryMeasurement::new(
            "test_measurement_1",
            crate::RegistryDataType::Ubyte,
            1,
            1,
            event1_1,
            0,
            0,
            1.0,
            1.0,
            "comment",
            "unit",
            None,
        ))
        .unwrap();

        reg.add_measurement(RegistryMeasurement::new(
            "test_measurement_1",
            crate::RegistryDataType::Ubyte,
            1,
            1,
            event1_2,
            0,
            0,
            1.0,
            1.0,
            "comment",
            "unit",
            None,
        ))
        .unwrap();

        reg.add_measurement(RegistryMeasurement::new(
            "test_measurement_2",
            crate::RegistryDataType::Ubyte,
            1,
            1,
            event2,
            0,
            0,
            1.0,
            1.0,
            "comment",
            "unit",
            None,
        ))
        .unwrap();

        reg.write_a2l().unwrap();

        #[cfg(feature = "a2l_reader")]
        {
            if let Err(e) = reg.a2l_load("test_registry_2.a2l") {
                log::error!("A2l file check error: {}", e);
            } else {
                log::info!("A2L file check ok");
            }
        }

        reg.freeze();
        assert!(reg.is_frozen());

        let err = reg.write_a2l();
        assert!(err.is_err());

        let _ = std::fs::remove_file("test_registry_2.a2l");
    }

    //-----------------------------------------------------------------------------
    // Test A2L writer

    #[derive(serde::Serialize, serde::Deserialize, XcpTypeDescription, Debug, Clone, Copy)]
    struct CalPage {
        #[type_description(comment = "comment")]
        #[type_description(unit = "unit")]
        #[type_description(min = "-128.0")]
        #[type_description(max = "127.0")]
        test_characteristic_1: i8,
        #[type_description(comment = "comment")]
        #[type_description(unit = "unit")]
        #[type_description(min = "-128.0")]
        #[type_description(max = "127.0")]
        test_characteristic_2: i8,
        test_characteristic_3: i16,
    }

    const CAL_PAGE: CalPage = CalPage {
        test_characteristic_1: 0,
        test_characteristic_2: 0,
        test_characteristic_3: 0,
    };

    #[test]
    fn test_registry_1() {
        crate::xcp::xcp_test::test_setup(log::LevelFilter::Info);

        let xcp = crate::Xcp::get();
        let reg_ref = xcp.get_registry();

        {
            let mut reg = reg_ref.lock();

            reg.set_name("test_registry_1");
            reg.set_epk("TEST_EPK", 0x80000000);
            reg.set_tl_params("UDP", Ipv4Addr::new(127, 0, 0, 1), 5555);
        }

        let _calseg1 = xcp.create_calseg("test_cal_seg_1", &CAL_PAGE).register_fields();

        let event1_1 = xcp.create_event_ext("event1", true, 0);
        let event1_2 = xcp.create_event_ext("event1", true, 0);
        let event2 = xcp.create_event_ext("event2", false, 0);

        {
            let mut reg = reg_ref.lock();

            reg.add_measurement(RegistryMeasurement::new(
                "test_measurement_1",
                crate::RegistryDataType::Ubyte,
                1,
                1,
                event1_1,
                0,
                0,
                1.0,
                1.0,
                "comment",
                "unit",
                None,
            ))
            .unwrap();

            reg.add_measurement(RegistryMeasurement::new(
                "test_measurement_1",
                crate::RegistryDataType::Ubyte,
                1,
                1,
                event1_2,
                0,
                0,
                1.0,
                1.0,
                "comment",
                "unit",
                None,
            ))
            .unwrap();

            reg.add_measurement(RegistryMeasurement::new(
                "test_measurement_2",
                crate::RegistryDataType::Ubyte,
                1,
                1,
                event2,
                0,
                0,
                1.0,
                1.0,
                "comment",
                "unit",
                None,
            ))
            .unwrap();
        }

        xcp.write_a2l().unwrap();

        #[cfg(feature = "a2l_reader")]
        {
            let mut reg = reg_ref.lock();

            if let Err(e) = reg.a2l_load("test_registry_1.a2l") {
                log::error!("A2l file check error: {}", e);
            } else {
                log::info!("A2L file check ok");
            }
        }

        let _ = std::fs::remove_file("test_registry_1.a2l");
    }
}
