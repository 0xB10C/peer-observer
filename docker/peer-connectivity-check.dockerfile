FROM peer-base

# Set working directory (inherited from base image)
WORKDIR /home/appuser/peer-observer

# Run the connectivity-check binary with sudo
CMD ["sudo", "-E", "/usr/local/cargo/bin/cargo", "run", "--bin", "connectivity-check"]

# docker build -f peer-connectivity-check.dockerfile -t peer-connectivity-check .
# docker run peer-connectivity-check
