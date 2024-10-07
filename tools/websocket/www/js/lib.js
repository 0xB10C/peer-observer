
function uint8ArrayToHexString(uint8Array) {
  return Array.from(uint8Array, byte => byte.toString(16).padStart(2, '0')).join('');
}

function removePortFromIPPort(str) {
  let port = str.split(":").slice(-1)[0]
  return str.replace(":" + port, "")
}

function connTypeToString(i) {
  switch (i) {
    case 1:
      return "inbound"
    case 2:
      return "outbound-full"
    case 3:
      return "outbound-block"
    case 4:
      return "feeler"
    default:
      return "unknown"
  }
}
