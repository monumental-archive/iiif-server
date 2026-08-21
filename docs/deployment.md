# Deployment recipes

The engine is deliberately narrow: stateless, no TLS, no auth, no
derivative cache. Each of those is one proven layer in front of it.

## Running

### Container

The official image is `ghcr.io/monumental-archive/iiif-server` — one
static binary and a certificate bundle on nothing else. No shell, no
package manager, no distro, about 6 MB to pull and 16–18 MB unpacked
depending on architecture — under a
ceiling the release enforces on the published bytes, so the figure cannot
drift away from the artifact.

```bash
docker run --rm -p 6363:6363 -v ./masters:/imageroot:ro \
    ghcr.io/monumental-archive/iiif-server
```

Compose, digest-pinned as a deployment should be. Renovate keeps the digest
current:

```yaml
services:
  images:
    image: ghcr.io/monumental-archive/iiif-server@sha256:...
    command: ["serve", "/imageroot", "--bind", "0.0.0.0:6363"]
    volumes: ["./masters:/imageroot:ro"]
    ports: ["6363:6363"]
    # All true of this image, and worth asserting: the server is stateless,
    # needs no writable filesystem, and drops every capability.
    read_only: true
    cap_drop: [ALL]
    security_opt: ["no-new-privileges:true"]
```

The image declares its own `HEALTHCHECK`, which is why `iiif-server
healthcheck` exists: there is no shell in the image for a probe to use, so the
binary probes itself. Under Kubernetes use an ordinary `httpGet` probe against
`/healthz` instead — that needs nothing inside the container.

Object storage works the same way, with credentials from the environment:

```bash
docker run --rm -p 6363:6363 -e AWS_ACCESS_KEY_ID -e AWS_SECRET_ACCESS_KEY \
    ghcr.io/monumental-archive/iiif-server \
    serve s3://bucket/prefix --endpoint https://objects.example.com
```

Before deploying, audit a collection with the offline inspector — no server,
no config:

```bash
docker run --rm -v ./masters:/imageroot:ro \
    ghcr.io/monumental-archive/iiif-server check /imageroot
```

### Verifying what you pulled

The image is signed keylessly by the organisation's signer — a repository
that runs no build code and holds the only signing identity on the
release path — so the identity you require is the signer's workflow, not
this repository's:

```bash
gh attestation verify oci://ghcr.io/monumental-archive/iiif-server:v0.2.0 \
    --owner monumental-archive \
    --signer-workflow monumental-archive/signer/.github/workflows/sign.yml \
    --source-ref refs/tags/v0.2.0 \
    --deny-self-hosted-runners
```

### There is no binary download

From v0.2.0 the image is the only published artifact. Standalone
binaries, their checksums and the installer script are not published: an
artifact class is a standing promise of evidence, and a second one whose
artifacts nobody pulls is evidence owed for nothing. Run the container,
or build from source with the pinned toolchain (`mise install && cargo
build --release --locked --bin iiif-server`).

The invocations are the same either way:

```bash
iiif-server serve ./images
```

```bash
iiif-server serve s3://bucket/prefix --endpoint https://objects.example.com
```

Credentials come from the environment (`AWS_ACCESS_KEY_ID`,
`AWS_SECRET_ACCESS_KEY`, or the platform's IMDS/IRSA/workload-identity
machinery — `object_store` owns that swamp). The only other knobs:

| Flag | Default | Meaning |
| --- | --- | --- |
| `--bind` | `127.0.0.1:6363` | listen address |
| `--public-base` | from Host header | scheme+authority used in `id`/`@id` and canonical links |
| `--max-width/--max-height` | 8192 | published and enforced size limits |
| `--max-area` | 33554432 (32 MP) | published and enforced area limit |
| `--workers` | CPU count | concurrent decode bound |
| `--queue-depth` | 64 | admitted waiters beyond the workers; overflow → 503 + Retry-After |
| `--endpoint` | — | S3-compatible endpoint URL |

Endpoints: `/iiif/3/…` (Image API 3.0), `/iiif/2/…` (Image API 2.1),
`/healthz`, `/metrics` (Prometheus text).

Before pointing the server at a collection, run the offline inspector —
it prints per-master serving advice with copy-paste fixes:

```bash
iiif-server check ./images
```

## TLS + caching: any CDN or reverse proxy

Derivatives are immutable per canonical URL and carry strong ETags, so
ordinary HTTP caching does all the work. nginx sketch:

```nginx
proxy_cache_path /var/cache/iiif keys_zone=iiif:64m max_size=20g inactive=30d;

server {
    listen 443 ssl;
    # ssl_certificate …; ssl_certificate_key …;

    location /iiif/ {
        proxy_pass http://127.0.0.1:6363;
        proxy_cache iiif;
        proxy_cache_valid 200 30d;
        proxy_cache_use_stale error timeout updating;
        proxy_cache_lock on;      # collapse concurrent misses per tile
    }
}
```

A CDN in front (any of them) works the same way: honor `Cache-Control`
and `ETag`, key on the full path. `503 + Retry-After` from the engine
means the decode pool is saturated — let it surface rather than retrying
instantly.

## Access control: forward-auth at the proxy

The engine serves whatever it is asked for; the proxy decides who asks.
nginx `auth_request` sketch:

```nginx
location /iiif/ {
    auth_request /_authz;
    proxy_pass http://127.0.0.1:6363;
}

location = /_authz {
    internal;
    proxy_pass http://auth-service/check;
    proxy_pass_request_body off;
    proxy_set_header X-Original-URI $request_uri;
}
```

Traefik ForwardAuth / Caddy `forward_auth` are equivalent. Per-image
policy belongs to the auth service, which sees the original URI
(identifier included).

## Systemd sketch

```ini
[Service]
ExecStart=/usr/local/bin/iiif-server serve /srv/images --bind 127.0.0.1:6363 \
    --public-base https://images.example.org
Restart=on-failure
DynamicUser=yes
ProtectSystem=strict
ReadOnlyPaths=/srv/images
NoNewPrivileges=yes
```

The binary is static (musl) and needs no shared libraries, no config
file, and no writable filesystem — download it from the release, or copy it
out of the container image, which is the same build.
