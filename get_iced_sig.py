import os
import subprocess

def find_iced_signature():
    cargo_home = os.environ.get('CARGO_HOME', os.path.expanduser('~/.cargo'))
    registry_src = os.path.join(cargo_home, 'registry', 'src')
    
    for root, dirs, files in os.walk(registry_src):
        if 'iced-0.14' in root or 'iced-0.13' in root:
            if 'application.rs' in files:
                filepath = os.path.join(root, 'application.rs')
                with open(filepath, 'r', encoding='utf-8') as f:
                    content = f.read()
                    print(f"Found {filepath}")
                    lines = content.split('\n')
                    for i, l in enumerate(lines):
                        if 'pub fn application<' in l or 'pub fn application(' in l:
                            print("\n".join(lines[i-2:i+15]))
                            return
    print("Could not find iced application source.")

find_iced_signature()
