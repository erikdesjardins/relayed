# nproxy

Proxy traffic to a machine behind a dynamic IP/firewall. Simpler and less secure than a VPN.

## Example

On a publicly-exposed machine (IP `1.2.3.4`):

```bash
nproxy server 0.0.0.0:8080 0.0.0.0:3000
```

On the private machine:

```bash
nproxy client 1.2.3.4:3000 127.0.0.1:80
```

Which will proxy TCP from `1.2.3.4:8080` to `localhost:80` on the private machine.
