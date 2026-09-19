# daemon-generated

An example generated using the daemon template

- `daemon-generatedd(8)`: the daemon
- `daemon-generatedctl(8)`: control program
- `daemon-generatedd.conf(5)`: configuration file

## Design

Two processes: a privileged parent that reads the configuration and supervises,
and an unprivileged engine that runs the work and serves the control socket. The
parent re-executes itself to start the engine and talks to it over a socket pair
with framed messages (type, length, peer id, pid) that can carry a file
descriptor. With `user` set the engine runs as that user inside a `chroot`. The
control socket is created by the parent, mode 0660 owned by root; only `show
status` is answered for other users.

Configuration is TOML; there is no `-D macro=value` option.

## License

[ISC](LICENSE)
