import sys
import re

def main():
    if len(sys.argv) < 2:
        return
    
    filepath = sys.argv[1]
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()

    parts = content.split('## File Contents\n\n')
    if len(parts) < 2:
        print("Could not find '## File Contents'")
        with open('context.md', 'w', encoding='utf-8') as f:
            f.write(content)
        return

    header = parts[0] + '## File Contents\n\n'
    body = parts[1]

    blocks = re.split(r'(### File: `[^`]+`)', body)

    if len(blocks) < 3:
        with open('context.md', 'w', encoding='utf-8') as f:
            f.write(content)
        return

    parsed_blocks = []
    for i in range(1, len(blocks), 2):
        heading = blocks[i]
        content_block = blocks[i+1]
        
        m = re.search(r'### File: `([^`]+)`', heading)
        fname = m.group(1).replace('\\', '/') if m else ""
        
        parsed_blocks.append((fname, heading + content_block))

    def get_priority(fname):
        if fname == 'src/lib.rs': return 0
        if fname == 'src/main.rs': return 1
        if fname == 'Cargo.toml': return 2
        if fname.startswith('tests/'): return 3
        if fname.startswith('benches/'): return 4
        if fname.startswith('examples/'): return 5
        return 6

    parsed_blocks.sort(key=lambda x: (get_priority(x[0]), x[0]))

    final_content = header + "".join([b[1] for b in parsed_blocks])
    with open('context.md', 'w', encoding='utf-8') as f:
        f.write(final_content)
    print("Reorganized successfully")

if __name__ == "__main__":
    main()
