import sys

def apply_merge(filepath, merge_file):
    with open(filepath, 'r') as f:
        content = f.read()

    with open(merge_file, 'r') as f:
        merge_content = f.read()

    search = merge_content.split('<<<<<<< SEARCH\n')[1].split('\n=======\n')[0]
    replace = merge_content.split('\n=======\n')[1].split('\n>>>>>>> REPLACE')[0]

    if search in content:
        content = content.replace(search, replace)
        with open(filepath, 'w') as f:
            f.write(content)
        print("Success")
    else:
        print("Search string not found in content")

apply_merge('crates/crepuscularity-cli/src/web_serve.rs', 'patch_file')
