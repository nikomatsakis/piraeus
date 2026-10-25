# Requirements

Capitalized requirement terms in this chapter use the meanings defined by BCP 14 ([RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) and [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174)).

:::{req} tui.terminal
The application MUST run as an interactive text user interface in a terminal.
:::

:::{req} tui.table
The main view MUST be a table of port forwards managed by the application. Each row MUST show the SSH host, remote port, actual local port, and current state. A failed forwarding attempt MUST remain in the table so its error is visible.

For example:

| SSH host | Remote port | Local port | State |
| --- | ---: | ---: | --- |
| chronos | 3000 | 3000 | ✅ Running |
| mnemosyne | 8080 | 8081 | 🆇 Error: SSH connection failed |
:::

:::{req} tui.selection
The application MUST let the user select a table row with the keyboard. Actions that refer to a selected port forward MUST use the visibly selected row.
:::

:::{req} forwarding.ssh
The application MUST create local port forwards through SSH, connecting a port on localhost to the same service port on the selected remote SSH host. It MUST track the SSH processes it starts so that subsequent actions affect only port forwards it manages.

For example, a forward for `chronos` remote port `3000` using local port `3001` makes the remote service available at `localhost:3001`.
:::

:::{req} forwarding.startup_state
The table MUST show a new or refreshed forward as running only after its SSH process has established the chosen local listener. If startup fails, the row MUST show an error for the attempted host and ports.
:::

:::{req} forwarding.add
Pressing `a` MUST open an add-forward dialog. The dialog MUST let the user select an SSH host name discovered from the user's SSH configuration (for example, `chronos` or `mnemosyne`), enter a remote port, and select exactly one of the offered radio-style local-port choices with the keyboard. Confirming the dialog MUST create the selected port forward and add it to the table.
:::

:::{req} forwarding.port_validation
The add-forward dialog MUST accept only whole-number remote and manually entered local ports from 1 through 65535. It MUST explain invalid input inside the dialog and MUST NOT start or replace a forward while the input is invalid.
:::

:::{req} forwarding.host_discovery
The host picker MUST use `ssh2-config` to read `Host` declarations in the user's `~/.ssh/config` and find selectable host names. It MUST use the parsed configuration only to collect literal names; wildcard patterns and negated patterns MUST NOT appear as selectable hosts. For example, `Host chronos mnemosyne` contributes two names, while `Host *` contributes none.
:::

:::{req} forwarding.ssh_config
The application MUST delegate interpretation of SSH configuration settings to OpenSSH commands: `ssh` when connecting and `ssh -G <host>` when it needs the effective configuration for a supplied host name. Apart from discovering names in `Host` declarations, it MUST NOT use the parsed configuration to determine connection settings.
:::

:::{req} forwarding.default_local_port
When the entered remote port is free on localhost, the add-forward dialog MUST initially select that same port as the local port and MUST also offer a user-specified alternative.

```text
(x) Use local port 8030
( ) Use alternative local port [       ]
```
:::

:::{req} forwarding.manual_local_port
In every add-forward case, the dialog MUST offer a radio choice that lets the user enter an alternative **local** port explicitly. The entered remote port MUST remain unchanged when the user chooses an alternative local port.
:::

:::{req} forwarding.manual_conflict
If a manually entered local port is already occupied, the add-forward dialog MUST show that conflict and require the user to choose a free port before confirming. It MUST NOT stop the occupying process.
:::

:::{req} forwarding.detect_conflict
Before presenting local-port choices, the application MUST determine whether the preferred local port is free, occupied by a pfm-managed forward, or occupied by another process. On a conflict, it MUST suggest the first free local port found by incrementing the preferred port repeatedly (for example, checking `8031`, then `8032` after a conflict on `8030`).
:::

:::{req} forwarding.suggestion_exhausted
The suggested-port search MUST stop at port 65535. If it finds no free higher port, the add-forward dialog MUST explain that no suggested port is available and MUST still allow manual port entry.
:::

:::{req} forwarding.managed_conflict_choices
If a pfm-managed forward occupies the preferred local port, the dialog MUST offer three mutually exclusive choices: replace that forward on the preferred port, use the suggested free port, or enter an alternative local port. It MUST initially select the suggested free port. If the user chooses replacement, the application MUST stop the existing managed SSH process before starting the new forward and MUST update the table so the replaced forward is no longer shown as running.

```text
( ) Use local port 8030, replacing existing forward
(x) Use suggested alternative local port 8031
( ) Use alternative local port [       ]
```
:::

:::{req} forwarding.unmanaged_conflict_choices
If a process outside pfm occupies the preferred local port, the dialog MUST offer the suggested free port and a user-specified alternative. It MUST initially select the suggested free port, MUST NOT offer to replace the other process, and MUST NOT stop it.

```text
(x) Use suggested alternative local port 8031
( ) Use alternative local port [       ]
```
:::

:::{req} forwarding.remove
Pressing `x` MUST stop the selected managed port forward, if it is running, and remove its row from the table.
:::

:::{req} forwarding.refresh
Pressing `r` MUST stop the SSH processes for all listed port forwards that are running and attempt to recreate every listed port forward, including those currently in an error state.
:::

:::{req} forwarding.quit
Pressing `q` MUST stop all port forwards managed by the application and then exit the TUI.
:::

:::{req} browser.open
Pressing `b` MUST open `http://localhost:P` in the system's default browser, where `P` is the actual local port of the selected port forward.
:::

:::{req} errors.visible
When an action on a port forward fails, or its SSH process stops unexpectedly, the affected table row MUST show a brief error and be highlighted in red. Failures MUST NOT interrupt the user with a modal dialog.
:::

:::{req} errors.info
Pressing `i` on a row with an error MUST show more diagnostic detail within the TUI without opening a modal dialog.
:::

:::{req} help.shortcuts
Pressing `?` MUST show the available keystrokes and their actions, including how to select a row and the `a`, `b`, `x`, `q`, `r`, and `i` commands.
:::
