# Build stage
FROM ubuntu:22.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive
ARG BTC_CORE_TAG=v29.0

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    pkgconf \
    python3 \
    libevent-dev \
    libboost-dev \
    libsqlite3-dev \
    systemtap-sdt-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

# Clone Bitcoin Core repository
WORKDIR /bitcoin
RUN git clone https://github.com/bitcoin/bitcoin.git .
RUN git checkout $BTC_CORE_TAG

# Build Bitcoin Core
RUN cmake -B build -DBUILD_GUI=OFF -DWITH_USDT=ON \
    && cmake --build build -j$(nproc)

# Runtime stage
FROM ubuntu:22.04 AS runtime

ENV DEBIAN_FRONTEND=noninteractive

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libevent-dev \
    libboost-system-dev \
    libboost-filesystem-dev \
    libboost-thread-dev \
    libsqlite3-dev \
    libzmq3-dev \
    && rm -rf /var/lib/apt/lists/*

# Create bitcoin user and directories
RUN useradd -r -m -s /bin/bash bitcoin \
    && mkdir -p /home/bitcoin/.bitcoin \
    && mkdir -p /shared \
    && chown -R bitcoin:bitcoin /home/bitcoin /shared

# Copy everything from compiled binaries from builder
COPY --from=builder /bitcoin/build/bin/ /usr/local/bin/
COPY --from=builder /bitcoin/build/bin/ /shared/

# Switch to bitcoin user
USER bitcoin

# Expose Bitcoin ports (RPC: 8332, P2P: 8333)
EXPOSE 8332 8333

# Set data directory and shared volume
VOLUME /home/bitcoin/.bitcoin
VOLUME /shared

# Default command to run bitcoind in regtest mode
ENTRYPOINT ["bitcoind"]
CMD ["-regtest", "-printtoconsole"]