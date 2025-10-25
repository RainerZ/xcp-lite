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

Usage: xcp_client [OPTIONS]

Options:
      --log-level <LOG_LEVEL>
          Log level (Off=0, Error=1, Warn=2, Info=3, Debug=4, Trace=5)

          [default: 3]

      --verbose <VERBOSE>
          Verbose output Enables additional output when reading ELF files and creating A2L files
          
          [default: 0]

      --dest-addr <DEST_ADDR>
          XCP server address (IP address or IP:port). If port is omitted, uses --port parameter
          
          [default: 127.0.0.1]

      --port <PORT>
          XCP server port number (used when --dest-addr doesn't include port)
          
          [default: 5555]

      --bind-addr <BIND_ADDR>
          Bind address (IP address or IP:port). If port is omitted, system assigns an available port
          
          [default: 0.0.0.0]

      --tcp
          Use TCP for XCP communication..

      --udp
          Use UDP for XCP communication

      --offline
          Force offline mode (no network communication), communication parameters are used to create A2L file

      --a2l <A2L>
          Specify and overide the name of the A2L file name. If not specified, The A2L file name is read from the XCP server
          
          [default: ]

      --upload-a2l
          Upload A2L file from XCP server. Requires that the XCP server supports GET_ID A2L upload

      --create-a2l
          Build an A2L file template from XCP server information about events and memory segments. Requires that the XCP server supports the GET_EVENT_INFO and GET_SEGMENT_INFO commands. Insert all visible measurement and calibration variables from ELF file if specified with --elf

      --fix-a2l
          Update the given A2L file with XCP server information about events and memory segments. Requires that the XCP server supports the GET_EVENT_INFO and GET_SEGMENT_INFO commands

      --elf <ELF>
          Specifiy the name of an ELF file, create an A2L file from ELF debug information. If connected to a XCP server, events and memory segments will be extracted from the XCP server
          
          [default: ]

      --elf-unit-limit <ELF_UNIT_LIMIT>
          Parse only compilations units <= n
          
          [default: 18446744073709551615]

      --bin <BIN>
          Specify the pathname of a binary file (Intel-HEX or XCPlite-BIN) for calibration parameter segment data
          
          [default: ]

      --upload-bin
          Upload all calibration segments working page data and store into a given binary file. Requires that the XCP server supports GET_ID A2L upload

      --download-bin
          Download all calibration segments working page data in a given binary file

      --list-mea <LIST_MEA>
          Lists all specified measurement variables (regex) found in the A2L file
          
          [default: ]

      --mea <MEA>...
          Specify variable names for DAQ measurement (list), may be list of names separated by space or single regular expressions (e.g. ".*")

      --time-ms <TIME_MS>
          Limit measurement duration to n ms
          
          [default: 0]

      --time <TIME>
          Limit measurement duration to n s
          
          [default: 0]

      --list-cal <LIST_CAL>
          Lists all specified calibration variables (regex) found in the A2L file
          
          [default: ]

      --cal <NAME> <VALUE>
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
xcp_client -- --list-cal ".*"
```

### Set a calibration variable

```bash
xcp_client -- --cal variable_name 42.5
```

### Measure variables

With A2L upload

```bash
xcp_client --dest-addr=192.168.0.206  --tcp --upload-a2l --mea ".*" -t 5000
```

With A2L given

```bash
xcp_client --dest-addr=192.168.0.206  --tcp --a2l hello_xcp.a2l  --mea ".*" 
```

With ELF file only

```bash
```bash
xcp_client --dest-addr=192.168.0.206  --tcp --elf fixtures/no_a2l_demo.out --mea "counter"
```

```

### Upload the  A2L file from the target

```bash
xcp_client --dest-addr=192.168.0.206:5555 --tcp  --upload-a2l   
```

### Create an A2L file for a target without A2L upload support

```bash
xcp_client --dest-addr=192.168.0.206:5555 --tcp  --create-a2l --elf no_a2l_demo.out --a2l test.a2l 
```

### Detailed A2L Generation Options

See XCPlite no_a2l_demo README.md.  

Create an A2L for an application ELF file with DWARF debug information.

```bash
xcp_client --dest-addr=192.168.0.206 --tcp --elf no_a2l_demo.out  --create-a2l --create-epk-segment --a2l no_a2l_demo.a2l --offline >no_a2l_demo.log
```
