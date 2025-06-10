#!/bin/bash

# Check if an argument was provided
if [ -z "$1" ]; then
  echo "Usage: $0 <docker-image-name>"
  exit 1
fi

# Run the container using the provided image name
docker run "$1"
