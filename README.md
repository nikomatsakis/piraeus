![Piraeus harbor-gate logo](md/images/piraeus.svg)

# Piraeus

`piraeus` is a terminal UI for SSH local port forwards. Its table shows each forward's SSH host, remote port, local URL, and status. Forwards started by `piraeus` stop when you quit.

## Install and run

From crates.io:

```sh
cargo install piraeus
piraeus
```

From this checkout:

```sh
cargo install --path .
piraeus
```

The host picker reads aliases from `~/.ssh/config`. You can also type another SSH host name. OpenSSH handles the connection settings.

## Keys

| Key | Action |
| --- | --- |
| `↑` / `↓`, `j` / `k` | Select a forward in the main table |
| `a` | Add a forward |
| `b` | Open the selected local port in the default browser |
| `x` | Stop and remove the selected forward |
| `r` | Restart all listed forwards |
| `i` | Show details for an error on the selected row |
| `?` | Show keyboard help |
| `q` | Quit from the main table and stop managed forwards |

Click a `http://localhost:PORT` URL in the main table to open that forward in the default browser.

Adding a forward has three stages:

1. Choose an SSH host with `↑` / `↓`, or start typing to use **Other**. Press `Enter`.
2. Choose a recent remote port with `↑` / `↓`, or type digits for another port. Press `Enter`.
3. Choose a local-port option with `↑` / `↓`, or type digits for a manual local port. Press `Enter` to start the forward.

`Esc` goes back one stage; at the host stage it cancels. When the matching local port is occupied, Piraeus offers a free alternative. If the occupying forward is managed by Piraeus, you can choose to replace it.

Recent remote ports persist across runs in `~/.config/piraeus/config.toml`, or `$XDG_CONFIG_HOME/piraeus/config.toml` when `XDG_CONFIG_HOME` is set to an absolute path. When the new file is absent, Piraeus imports existing history from `~/.piraeus.toml`, `~/.pfm.toml`, or `pfm/config.toml` in the selected XDG configuration directory.

## Development

```sh
cargo test --workspace
mdbook test
mdbook build
```

The mdBook in `md/` records the requirements and their Habeas evidence. `cargo test --workspace` includes that validation.

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.
