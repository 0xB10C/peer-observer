SRC_DIR="protos"
DST_DIR="p2pnetobserver_types_py"

protoc -I=$SRC_DIR --python_out=$DST_DIR $SRC_DIR/*.proto
