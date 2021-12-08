# rres

> A xrandr replacement to gather display resolutions

## Install

```
cargo install --git https://github.com/rokbma/rres.git
```

## Usage

```
$ rres -h
Usage: rres [options]

  -c, --card <card>	Specify a GPU (file existing in /dev/dri/, eg. card0)
  -m, --multi		Read all monitors. If this option is ommited, rres will
             		return the resolution of the first detected monitor
  -v, --verbose		Verbosity level. Can be specified multiple times, e.g. -vv
  -q, --quiet		Lower verbosity level. Opposite to -v
  -h, --help		Show this help message

Environment variables:

  RRES_DISPLAY=<index>	Select display in single mode (starting at 0)

```

## Contributing

Please speak with me in [Matrix](https://matrix.to/#/!SlYhhmreXjJylcsjfn:tedomum.net?via=matrix.org&via=tedomum.net) before sending PRs.

## License

Licensed under the GPLv3 license.

Copyright (c) 2021 rokbma & the johncena141 hacker group on 1337x.to
