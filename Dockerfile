FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev cmake make gcc g++

WORKDIR /usr/src/seedrelay
COPY . .
RUN CMAKE_POLICY_VERSION_MINIMUM=3.5 cargo build --release

FROM alpine:3.23

RUN apk add --no-cache ca-certificates opus

WORKDIR /app
COPY --from=builder /usr/src/seedrelay/target/release/seedrelay .

EXPOSE 8000

CMD ["./seedrelay", "--host", "0.0.0.0"]
