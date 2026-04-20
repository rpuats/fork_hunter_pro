#!/usr/bin/env python3
"""
Struct Field Mapping Diagnostic Tool

Analyzes what fields are returned by parsers and maps them to
the correct shared::Event and shared::Odd struct fields.
"""

import json
import re
from pathlib import Path
from typing import Dict, List, Set


def extract_struct_fields(file_path: str, struct_name: str) -> Dict[str, str]:
    """Extract field names and types from Rust struct definition"""
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Find struct definition
    pattern = rf'pub struct {struct_name}\s*\{{(.*?)\}}'
    match = re.search(pattern, content, re.DOTALL)
    
    if not match:
        return {}
    
    fields = {}
    struct_content = match.group(1)
    
    # Extract fields
    field_pattern = r'pub\s+(\w+):\s+([^,\n]+)'
    for field_match in re.finditer(field_pattern, struct_content):
        field_name = field_match.group(1)
        field_type = field_match.group(2).strip()
        fields[field_name] = field_type
    
    return fields


def analyze_parser_file(parser_path: str, struct_name: str) -> Dict:
    """Analyze a parser file to find struct initialization"""
    with open(parser_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Find struct initialization patterns
    pattern = rf'{struct_name}\s*\{{(.*?)\}}'
    matches = list(re.finditer(pattern, content, re.DOTALL))
    
    initialized_fields = set()
    
    for match in matches:
        init_content = match.group(1)
        # Extract field names being initialized
        field_pattern = r'(\w+):\s*'
        for field_match in re.finditer(field_pattern, init_content):
            initialized_fields.add(field_match.group(1))
    
    return {
        'initialized_fields': list(initialized_fields),
        'initializations_found': len(matches),
    }


def main():
    print("[*] Analyzing Rust struct definitions and parser implementations...\n")
    
    # Shared module paths
    shared_file = "crates/shared/src/models.rs"
    
    # Parser paths
    parsers = {
        'mbet': 'crates/parsers/src/mbet.rs',
        'melbet': 'crates/parsers/src/melbet.rs',
        'tennis': 'crates/parsers/src/tennis.rs',
    }
    
    # Extract Event and Odd struct definitions
    print("[+] Shared Structs (from models.rs):\n")
    
    event_fields = extract_struct_fields(shared_file, 'Event')
    odd_fields = extract_struct_fields(shared_file, 'Odd')
    
    print(f"Event struct fields ({len(event_fields)}):")
    for field, ftype in sorted(event_fields.items()):
        print(f"  - {field}: {ftype}")
    
    print(f"\nOdd struct fields ({len(odd_fields)}):")
    for field, ftype in sorted(odd_fields.items()):
        print(f"  - {field}: {ftype}")
    
    # Analyze each parser
    print("\n" + "="*60)
    print("[+] Parser Implementations:\n")
    
    for parser_name, parser_path in parsers.items():
        print(f"\n{parser_name.upper()}:")
        
        # Check Event
        event_analysis = analyze_parser_file(parser_path, 'Event')
        print(f"  Event initializations: {event_analysis['initializations_found']}")
        print(f"  Fields initialized: {event_analysis['initialized_fields']}")
        
        # Check differences
        missing_fields = set(event_fields.keys()) - set(event_analysis['initialized_fields'])
        wrong_fields = set(event_analysis['initialized_fields']) - set(event_fields.keys())
        
        if missing_fields:
            print(f"  MISSING FIELDS: {missing_fields}")
        if wrong_fields:
            print(f"  WRONG FIELD NAMES (not in Event struct): {wrong_fields}")
        
        # Check Odd
        odd_analysis = analyze_parser_file(parser_path, 'Odd')
        print(f"  Odd initializations: {odd_analysis['initializations_found']}")
        print(f"  Fields initialized: {odd_analysis['initialized_fields']}")
        
        # Check differences
        missing_odd_fields = set(odd_fields.keys()) - set(odd_analysis['initialized_fields'])
        wrong_odd_fields = set(odd_analysis['initialized_fields']) - set(odd_fields.keys())
        
        if missing_odd_fields:
            print(f"  MISSING ODD FIELDS: {missing_odd_fields}")
        if wrong_odd_fields:
            print(f"  WRONG ODD FIELD NAMES: {wrong_odd_fields}")
    
    # Generate mapping report
    print("\n" + "="*60)
    print("[+] FIELD MAPPING RECOMMENDATIONS:\n")
    
    mappings = {
        'mbet': {
            'Event': {
                'RENAME': {
                    'name': 'NOT IN STRUCT (remove)',
                    'bookmaker': 'bookmaker_slug',
                    'timestamp': 'NOT IN STRUCT (remove)',
                },
                'ADD': ['raw_url', 'extra (HashMap::new())'],
            },
            'Odd': {
                'RENAME': {
                    'bookmaker': 'bookmaker_slug',
                    'coefficient': 'odds',
                },
                'REMOVE': ['parameter'],
            },
        },
        'melbet': {
            'Event': {
                'ADD': ['raw_url', 'extra (HashMap::new())'],
                'CHECK': ['league should be Option<String>'],
            },
        },
        'tennis': {
            'Event': {
                'ADD': ['raw_url', 'extra (HashMap::new())'],
            },
        },
    }
    
    for parser, fixes in mappings.items():
        print(f"{parser.upper()}:")
        if 'Event' in fixes:
            print("  Event struct fixes:")
            for key, items in fixes['Event'].items():
                if isinstance(items, dict):
                    for old, new in items.items():
                        print(f"    - {key}: {old} -> {new}")
                elif isinstance(items, list):
                    for item in items:
                        print(f"    - {key}: {item}")
        if 'Odd' in fixes:
            print("  Odd struct fixes:")
            for key, items in fixes['Odd'].items():
                if isinstance(items, dict):
                    for old, new in items.items():
                        print(f"    - {key}: {old} -> {new}")
                elif isinstance(items, list):
                    for item in items:
                        print(f"    - {key}: {item}")
        print()


if __name__ == '__main__':
    main()
