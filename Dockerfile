FROM --platform=$BUILDPLATFORM alpine:3.23 AS builder-amd64
COPY binaries/seedrelay-linux-amd64/seedrelay /tmp/seedrelay

FROM --platform=$BUILDPLATFORM alpine:3.23 AS builder-arm64
COPY binaries/seedrelay-linux-arm64/seedrelay /tmp/seedrelay

FROM builder-amd64 AS builder-default

FROM alpine:3.23
RUN apk add --no-cache ca-certificates opus
ARG TARGETARCH
COPY --from=builder-${TARGETARCH} /tmp/seedrelay /app/seedrelay
WORKDIR /app
EXPOSE 8000
CMD ["./seedrelay", "--host", "0.0.0.0"]
