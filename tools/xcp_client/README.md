# xcp_client

XCP client implementation in Rust

Used for integration testing xcp-lite and for uploading or generating A2L files.  
Partial XCP implementation hard-coded for xcp-lite and XCPlite.  

XCP client v0.9.3 for testing XCP servers and managing A2L files.

This tool can:

- Connect to XCP on Ethernet servers via TCP or UDP
- Upload A2L files from XCP servers (GET_ID command)
- Create complete A2L files from ELF debug information the XCP server event and memory segment information
- Create A2L templates from the XCP server event and memory segment information
- Read and write calibration variables (CAL)
- Configure and acquire measurement data (DAQ)
- List available variables and parameters with regex patterns
- Execute test sequences

Examples:
  xcp_client --tcp --dest-addr 192.168.1.100 --port 5555 --upload-a2l
  xcp_client --dest-addr 192.168.1.100:8080 --upload-a2l
  xcp_client --bind-addr 192.168.1.50 --dest-addr 192.168.1.100 --upload-a2l
  xcp_client --mea ".*temperature.*" --time 10
  xcp_client --elf myprogram.elf --create-a2l
  xcp_client --cal variable_name 42.5
  xcp_client --list-mea "sensor.*" --list-cal "param.*"
  xcp_client --test

Usage: xcp_client [OPTIONS]

Options:
  -l, --log-level '<LOG_LEVEL>'
          Log level (Off=0, Error=1, Warn=2, Info=3, Debug=4, Trace=5)

          [default: 3]

  -v, --verbose '<VERBOSE>'
          Verbose output Enables additional output when reading ELF files and creating A2L files

          [default: 0]

  -d, --dest-addr '<DEST_ADDR>'
          XCP server address (IP address or IP:port). If port is omitted, uses --port parameter

          [default: 127.0.0.1]

  -p, --port '<PORT>'
          XCP server port number (used when --dest-addr doesn't include port)

          [default: 5555]

  -b, --bind-addr '<BIND_ADDR>'
          Bind address (IP address or IP:port). If port is omitted, system assigns an available port

          [default: 0.0.0.0]

      --tcp
          Use TCP for XCP communication

      --udp
          Use UDP for XCP communication

      --offline
          Use offline mode (no network communication), communication parameters are used to create A2L file only

  -a, --a2l '<A2L>'
          Specify and overide the name of the A2L file name If not specified, The A2L file name is read from the XCP server

          [default: ]

  -u, --upload-a2l
          Upload A2L file from XCP server Requires that the XCP server supports GET_ID A2L upload

      --create-a2l
          Build an A2L file template from XCP server information about events and memory segments Requires that the XCP server supports the GET_EVENT_INFO and GET_SEGMENT_INFO commands Insert all visible measurement and calibration variables from ELF file if specified with --elf

      --fix-a2l
          Update the given A2L file with XCP server information about events and memory segments Requires that the XCP server supports the GET_EVENT_INFO and GET_SEGMENT_INFO commands

  -e, --elf '<ELF>'
          Specifiy the name of an ELF file, create an A2L file from ELF debug information If connected to a XCP server, events and memory segments will be extracted from the XCP server

          [default: ]

      --elf-unit-limit '<ELF_UNIT_LIMIT>'
          Parse only compilations units '<= n
          
          [default: 0]

      --list-mea '<LIST_MEA>'
          Lists all specified measurement variables (regex) found in the A2L file
          
          [default: ]

  -m, --mea '<MEA>'...
          Specify variable names for DAQ measurement (list), may be list of names separated by space or single regular expressions (e.g. ".*")

      --time-ms '<TIME_MS>'
          Limit measurement duration to n ms
          
          [default: 0]

  -t, --time '<TIME>'
          Limit measurement duration to n s

          [default: 0]

      --list-cal '<LIST_CAL>'
          Lists all specified calibration variables (regex) found in the A2L file
          
          [default: ]

      --cal '<NAME>' '<VALUE>'
          Set calibration variable to a value (format: "variable_name value")

      --test
          --test Execute a test sequence on the XCP server

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

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

### Upload the  A2L file of the target

```bash
cargo r -p xcp_client -- --dest-addr=192.168.0.206:5555 --tcp  --upload-a2l   
```

### Create an A2L file for a target without A2L upload support

```bash
cargo r -p xcp_client -- --dest-addr=192.168.0.206:5555 --tcp  --create-a2l --elf no_a2l_demo.out --a2l test.a2l 
```

### Detailed A2L Generation Options

See XCPlite no_a2l_demo README.md.  
