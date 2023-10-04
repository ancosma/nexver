# NexVer

Detect next version based on changes in repository.

## Usage

```
Usage: nexver [OPTIONS] [PATH]

Arguments:
  [PATH]  [default: .]

Options:
      --base-ref <BASE_REF>                [default: not-in-use]
      --head-ref <HEAD_REF>                [default: HEAD]
      --input-template <INPUT_TEMPLATE>    [default: ]
      --output-template <OUTPUT_TEMPLATE>  [default: v{version}]
      --set <VARS>
      --major-types <MAJOR_TYPES>
      --minor-types <MINOR_TYPES>...       [default: feat]
      --patch-types <PATCH_TYPES>...       [default: fix]
  -h, --help                               Print help
  -V, --version                            Print version
```

For single package per repository add some initial tag to repository (ex. v0.0.1) and run simply `nexver`.

For monorepo run it with path as part of template `nexver --output-template '{path[-1]}-v{version}'` - it will prefix package directory name to the output.

To get version and tag in same output, add it to template as in `--output-template '{path[-1]}-v{version} {version}'`.


## Github Action

### Usage

```yaml
- uses: ancosma/nexver@main
  with:
    # Git base ref
    # default: main
    base-ref: ''

    # Git head ref
    # default: HEAD
    head-ref: ''

    # Template used to detect the version
    # default: v{version}
    input-template: ''

    # Template used to write next version out
    # default: v{version}
    output-template: ''

    # Define types which increment major number
    # default: ''
    major-types: ''

    # Define types which increment minor number (ex: feat,chore)
    # default: 'feat'
    minor-types: ''

    # Define types which increment patch number (ex: fix,docs)
    # default: 'fix'
    patch-types: ''

    # Pass extra variables to be used in output template (ex: build=b1,info=something)
    # default: ''
    vars: ''

    # Package location - used to detect changes within that directory (and its children)
    # default: '.'
    working-directory: ''
```

Output: `output` - render the output-template in it.
