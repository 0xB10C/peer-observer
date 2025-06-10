FROM peer-base

# Set working directory (inherited from base image)
WORKDIR /home/appuser/peer-observer

# Run the logger binary with sudo
CMD ["sudo", "-E", "/usr/local/cargo/bin/cargo", "run", "--bin", "logger"]

# docker build -f peer-logger.dockerfile -t peer-logger .
# docker run peer-logger
