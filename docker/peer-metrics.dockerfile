FROM peer-base

# Set working directory (inherited from base image)
WORKDIR /home/appuser/peer-observer

# Run the metrics binary with sudo
CMD ["sudo", "-E", "/usr/local/cargo/bin/cargo", "run", "--bin", "metrics", "--", "--nats-address", "nats://nats:4222"]

# docker build -f peer-metrics.dockerfile -t peer-metrics .
# docker run peer-metrics
