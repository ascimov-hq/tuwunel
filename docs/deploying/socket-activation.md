# Systemd socket activation

Systemd can open listening sockets itself and hand them to Tuwunel at startup.
The packages ship a `tuwunel.socket` unit for this, disabled by default.

Two things make it worth using:

- **Privileged ports without privileges.** The shipped service units run with an
  empty `CapabilityBoundingSet=`, so Tuwunel cannot bind a port below 1024 on
  its own. Systemd binds it as PID 1 and passes the open socket down, which is
  how the server reaches 443 or 8448 while holding no capability itself.
- **The socket outlives the server.** `systemctl restart tuwunel` leaves the
  listening socket bound, so clients connecting during the restart wait in the
  accept queue instead of getting a refused connection.

Socket activation needs Linux and a build carrying the `systemd` feature, which
is enabled by default.

## Enabling it

The Debian package installs the unit to `/usr/lib/systemd/system/tuwunel.socket`
and the Red Hat packages under the systemd unit directory. None of them enable
it, so nothing changes until you do. It listens on port 8448, the
federation port, which does not collide with the default configuration:

```bash
systemctl enable --now tuwunel.socket
systemctl restart tuwunel.service
```

The startup log lists every listener, with passed sockets marked:

```
INFO Listening on ["tcp:127.0.0.1:8008", "tcp:[::1]:8008", "passed:tcp:[::]:8448"]
```

To listen somewhere else, override the unit rather than editing it. An empty
`ListenStream=` is required first, since systemd appends to list settings
instead of replacing them:

```bash
systemctl edit tuwunel.socket
```

```ini
[Socket]
ListenStream=
ListenStream=443
```

## How passed sockets combine with the configuration

Passed sockets are served **in addition to** everything in the configuration
file. The `address`, `port` and `unix_socket_path` settings keep working
exactly as before, and Tuwunel binds those itself.

Where the two overlap, the passed socket wins and the configured address is
skipped, with a line saying so:

```
INFO Not binding: a listener already answers for it. addr=127.0.0.1:8448
```

A wildcard address covers every address of its family, and a dual-stack `[::]`
socket covers IPv4 as well, so `ListenStream=8448` takes over the configured
entries on that port however they are written. A passed unix socket matching
`unix_socket_path` is skipped the same way, which also keeps the socket systemd
holds from being unlinked and replaced.

An address you configured that is genuinely unavailable, held by some other
process, fails startup rather than disappearing quietly:

```
There was a problem with the 'address' directive in your configuration:
Failed to bind 127.0.0.1:8448: Address in use (os error 98)
```

Leaving `address` unset takes the built-in default instead, which is only a
guess that both loopback families exist on the host. An address from that guess
is logged at error level and skipped rather than being fatal, so a host without
IPv6 still serves on `127.0.0.1`; startup fails only once no listener is left.

The common shape for a public deployment is a configuration bound to localhost
for a reverse proxy, plus a socket unit for the port that has to be privileged.
To serve *only* passed sockets, point the configuration at a unix socket and
leave `address` unset, which is the one case where Tuwunel binds no TCP address
of its own.

## Direct TLS

Passed sockets carry TLS exactly like bound ones. Setting `tls.certs` and
`tls.key` applies to every listener Tuwunel serves, so a socket unit listening
on 443 serves HTTPS with no further configuration:

```toml
[global.tls]
certs = "/etc/tuwunel/tls/fullchain.pem"
key = "/etc/tuwunel/tls/privkey.pem"
```

`tls.dual_protocol` works on passed sockets too, serving HTTP and HTTPS on the
same passed port.

TLS is a property of the server, not of the individual socket, so passed and
bound listeners cannot use different certificates or mix TLS with plaintext.
Ports that should stay plaintext behind a proxy belong to a separate reverse
proxy, not to a second socket unit.

## Unix sockets

A socket unit can pass a unix socket instead of a port:

```ini
[Socket]
ListenStream=/run/tuwunel/tuwunel.sock
SocketUser=tuwunel
SocketGroup=www-data
SocketMode=0660
RuntimeDirectory=tuwunel
RuntimeDirectoryPreserve=yes
```

`RuntimeDirectory=` is needed in the socket unit because the service unit's own
`RuntimeDirectory=tuwunel` is created when the service starts, which is after
the socket unit has already tried to bind inside it. `RuntimeDirectoryPreserve=`
keeps the directory when the socket stops.

Pointing `unix_socket_path` at the same path is harmless, since the passed
socket takes precedence and the configured one is skipped. A different path
binds a second unix socket of its own, which is a fine way to serve both.

## Restarts

`systemctl restart tuwunel.service` and `systemctl stop tuwunel.service` leave
the socket unit running and the socket bound. The listener survives, and the
server picks the same socket back up when it starts.

The admin command `!admin server restart` is different. It replaces the running
process image in place, and the descriptors systemd passed are closed by that
exec, so the restarted server comes back listening only on the addresses in its
configuration file. The passed sockets return on the next
`systemctl restart tuwunel.service`, which is the restart to prefer on a
socket-activated deployment.
