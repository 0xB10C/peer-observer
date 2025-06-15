FROM rust:1.87.0-slim-bookworm

# Install dependencies
RUN apt-get update -y && apt-get install -y \
    sudo \
    git \
    protobuf-compiler \
    libelf-dev \
    clang \
    llvm \
    llvm-14 \
    zstd \
    binutils-dev \
    elfutils \
    gcc-multilib

RUN apt-get update -y && apt-get install -y \
    make \
    pkg-config \
    libbpf-dev

# Install bitcoin runtime dependencies
RUN apt-get update && apt-get install -y \
    libevent-dev \
    libboost-system-dev \
    libboost-filesystem-dev \
    libboost-thread-dev \
    libsqlite3-dev \
    libzmq3-dev

RUN apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Create a non-root user and configure sudo
RUN useradd -m -s /bin/bash appuser && \
    echo "appuser ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers.d/appuser

# Switch to non-root user
USER appuser
WORKDIR /home/appuser

RUN rustup component add rustfmt

RUN sudo $(which rustup) default stable

# Copy the local repository to the container
COPY . /home/appuser/peer-observer

# Set working directory to the repository
WORKDIR /home/appuser/peer-observer

# Build the project
RUN cargo build

# docker build -f peer-base.dockerfile -t peer-base .
