# Live acceptance — 27 October 2026

Authorized test hosts only. Primary executor performed all host mutations after read-only inventory; host keys were pinned at first contact after the user delegated that decision. Independent provider-console verification was unavailable. Subsequent SSH connections enforce strict pinned host-key checking, with ControlPath=none.

## Management agents: PASS

| Host | Observed OS | Agent bind | mTLS health/metrics | Negative trust | Fresh SSH/Zabbix |
|---|---|---|---|---|---|
| 46.19.66.26 | Ubuntu26.04.1 x86_64 | 127.0.0.1:9443 | PASS | no client cert/wrong server CA rejected | PASS |
| 91.210.170.233 | Ubuntu24.04.5 x86_64 | 127.0.0.1:9443 | PASS | no client cert/wrong server CA rejected | PASS |

Installed tavra-node_0.1.0_amd64.deb from CI run40412863029, sourcead9115951e0b8d16db6aca60edd340e4e11c1770. SHA256: `24a6e2017588822387e5820155209274ffc7ae3d5ad2c6211b95a4fb05c7bf1a`.

The package passed installation/removal in Ubuntu22/24/26 CI containers. Actual systemd service runs as unprivileged tavra on both live hosts. Queries used loopback-only SSH forwarding with independently issued management client certificates. Real memory/load/disk/network/listening-port metrics were returned; no fabricated VPN readiness. Routes match baseline, SSH and Zabbix remain active. No OS upgrade, reboot, firewall flush, default-route/NAT/forwarding change or existing application replacement.

Strict certificate validation initially rejected a test leaf missing Authority Key Identifier. Deployment automatically rolled back, then certificates were reissued with SKI/AKI and passed openssl -x509_strict before redeployment. Certificate validation was never disabled.

These are short-lived private test certificates, expiring2 November2026 UTC; certificate renewal and production PKI automation remain pending. CA private keys never leave the coordinator. Node private identity/config is root:tavra0640, private staging root0700. Operator rollback on each host: /root/tavra-test/rollback-node.sh. It removes this test agent/config, preserving package-created account/state as documented by packaging. No control API or client VPN is publicly exposed by this step.

## Own-protocol TUN acceptance

PASS: a real exclusive, nonpersistent Linux TUN pair10.250.0.1↔10.250.0.2 carried authenticated/encrypted TAVRA Link packets across the two public test hosts. Each direction: ping3×1000-byte payload,3 replies,0% packet loss. Only the private peer /32 routes were created; no default route, NAT, Internet forwarding or firewall rule.

Six negative live cases rejected without TUN creation observed: wrong TLS root, wrong server name, expired lease, wrong node audience, wrong epoch and wrong device key. Each case used a fresh single-admission listener.

A finite35-second signed lease expired during active packet flow. Last successful packet was2 seconds before expiry; both TUN interfaces were absent1 second after expiry, ruling out the15-second idle timeout as the cause. A separate admitted pair was stopped with SIGTERM, and both interfaces disappeared. Fresh independent SSH and SSH/Zabbix/node service checks passed afterward on both hosts; the route tables exactly matched their pre-test baselines. Test processes and rollback timers were stopped. Private root-owned fixture files/binaries remain in /root/tavra-test; the loopback monitoring agents remain active.

The experimental runtime is an isolated packet path, distinct from a full client VPN. No application-selected system VPN, multi-user daemon, IPv6/default-traffic routing, regional DPI camouflage or production performance claim is established by this test.
