FROM golang:1.24.4-alpine

# Install curl
RUN apk add --no-cache curl

# Install nats-server
RUN go install github.com/nats-io/nats-server/v2@main

# Expose NATS default port
EXPOSE 4222 8222

# Run nats-server as the default command
CMD ["nats-server", "-m", "8222"]

# docker build -f nats.dockerfile -t nats .
# docker run -p 4222:4222 nats
