# TAVRA node agent v1

`tavra-node inspect` emits a nonsecret JSON snapshot without root privileges or changes.
On Linux it reads `/etc/os-release`, `/proc/{loadavg,meminfo,net/dev,net/tcp,net/tcp6,net/udp,net/udp6,sys/kernel/hostname}`, checks `/dev/net/tun`, and uses `statvfs("/")`. No shell, process command lines, IP addresses, environment, credentials, machine-id or private key contents are returned. Hostname accepts only bounded ASCII hostname characters. Network counters are cumulative per interface; load is the 1/5/15 minute average; memory and disk units are bytes. Ports contain only transport and port, deduplicated across IPv4/IPv6. UDP reports unconnected bound sockets, not proof of health. TUN presence is not a permission or forwarding test.

Unavailable reads appear in `unavailable`; missing numeric values are null. On non-Linux systems Linux fields are unavailable, never substituted with invented metrics. Root disk measurements still work on Unix. No historical rates or CPU percentage are claimed. Every protocol capability is currently `supported:false`: no managed VPN runtime has been installed/verified by this agent. Agent health is separate from VPN health.

## Commands and configuration

```
tavra-node inspect
tavra-node check-config --config /etc/tavra/node.json
tavra-node --config /etc/tavra/node.json
# equivalent: tavra-node serve --config /etc/tavra/node.json
```

The service has no implicit config or plaintext mode. Missing/invalid config, key, certificate or CA fails before binding. Errors never echo config/PEM. `Config` has no Debug implementation, and unknown keys are denied.

```json
{
  "bind": "127.0.0.1:9443",
  "server_cert": "/etc/tavra/node-server.crt",
  "server_key": "/etc/tavra/node-server.key",
  "management_client_ca": "/etc/tavra/management-client-ca.crt"
}
```

`bind` is optional and defaults to `127.0.0.1:9443`; all paths are mandatory. For remote management explicitly bind a chosen private address and constrain ingress in deployment. Config/key files should be readable only by the service identity; directories must not be writable by untrusted users. Certificate rotation currently requires a service restart; configuration is not reloaded automatically.

## Trust contract

TLS 1.2/1.3 using rustls is mandatory. The client must verify the server chain and hostname/SAN. Server authentication alone grants no management access: every connection must supply an unexpired, valid client certificate chaining to `management_client_ca`. Configure a **dedicated private management-client CA** that issues certificates only to authorized control-plane/observer clients. Do not use public roots, the node-peer CA, VPN device CA or a bundle containing them. A node-peer certificate signed by a different CA is rejected. Files encode trust roots, not role strings: any certificate accepted by the configured management CA grants read access. Sharing that CA with peer issuance violates this contract. Revocation/roles/jobs are not implemented; remove compromised trust and rotate/restart before relying on revocation.

Only exact `GET /healthz` and `GET /v1/metrics` HTTP/1.1 requests are served. Health JSON: `{"status":"ok","scope":"management-agent","vpn_ready":false}`. Metrics uses the same version-1 typed schema as inspect. Other methods return 405; unknown routes return 404. Responses close each connection and disable caching. There are no shell, install, restart, firewall or job execution endpoints. Header limit: 8192 bytes. One 10-second deadline covers TLS handshake, request read, response write; at most 32 simultaneous connections. Excess connections close immediately.

## Validation and deployment limits

`cargo test -p tavra-node -j 2` verifies proc fixtures, missing/malformed data, strict config, and real loopback TLS with no certificate, an unrelated peer CA, and an authorized management client. The accepted response is deserialized as `Metrics` and checked against actual OS and filesystem data. The integration test uses the production PEM configuration loader.

Linux VM/systemd/package/TUN/actual VPN traffic checks are separate acceptance gates. This component does not enroll identities, install engines, configure routing or authorize mutations. Deployment and CA generation are owned by the primary executor. Do not expose a shared/public management CA to make connectivity pass.
