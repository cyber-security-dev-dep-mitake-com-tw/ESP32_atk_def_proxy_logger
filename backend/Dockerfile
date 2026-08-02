# Multi-stage build for the Go control agent. The PCAP writer uses gopacket's
# pure-Go pcapgo (no libpcap), so we build a fully static binary and ship it on a
# minimal base.
FROM golang:1.25-alpine AS build
WORKDIR /src
# Cache module downloads.
COPY go.mod go.sum ./
RUN go mod download
COPY . .
ARG VERSION=docker
RUN CGO_ENABLED=0 go build -ldflags "-s -w -X main.version=${VERSION}" -o /out/agent ./cmd/agent

FROM alpine:3.20
RUN adduser -D -u 10001 agent
# Data dir for pcap + event logs; mount a volume here to persist captures.
RUN mkdir -p /data && chown agent /data
USER agent
COPY --from=build /out/agent /usr/local/bin/agent
EXPOSE 8080
VOLUME ["/data"]
# Default to demo mode so `docker run` works with no hardware. Override the command
# for real nodes, e.g. `--node1-serial /dev/ttyUSB0 --own-bssids AA:BB:...`.
ENTRYPOINT ["agent"]
CMD ["--demo", "--addr", ":8080", "--data", "/data"]
