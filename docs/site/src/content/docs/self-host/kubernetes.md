---
title: Kubernetes
description: Run the sharing service on a Kubernetes cluster.
---

The repository's [`deploy/k8s/base`](https://github.com/nickhartjes/specdeck/tree/main/deploy/k8s/base) is a Kustomize deployment of the SpecDeck server and coturn. Its [README](https://github.com/nickhartjes/specdeck/blob/main/deploy/k8s/README.md) has the details; this page is the outline.

## What to provide

- **The server image version.** `base` runs the signed release image `ghcr.io/nickhartjes/specdeck-server` at tag `1`, the newest 1.x release. Pin a version (or a digest) in your overlay so pods do not change when they restart:

  ```yaml
  images:
    - name: ghcr.io/nickhartjes/specdeck-server
      newTag: 1.7.0
  ```

  To run your own build instead, build it from the repository root, push it to a registry the cluster can pull from, and set `newName` and `newTag` to it:

  ```sh
  docker build -f docker/server.Dockerfile -t registry.example.com/specdeck-server:1.7.0 .
  ```

- **The TURN secret**, in `deploy/k8s/base/turn-secret.env` (not committed):

  ```sh
  umask 077 && echo "TURN_SECRET=$(openssl rand -hex 32)" > deploy/k8s/base/turn-secret.env
  ```

- **The network settings** in the `specdeck-network` ConfigMap: `TURN_EXTERNAL_IP` (the public address coturn advertises) and `TURN_URLS` (for example `turn:turn.example.com:3478?transport=udp`).
- **An ingress with TLS** for the `specdeck-server` Service on port 8080, for all paths: `/signal` for the apps, `/` and `/join` for the web viewer, and `/health`. The deployment assumes one proxy in front (`SPECDECK_TRUSTED_PROXIES=1`).
- **UDP exposure for coturn**: 3478 and the relay ports 49160-49179, reachable from the internet on the same port numbers coturn advertises. The Service is a `LoadBalancer`; how UDP is exposed (load balancer, node port, host network) is something to agree with the cluster's operators.

```sh
kubectl apply -k deploy/k8s/base     # or your overlay
```

## Good to know

- Run **one** replica of the server: rooms live in memory.
- Kubernetes Services cannot declare port ranges, so every relay port is listed. 20 ports cover about 10 relayed viewers; widen coturn's `--min-port`/`--max-port`, `--total-quota` and the Service's port list together.
- coturn limits each relayed connection to 20 Mbit/s (`--max-bps`, in bytes per second), so the relay's total is at most `--total-quota` × 20 Mbit/s.
- Both pods meet the `restricted` Pod Security Standard.
- To try it on your own machine, `just local-up` deploys the `local` overlay to Colima's k3s.

## Which version runs

From 1.7 on, the server states the release it runs. After a rollout, check it through your Ingress:

```sh
curl https://specdeck.example.com/web-viewer.json   # prints, for example: {"protocol":6,"version":"1.7.0"}
```

## Upgrading from 1.6

In 1.7 the Deployment and Service were renamed from `specdeck-signalling` to `specdeck-server`, and the image to `ghcr.io/nickhartjes/specdeck-server`. Kubernetes cannot rename them in place, so the upgrade creates the new ones next to the old ones; the old ones keep serving until you switch over.

1. If your overlay pins the image, change the name in its `images` entry to `ghcr.io/nickhartjes/specdeck-server`. Then apply:

   ```sh
   kubectl apply -k deploy/k8s/base     # or your overlay
   kubectl rollout status deployment/specdeck-server
   ```

2. Point your Ingress's backend at the `specdeck-server` Service (port 8080) instead of `specdeck-signalling`, and apply it. Check that the apps and the web viewer connect, and that `/web-viewer.json` gives the new version.
3. Remove the old Deployment and Service:

   ```sh
   kubectl delete deployment,service specdeck-signalling
   ```

Do steps 2 and 3 right after each other. Rooms live in memory, so while both run, a viewer who reaches the new server does not find a host that is still connected to the old one. Once the old one is gone, hosts reconnect to the new server by themselves. Sessions that are already connected keep running throughout.
