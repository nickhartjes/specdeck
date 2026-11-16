# Spec Delta

## MODIFIED Requirements

### Requirement: Relay quotas
The TURN relay SHALL limit allocations per credential and in total, and bandwidth per allocation to 20 Mbit/s. Its total bandwidth SHALL be bounded by the allocation quota, not by a separate capacity that would refuse allocations before the quota is reached. A host with the maximum number of viewers SHALL still be able to relay every one. In every deployment the repository ships, the per-credential quota SHALL be lower than the total quota, and the deployments SHALL run the same coturn version; a check SHALL fail when either is not so.

#### Scenario: One credential, many allocations
- **WHEN** a client opens allocations with one credential beyond the per-credential quota
- **THEN** the relay refuses the extra allocations, and other credentials are unaffected

#### Scenario: Host with twenty relayed viewers
- **WHEN** a host has 20 viewers who all connect through a relay with enough relay ports for them
- **THEN** the per-credential quota refuses none of the host's allocations, and all 20 connections are relayed

#### Scenario: Heavy session
- **WHEN** one relayed session sends as fast as it can
- **THEN** it is relayed at no more than about 20 Mbit/s, and other relayed sessions keep their bandwidth

#### Scenario: Busy relay
- **WHEN** as many viewers are connected as the relay has ports for
- **THEN** the relay admits allocations up to its allocation quota, and refuses only beyond it

#### Scenario: One credential on the Kubernetes relay as shipped
- **WHEN** a client opens allocations with one credential on the relay of `deploy/k8s/base`, unchanged
- **THEN** the relay refuses them before that credential holds every allocation the relay has

#### Scenario: Manifests drift apart
- **WHEN** a change gives the Compose and Kubernetes relays different coturn versions, or sets a per-credential quota at or above the total quota
- **THEN** `just check` fails and says which
