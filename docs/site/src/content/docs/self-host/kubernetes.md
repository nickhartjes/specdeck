---
title: Kubernetes
description: Run the sharing service on a Kubernetes cluster.
---

The repository's [`deploy/k8s/base`](https://github.com/nickhartjes/specdeck/tree/main/deploy/k8s/base) is a Kustomize deployment of the signalling service and coturn. Its [README](https://github.com/nickhartjes/specdeck/blob/main/deploy/k8s/README.md) has the details; this page is the outline.

## What to provide

- **The signalling image version.** `base` runs the signed release image `ghcr.io/nickhartjes/specdeck-signalling` at tag `1`, the newest 1.x release. Pin a version (or a digest) in your overlay so pods do not change when they restart:

  ```yaml
  images:
    - name: ghcr.io/nickhartjes/specdeck-signalling
      newTag: 1.0.0
  ```

  To run your own build instead, build it from the repository root, push it to a registry the cluster can pull from, and set `newName` and `newTag` to it:

  ```sh
  docker build -f docker/signalling.Dockerfile -t registry.example.com/specdeck-signalling:1.0.0 .
  ```

- **The TURN secret**, in `deploy/k8s/base/turn-secret.env` (not committed):

  ```sh
  umask 077 && echo "TURN_SECRET=$(openssl rand -hex 32)" > deploy/k8s/base/turn-secret.env
  ```

- **The network settings** in the `specdeck-network` ConfigMap: `TURN_EXTERNAL_IP` (the public address coturn advertises) and `TURN_URLS` (for example `turn:turn.example.com:3478?transport=udp`).
- **An ingress with TLS** for the `specdeck-signalling` Service on port 8080, paths `/signal` and `/health`. The deployment assumes one proxy in front (`SPECDECK_TRUSTED_PROXIES=1`).
- **UDP exposure for coturn**: 3478 and the relay ports 49160-49179, reachable from the internet on the same port numbers coturn advertises. The Service is a `LoadBalancer`; how UDP is exposed (load balancer, node port, host network) is something to agree with the cluster's operators.

```sh
kubectl apply -k deploy/k8s/base     # or your overlay
```

## Good to know

- Run **one** replica of the signalling service: rooms live in memory.
- Kubernetes Services cannot declare port ranges, so every relay port is listed. 20 ports cover about 10 relayed viewers; widen coturn's `--min-port`/`--max-port` and the Service's port list together.
- Both pods meet the `restricted` Pod Security Standard.
- To try it on your own machine, `just local-up` deploys the `local` overlay to Colima's k3s.
