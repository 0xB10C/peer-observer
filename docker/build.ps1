docker build -f peer-base.dockerfile -t peer-base .
docker build -f peer-logger.dockerfile -t peer-logger .
docker build -f peer-metrics.dockerfile -t peer-metrics .
docker build -f peer-connectivity-check.dockerfile -t peer-connectivity-check .
docker build -f peer-websocket.dockerfile -t peer-websocket .
