#!/usr/bin/env python3
"""
Compare two Intel HEX files by parsing them and showing their data content.
"""

import sys
from collections import defaultdict

def parse_hex_file(filename):
    """Parse Intel HEX file and return dict of address -> data"""
    data = {}
    extended_addr = 0
    
    with open(filename, 'r') as f:
        for line_num, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            
            if not line.startswith(':'):
                print(f"Warning: Line {line_num} doesn't start with ':'")
                continue
            
            # Parse record
            byte_count = int(line[1:3], 16)
            address = int(line[3:7], 16)
            record_type = int(line[7:9], 16)
            
            if record_type == 0x00:  # Data record
                full_addr = extended_addr | address
                data_bytes = bytes.fromhex(line[9:9+byte_count*2])
                if full_addr in data:
                    print(f"Warning: Duplicate address 0x{full_addr:08X}")
                data[full_addr] = data_bytes
                
            elif record_type == 0x01:  # End of file
                break
                
            elif record_type == 0x02:  # Extended Segment Address
                seg = int(line[9:13], 16)
                extended_addr = seg << 4
                
            elif record_type == 0x04:  # Extended Linear Address
                ela = int(line[9:13], 16)
                extended_addr = ela << 16
                
            elif record_type == 0x05:  # Start Linear Address
                pass  # Ignore
                
    return data

def compare_hex_files(file1, file2):
    """Compare two hex files and show differences"""
    print(f"Parsing {file1}...")
    data1 = parse_hex_file(file1)
    
    print(f"Parsing {file2}...")
    data2 = parse_hex_file(file2)
    
    print(f"\n{'='*80}")
    print(f"Comparison Results:")
    print(f"{'='*80}\n")
    
    print(f"File 1: {file1}")
    print(f"  Total addresses: {len(data1)}")
    print(f"  Total bytes: {sum(len(v) for v in data1.values())}")
    
    print(f"\nFile 2: {file2}")
    print(f"  Total addresses: {len(data2)}")
    print(f"  Total bytes: {sum(len(v) for v in data2.values())}")
    
    # Get all addresses
    all_addrs = sorted(set(data1.keys()) | set(data2.keys()))
    
    print(f"\n{'='*80}")
    print(f"Address Ranges:")
    print(f"{'='*80}\n")
    
    # Group consecutive addresses into segments
    segments1 = []
    segments2 = []
    
    for addr in sorted(data1.keys()):
        if not segments1 or addr != segments1[-1][1]:
            segments1.append([addr, addr + len(data1[addr])])
        else:
            segments1[-1][1] = addr + len(data1[addr])
    
    for addr in sorted(data2.keys()):
        if not segments2 or addr != segments2[-1][1]:
            segments2.append([addr, addr + len(data2[addr])])
        else:
            segments2[-1][1] = addr + len(data2[addr])
    
    print(f"File 1 segments:")
    for start, end in segments1:
        print(f"  0x{start:08X} - 0x{end:08X} ({end-start} bytes)")
    
    print(f"\nFile 2 segments:")
    for start, end in segments2:
        print(f"  0x{start:08X} - 0x{end:08X} ({end-start} bytes)")
    
    # Check for differences
    differences = []
    only_in_1 = []
    only_in_2 = []
    
    for addr in all_addrs:
        if addr in data1 and addr in data2:
            if data1[addr] != data2[addr]:
                differences.append((addr, data1[addr], data2[addr]))
        elif addr in data1:
            only_in_1.append((addr, data1[addr]))
        else:
            only_in_2.append((addr, data2[addr]))
    
    if not differences and not only_in_1 and not only_in_2:
        print(f"\n{'='*80}")
        print(f"✓ FILES ARE IDENTICAL!")
        print(f"{'='*80}\n")
        return True
    
    print(f"\n{'='*80}")
    print(f"Differences Found:")
    print(f"{'='*80}\n")
    
    if only_in_1:
        print(f"Only in {file1}:")
        for addr, data_bytes in only_in_1[:10]:  # Show first 10
            print(f"  0x{addr:08X}: {data_bytes.hex()}")
        if len(only_in_1) > 10:
            print(f"  ... and {len(only_in_1)-10} more")
    
    if only_in_2:
        print(f"\nOnly in {file2}:")
        for addr, data_bytes in only_in_2[:10]:
            print(f"  0x{addr:08X}: {data_bytes.hex()}")
        if len(only_in_2) > 10:
            print(f"  ... and {len(only_in_2)-10} more")
    
    if differences:
        print(f"\nData differs at same address:")
        for addr, bytes1, bytes2 in differences[:10]:
            print(f"  0x{addr:08X}:")
            print(f"    File1: {bytes1.hex()}")
            print(f"    File2: {bytes2.hex()}")
        if len(differences) > 10:
            print(f"  ... and {len(differences)-10} more")
    
    return False

if __name__ == '__main__':
    if len(sys.argv) != 3:
        print("Usage: compare_hex.py <file1.hex> <file2.hex>")
        sys.exit(1)
    
    compare_hex_files(sys.argv[1], sys.argv[2])
