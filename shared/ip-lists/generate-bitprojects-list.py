# transforms a list of IP networks to a list of only IP addresses
import ipaddress

# see https://bnoc.xyz/t/increase-in-the-number-of-reachable-ipv4-nodes-bitprojects-io/45
bitprojects = [
    # AS 12029:
    "45.40.98.0/24",
    "103.47.56.0/24",
    "173.46.87.0/24",
    "206.206.109.0/24",
    # AS 29798:
    "89.106.27.0/24",
    "174.140.231.0/24",
    "184.174.95.0/24",
    "216.107.135.0/24",
    # AS 401199:
    "66.163.223.0/24",
    "103.246.186.0/24",
    "23.100.246.0/24",
    "203.11.72.0/24",
    # AS 1426
    "104.204.252.0/23",
]

with open("bitprojects-io.txt", "w") as out:
        for network in bitprojects:
            network = network.strip()
            if "/" in network:
                for ip in ipaddress.IPv4Network(network):
                    out.write(f"{ip}\n")
            else:
                out.write(f"{network}\n")
