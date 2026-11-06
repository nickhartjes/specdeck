# Kubernetes deployment

SpecDeck's server side: the SpecDeck server (matchmaking, TURN credentials and the web viewer) and coturn (the TURN relay). Neither can read session content; see design decisions 8 and 9 in [`design.md`](../../openspec/changes/archive/2026-10-31-native-shared-session-mvp/design.md).

| Path | Contents |
|---|---|
| [`base`](base) | The deployment for a real cluster |
| [`local`](local) | Overlay for Colima's local k3s, used by `just local-up` and `just local-test` |

## Image

`base` runs the server image published with every release, `ghcr.io/nickhartjes/specdeck-server`, at tag `1` (the newest 1.x release). For production, pin a version in your overlay so pods do not change on a restart:

```yaml
images:
  - name: ghcr.io/nickhartjes/specdeck-server
    newTag: 1.7.0        # or: digest: sha256:...
```

Each image is signed by the release workflow; check it with `cosign verify` as in the [self-hosting overview](https://nickhartjes.github.io/specdeck/self-host/overview/#the-web-viewers-code). To run an image you built yourself, push it to your registry and set `newName` and `newTag` there instead.

Up to 1.6 the image, Deployment and Service were called `specdeck-signalling`. To upgrade a cluster from then, follow [Upgrading from 1.6](https://nickhartjes.github.io/specdeck/self-host/kubernetes/#upgrading-from-16).

## Secrets

Both services share one TURN secret: the server signs short-lived TURN credentials with it, and coturn checks them (TURN REST API, `use-auth-secret`).

Create `base/turn-secret.env` before deploying; it is not committed:

```sh
umask 077 && echo "TURN_SECRET=$(openssl rand -hex 32)" > deploy/k8s/base/turn-secret.env
```

`just local-up` creates it if it is missing. Kustomize turns it into the Secret `specdeck-turn`.

## Configuration

Set in the ConfigMap `specdeck-network` ([`base/kustomization.yaml`](base/kustomization.yaml)):

| Key | Meaning |
|---|---|
| `TURN_EXTERNAL_IP` | Public address of the TURN service, which coturn advertises in relayed addresses |
| `TURN_URLS` | TURN URLs as the apps reach them, comma-separated, for example `turn:turn.example.com:3478?transport=udp` |

The server reads its settings from the environment (see [`crates/server/src/main.rs`](../../crates/server/src/main.rs)). In `base`, `SPECDECK_TRUSTED_PROXIES` is `1` because the service runs behind the cluster's ingress: the client's address for rate limiting is then taken from `X-Forwarded-For`. Set it to the number of proxies in front of the service.

## Ports

| Service | Port | Exposure |
|---|---|---|
| Server | 8080/TCP, all paths: `/signal` (WebSocket), the web viewer (`/`, `/join`) and `/health` | Through the cluster's ingress with TLS; apps connect to `wss://<host>/signal` |
| TURN | 3478/UDP | Reachable from the internet |
| TURN relay ports | 49160-49179/UDP | Reachable from the internet, on the same port numbers coturn advertises |

TURN is UDP only: the app's WebRTC library (libdatachannel with libjuice) does not support TURN over TCP or TLS. Viewers on networks that block UDP entirely cannot connect.

Kubernetes Services cannot declare port ranges, so [`base/coturn.yaml`](base/coturn.yaml) lists every relay port. Each relayed viewer uses a relay port on the host's side and possibly one on its own side, so 20 ports cover about 10 relayed viewers at once; widen `--min-port`/`--max-port` and the Service's port list together. How the cluster exposes UDP (LoadBalancer, NodePort, host network) has to be agreed with the cluster operators; adjust the Service type accordingly.

coturn refuses to relay to private and link-local address ranges (`--denied-peer-ip`), so TURN credentials cannot be used to reach services inside the cluster. The one exception is the pod's own address, so two relayed peers can meet on the same coturn.

## Unprivileged pods

Both pods meet the Kubernetes `restricted` Pod Security Standard: they run as a non-root user (the server as 10001, coturn as `nobody`, 65534) on a read-only root filesystem, without privilege escalation, with the runtime's default seccomp profile and with all capabilities dropped. coturn keeps only `NET_BIND_SERVICE`, because its binary carries that file capability and cannot be started without it; its pid file goes to an `emptyDir`. The local namespace enforces the level (`pod-security.kubernetes.io/enforce: restricted`), so a change that needs more privileges fails to deploy there. Label your production namespace the same way if the cluster does not already enforce it.

## Local deployment

Needs Colima with Kubernetes (`colima start --kubernetes`), whose k3s uses Colima's Docker, so the server image is built locally and never pulled.

```sh
just local-up      # build the image, deploy to the namespace specdeck1-local
just local-test    # two peers connect only through the relay
just local-down    # remove the deployment
```

The overlay differs from `base` in a few places:

- Namespace `specdeck1-local` and TURN on node port **3480**, so it does not clash with another TURN server on 3478.
- Apps reach TURN at the node's address, for example `192.168.64.2:3480`. On macOS, [`scripts/local-kubectl.sh`](../../scripts/local-kubectl.sh) talks to the API server through the port Colima forwards to localhost, because macOS's Local Network privacy blocks most programs from reaching the VM's address.
- Both test peers are relayed through the one coturn, so relay traffic stays inside the pod: coturn advertises the pod's address and the relay ports (51000-51999) are not exposed.
- No ingress: the server is reached with a port-forward, so `SPECDECK_TRUSTED_PROXIES` is `0`.
