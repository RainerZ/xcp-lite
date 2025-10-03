# xcp_client

XCP client implementation in Rust

Used for integration testing xcp-lite.  
Partial XCP implementation hard-coded for xcp-lite testing.  
Using tokio and a2lfile.  

Usage: xcp_client [OPTIONS]

Options:
  -l, --log-level <LOG_LEVEL>  Log level (Off=0, Error=1, Warn=2, Info=3, Debug=4, Trace=5) [default: 3]
  -d, --dest-addr <DEST_ADDR>  XCP server address [default: 127.0.0.1:5555]
  -p, --port <PORT>            XCP server port number [default: 5555]
  -b, --bind-addr <BIND_ADDR>  Bind address, master port number [default: 0.0.0.0:9999]
      --tcp                    Use TCP instead of UDP for XCP communication
  -a, --a2l <A2L>              Specify the name for the A2L file [default: ]
  -e, --elf <ELF>              Specify the name of the ELF file [default: ]
      --upload-a2l             Load A2L file from XCP server Requires that the XCP server supports the A2L upload command
      --create-a2l             Build an A2L file template from XCP information about events and memory segments Requires that the XCP server supports the GET_EVENT_INFO and GET_SEGMENT_INFO commands
      --list-mea <LIST_MEA>    Lists all matching measurement variables found in the A2L file [default: ]
      --list-cal <LIST_CAL>    Lists all matching calibration variables found in the A2L file [default: ]
  -m, --mea <MEA>...           Specify variable names for DAQ measurement, may be list of names separated by space or single regular expressions (e.g. ".*")
  -t, --time-ms <TIME_MS>      Specify measurement duration in ms [default: 5000]
      --cal <NAME> <VALUE>     Set calibration variable to a value (format: "variable_name value")
      --test                   --test
  -h, --help                   Print help
  -V, --version                Print version

## Examples

### List calibration variables

```bash
cargo run -p=xcp_client -- --list-cal ".*"
```

### Set a calibration variable

```bash
cargo run -p=xcp_client -- --cal variable_name 42.5
```

### Measure variables

```bash
cargo run -p=xcp_client -- -m ".*" -t 5000
```

  ``` rust

    // Create xcp_client
    let mut xcp_client = XcpClient::new("127.0.0.1:5555", "0.0.0.0:0");

    // Connect to the XCP server
    let res = xcp_client.connect(DaqDecoder::new(), ServTextDecoder::new()).await?;
    
    // Upload A2L file or read A2L file
    xcp_client.upload_a2l(false).await?;
    xcp_client.read_a2l("test.a2l",false)?;

    // Calibration
    // Create a calibration object for CalPage1.counter_max
    if let Ok(counter_max) = xcp_client.create_calibration_object("CalPage1.counter_max").await
    {
        // Get current value
        let v = xcp_client.get_value_u64(counter_max);
        info!("CalPage1.counter_max = {}", v);

        // Set value to 1000
        info!("CalPage1.counter_max = {}", v);
        xcp_client.set_value_u64(counter_max, 1000).await?;
    }

    // Measurement
    // Create a measurement for signal counter:u32
    xcp_client.init_measurement().await?;
    xcp_client.create_measurement_object("counter").await?;
    xcp_client.start_measurement().await?;
    sleep(Duration::from_secs(1)).await;
    xcp_client.stop_measurement().await?;

    // Disconnect
    xcp_client.disconnect().await?);

   ```
