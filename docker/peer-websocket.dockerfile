FROM peer-base

# Set working directory (inherited from base image)
WORKDIR /home/appuser/peer-observer

# Run the websocket binary with sudo
CMD ["sudo", "-E", "/usr/local/cargo/bin/cargo", "run", "--bin", "websocket"]

# docker build -f peer-websocket.dockerfile -t peer-websocket .
# docker run peer-websocket
