# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# NO CHECKED-IN PROTOBUF GENCODE
# source: mempool.proto
# Protobuf Python Version: 5.28.3
"""Generated protocol buffer code."""
from google.protobuf import descriptor as _descriptor
from google.protobuf import descriptor_pool as _descriptor_pool
from google.protobuf import runtime_version as _runtime_version
from google.protobuf import symbol_database as _symbol_database
from google.protobuf.internal import builder as _builder
_runtime_version.ValidateProtobufRuntimeVersion(
    _runtime_version.Domain.PUBLIC,
    5,
    28,
    3,
    '',
    'mempool.proto'
)
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()




DESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(b'\n\rmempool.proto\x12\x07mempool\"\xab\x01\n\x0cMempoolEvent\x12\x1f\n\x05\x61\x64\x64\x65\x64\x18\x01 \x01(\x0b\x32\x0e.mempool.AddedH\x00\x12%\n\x08replaced\x18\x02 \x01(\x0b\x32\x11.mempool.ReplacedH\x00\x12#\n\x07removed\x18\x03 \x01(\x0b\x32\x10.mempool.RemovedH\x00\x12%\n\x08rejected\x18\x04 \x01(\x0b\x32\x11.mempool.RejectedH\x00\x42\x07\n\x05\x65vent\"1\n\x05\x41\x64\x64\x65\x64\x12\x0c\n\x04txid\x18\x01 \x02(\x0c\x12\r\n\x05vsize\x18\x02 \x02(\x05\x12\x0b\n\x03\x66\x65\x65\x18\x03 \x02(\x03\"W\n\x07Removed\x12\x0c\n\x04txid\x18\x01 \x02(\x0c\x12\x0e\n\x06reason\x18\x02 \x02(\t\x12\r\n\x05vsize\x18\x03 \x02(\x05\x12\x0b\n\x03\x66\x65\x65\x18\x04 \x02(\x03\x12\x12\n\nentry_time\x18\x05 \x02(\x04\"(\n\x08Rejected\x12\x0c\n\x04txid\x18\x01 \x02(\x0c\x12\x0e\n\x06reason\x18\x02 \x02(\t\"\xba\x01\n\x08Replaced\x12\x15\n\rreplaced_txid\x18\x01 \x02(\x0c\x12\x16\n\x0ereplaced_vsize\x18\x02 \x02(\x05\x12\x14\n\x0creplaced_fee\x18\x03 \x02(\x03\x12\x1b\n\x13replaced_entry_time\x18\x04 \x02(\x04\x12\x18\n\x10replacement_txid\x18\x05 \x02(\x0c\x12\x19\n\x11replacement_vsize\x18\x06 \x02(\x05\x12\x17\n\x0freplacement_fee\x18\x07 \x02(\x03')

_globals = globals()
_builder.BuildMessageAndEnumDescriptors(DESCRIPTOR, _globals)
_builder.BuildTopDescriptorsAndMessages(DESCRIPTOR, 'mempool_pb2', _globals)
if not _descriptor._USE_C_DESCRIPTORS:
  DESCRIPTOR._loaded_options = None
  _globals['_MEMPOOLEVENT']._serialized_start=27
  _globals['_MEMPOOLEVENT']._serialized_end=198
  _globals['_ADDED']._serialized_start=200
  _globals['_ADDED']._serialized_end=249
  _globals['_REMOVED']._serialized_start=251
  _globals['_REMOVED']._serialized_end=338
  _globals['_REJECTED']._serialized_start=340
  _globals['_REJECTED']._serialized_end=380
  _globals['_REPLACED']._serialized_start=383
  _globals['_REPLACED']._serialized_end=569
# @@protoc_insertion_point(module_scope)
