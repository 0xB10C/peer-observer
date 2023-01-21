SRC_DIR="proto-types"
DST_DIR="python-types"

protoc -I=$SRC_DIR --python_out=$DST_DIR $SRC_DIR/*.proto
