# Packaging only: dist/web-wasm64 is built beforehand (cmake --build --preset
# web-wasm64). Build from the repository root:
#   docker build -f Web/Dockerfile -t ghcr.io/cefege/hp2-web:<tag> .
# Contract: serves the complete game at / on port 8080 with the cross-origin
# isolation headers the wasm64 + pthreads build requires.
FROM caddy:2.11.7-alpine

COPY dist/web-wasm64/ /srv/
COPY Web/Caddyfile /etc/caddy/Caddyfile

RUN apk add --no-cache brotli \
 && find /srv -type f \( -name '*.wasm' -o -name '*.js' -o -name '*.html' \) \
      -exec gzip -k -9 {} \; -exec brotli -k -q 11 {} \;

LABEL org.opencontainers.image.source=https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc
EXPOSE 8080
USER 65532:65532
