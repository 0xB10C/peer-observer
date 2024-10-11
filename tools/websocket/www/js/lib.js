function uint8ArrayToHexString(uint8Array) {
  return Array.from(uint8Array, (byte) =>
    byte.toString(16).padStart(2, "0")
  ).join("");
}

function removePortFromIPPort(str) {
  let port = str.split(":").slice(-1)[0];
  return str.replace(":" + port, "");
}

function connTypeToString(i) {
  switch (i) {
    case 1:
      return "inbound";
    case 2:
      return "outbound-full";
    case 3:
      return "outbound-block";
    case 4:
      return "feeler";
    default:
      return "unknown";
  }
}

const NETWORK_UNKNOWN = 0;
const NETWORK_IPV4 = 1;
const NETWORK_IPV6 = 2;
const NETWORK_ONION = 3;
const NETWORK_I2P = 4;
const NETWORK_CJDNS = 5;

function networkFromAddress(address) {
  if (address.includes(".onion") || address.includes("127.0.0.1")) {
    return NETWORK_ONION;
  } else if (address.includes(".i2p")) {
    return NETWORK_I2P;
  } else if (address.includes("[fc00")) {
    return NETWORK_CJDNS;
  } else if (address.includes("[")) {
    return NETWORK_IPV6;
  } else if (address.split(".").length == 4) {
    return NETWORK_IPV4;
  }
  return NETWORK_UNKNOWN;
}

function isLinkingLion(address) {
  return (
    address.startsWith("162.218.65.") ||
    address.startsWith("209.222.252.") ||
    address.startsWith("91.198.115.") ||
    address.startsWith("[2604:d500")
  );
}

function testIsLinkingLion() {
  const testcases = [
    ["162.218.65.53", true],
    ["[2604:d500:4:1::2]", true],
    ["209.222.252.1", true],
    ["91.198.110", false],
    ["127.0.0.1", false],
  ];

  for (test of testcases) {
    if (isLinkingLion(test[0]) != test[1]) {
      alert(
        "test 'isLinkingLion' failed for address '" +
          test[0] +
          "', expected: '" +
          test[1] +
          "' but got: '" +
          isLinkingLion(test[0]) +
          "'"
      );
    }
  }
}

function testNetworkFromAddress() {
  const testcases = [
    ["162.218.65.53", NETWORK_IPV4],
    ["[2604:d500:4:1::2]", NETWORK_IPV6],
    ["abcdefghijklmno.onion", NETWORK_ONION],
    ["abcdefghijklmno.b32.i2p", NETWORK_I2P],
    ["[fc00:1245::1]", NETWORK_CJDNS],
  ];

  for (test of testcases) {
    if (networkFromAddress(test[0]) != test[1]) {
      alert(
        "test 'networkFromAddress' failed for address '" +
          test[0] +
          "', expected: '" +
          test[1] +
          "' but got: '" +
          networkFromAddress(test[0]) +
          "'"
      );
    }
  }
}

function runUnitTests() {
  console.log("running lib.js unit tests..");
  testIsLinkingLion();
  testNetworkFromAddress();
}
