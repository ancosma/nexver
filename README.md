# NexVer

Detect next version based on changes in repository.

## Usage

```
Usage: nexver [OPTIONS] [PATH]

Arguments:
  [PATH]  [default: .]

Options:
      --input <INPUT>                      [default: git-tag]
      --input-template <INPUT_TEMPLATE>    [default: ]
      --input-branch <INPUT_BRANCH>        [default: main]
      --output-template <OUTPUT_TEMPLATE>  [default: v{version}]
      --major
      --minor
      --patch
  -c, --conventional-commits
      --set <VARS>
  -h, --help                               Print help information
  -V, --version                            Print version information
```

| Parameter                | Value                               | Default            | Description |
| ------------------------ | ------------------------------------| ------------------ | ----------- |
| `--input`                | git-tag (default)                   | :heavy_check_mark: | Read version from git tag |
| `--input-template`       | 'v{version}' (default)              | :heavy_check_mark: | Template used to parse the version from tag. When not specified, it is same as --output-template |
| `--input-branch`         | 'main' (default)                    | :heavy_check_mark: | (WIP) Used to read tags from or files with version |
| `--set`                  | key=val                             |                    | Set variables which can be used in output template |
| `--output-template`      | 'v{version}' (default)              | :heavy_check_mark: | Template used to render output |
| `--conventional-commits` | (default)                           | :heavy_check_mark: | (WIP) Parse commits to find next version increment |
| `--major`                |                                     |                    | Increment major by 1 and reset minor and patch to 0 |
| `--minor`                |                                     |                    | Increment minor by 1 and reset patch to 0 |
| `--patch`                |                                     |                    | Increment patch by 1 |

For single package per repository add some initial tag to ex. v0.0.1 and run simply `nexver`.

For monorepo run it with path as part of template `nexver --output-template '{path[-1]}-v{version}'` - it will prefix package directory name to the output.

To get version and tag in same output, add it to template as in `--output-template '{path[-1]}-v{version} {version}'`.


## Github Action

### Usage

```yaml
- uses: ancosma/nexver@main
  with:
    # From where to read the version
    # default: git-tag
    input: ''

    # Template used to detect the version
    # default: v{version}
    input-template: ''

    # Branch used to detect version from (tags on the branch)
    # default: main
    input-branch: ''

    # Template used to write next version out
    # default: v{version}
    output-template: ''

    # Manually increment major part
    # default: 'false'
    major: ''

    # Manually increment minor part
    # default: 'false'
    minor: ''

    # Manually increment patch part
    # default: 'false'
    patch: ''

    # Detect version using conventional commits
    # default: 'true'
    conventional-commits: ''

    # Pass extra variables to be used in output template (ex.: build=b1)
    # default: ''
    vars: ''

    # Package location - used to detect changes within that directory (and its children)
    # default: '.'
    working-directory: ''

```

Output: `output` - render the output-template in it.
