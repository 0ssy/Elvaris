# Milestone 15 — Networking and Server Profile

## 1. Objective

Give Elvaris a tested networking stack and a minimal server profile. Complete only when all phases pass and the end-to-end scenario works.

Target: QEMU x86-64 with VirtIO-net PCI, Ethernet + ARP + IPv4 + ICMP + UDP + TCP, user-space socket boundary, network permissions, and a minimal echo server.

## 2. Architectural Decisions

### 2.1 Target
- Architecture: x86-64
- Machine: QEMU q35
- Network device: VirtIO-net PCI
- Network mode: QEMU user-mode networking or documented host bridge
- Protocols: Ethernet + ARP + IPv4
- Transports: ICMP + UDP + TCP
- Socket API: Elvaris-native user-space API

### 2.2 Ownership
Kernel: PCI discovery, VirtIO-net driver, DMA/shared buffers, interrupts, packet buffer ownership. User space: network service, address config, protocol logic where practical, sockets, server apps. Keep interface boundary clear for future move to user-space service.

### 2.3 Initial Config
Static configuration: IPv4, subnet mask, gateway, DNS. DHCP deferred.

### 2.4 Transport Order
Ethernet → ARP → IPv4 → ICMP echo → UDP → TCP → User-space sockets → Server application.

## 3. Phases

15.0 Networking design and interfaces  
15.1 Packet buffers and network memory  
15.2 Network device abstraction  
15.3 VirtIO-net driver  
15.4 Ethernet and ARP  
15.5 IPv4 and ICMP  
15.6 UDP and socket foundation  
15.7 TCP and connection management  
15.8 User-space network service and permissions  
15.9 Server profile and remote service  
15.10 Network testing and hardening

## 4. Phase 15.0 — Design and Interfaces

Create docs/milestone-15.md (this file) with documented boundaries:
- Network device, transmit/receive queues, packet buffer, frame, network interface, socket, network service, server application. Shell/apps must not access device registers directly.

## 5. Phase 15.1 — Packet Buffers and Network Memory

Requirements:
- Packet buffer: storage, capacity, length, headroom/tailroom, ownership, alignment, max frame size
- Prevent OOB read/write, overflow, use-after-release, double release
- Exactly one owner unless explicit shared ownership
- Max packet size fixed initially, one buffer per frame acceptable, no fragmentation/jumbo initially

Tests: empty/max/short, header validation, OOB rejection, ownership transfer, double-release rejection, exhaustion, alignment.

## 6. Phase 15.2 — Network Device Abstraction

Interface: init/shutdown, get MAC, transmit/receive, link status, errors, stats. Metadata: name, MAC, MTU, link, features, queue counts. Fake device for unit tests (inject frames, capture tx, errors, link down, queue exhaustion).

## 7. Phase 15.3 — VirtIO-Net Driver

PCI discovery, VirtIO caps, feature negotiation, reset, queues, descriptor ownership, rx buffer provisioning, tx submit, used-ring processing, interrupt/polling, shutdown. Start simple (no multiqueue/offloads/zero-copy). Polling first acceptable. Tests for discovery/init/MAC/tx/rx/errors.

## 8. Phase 15.4 — Ethernet and ARP

Ethernet: MACs, EtherType, lengths, broadcast, reject truncated/invalid/unsupported/oversize. ARP: request/reply, cache (bounded), expiration/replacement. No full ARP security initially, document limitations.

## 9. Phase 15.5 — IPv4 and ICMP

IPv4: header parse/validate (version/length/checksum/addresses/proto/TTL), route to local, build packets. No fragmentation/reassembly/multi-if/routing/options initially. ICMP echo request/reply/checksum. QEMU test: configure interface, send ICMP echo, receive reply.

## 10. Phase 15.6 — UDP and Socket Foundation

UDP: header/ports/length/checksum, rx/tx, bind, collisions, rx queue, size limits. Socket: type/local/remote/state/queues/owner/permissions. Ops: socket/bind/send/receive/close. Nonblocking first. Enforce ownership/permissions/privileged ports.

## 11. Phase 15.7 — TCP and Connection Management

State machine: Closed/Listen/Syn-Sent/Syn-Received/Established/Fin-Wait/Close-Wait/Last-Ack/Closing/Time-Wait (minimal path). Handshake, seq/ack, retransmit, windows, ordering, duplicates, close/reset, listen backlog. No SACK/window scaling/timestamps/advanced congestion/TLS initially. Deterministic fake tests first.

## 12. Phase 15.8 — User-Space Network Service and Permissions

Boundary: app → socket syscalls/IPC → network service → stack → device. Apps no direct PCI/queues/packet sniffing/forgery/config without auth. Integrate with Milestone 14 (creds/privileges/ports/interfaces). Syscalls: socket/bind/listen/accept/connect/send/receive/close/get_network_info or service+IPC.

## 13. Phase 15.9 — Server Profile

Headless server profile: start net service, configure interface, start selected services, restrict by identity, log failures, continue if optional fails, remote admin if documented, clean shutdown. First service: echo server (listen/accept/recv/send/close, logged, owned, permission-checked). No SSH initially.

## 14. Phase 15.10 — Testing and Hardening

Unit tests (packet/buffers/eth/arp/ip/icmp/udp/tcp/ports/sockets/perms/config), fake-device tests (tx/rx/errors/exhaust/link/recovery), QEMU tests (discovery/MAC/init/ARP/IPv4/ICMP/UDP/TCP/server/denials/shutdown), regressions (all previous milestones), malformed-packet tests. Resource limits enforced. Diagnostics with levels, packet dumps debug-only.

## 15. Security Requirements
Validate all lengths/headers/checksums, reject malformed safely, bound queues/tables, prevent overflow/exhaustion, isolate packets per process, enforce ownership/permissions/privileged ports/interface config, no sensitive data in logs, malformed packet must not crash kernel.

## 16. Resource Limits
Packet size, rx/tx queue length, ARP entries, sockets per process/total, listen backlog, TCP connections, UDP datagram size, network service memory. Enforce and return defined errors.

## 17. Out of Scope
Wi-Fi/USB/physical NICs, ARM64 drivers, IPv6, DHCP, DNS, TLS, SSH, VPN, BT, VLAN, namespaces/containers, advanced firewall/IDS, offloads/zero-copy/multiqueue/routing between interfaces, full POSIX compat, advanced congestion control.

## 18. Exit Criteria
All items checked: design/docs, packet buffers tested, device abstraction+fake device, VirtIO-net init/MAC/tx/rx, Ethernet/ARP/IPv4/ICMP/UDP/TCP working, socket boundary+ownership+permissions, server listen/accept/recv/send, error bounds/limits/observability, QEMU tests pass, all previous tests pass, malformed-packet tests pass, limitations documented.

## 19. Definition of Done
End-to-end: boot server profile → discover/init VirtIO → configure static IP → ARP → ICMP echo → start net service + echo server → remote client connects → recv/send/close → unauthorized op denied/logged → clean shutdown.

**Rule:** Complete and test each layer in order (packet buffers → device abstraction → fake device tests → VirtIO driver → Ethernet/ARP → IPv4/ICMP → UDP/sockets → TCP → service/permissions → server) before moving to next.
