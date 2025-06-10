FROM golang:1.24.4-alpine

# Install nats-server
RUN go install github.com/nats-io/nats-server/v2@main

# Expose NATS default port
EXPOSE 4222

# Run nats-server as the default command
CMD ["nats-server"]

# docker build -f nats.dockerfile -t nats-go .
# docker run -p 4222:4222 nats-go
