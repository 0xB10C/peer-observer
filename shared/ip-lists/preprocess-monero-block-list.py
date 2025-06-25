# transforms a file of mixed IP addresses and IP networks to a list of only IP addresses
import ipaddress

with open("monerobanlist.txt", "w") as out:
    with open("block.txt", "r") as f:
        for line in f.readlines():
            line = line.strip()
            if "/" in line:
                for ip in ipaddress.IPv4Network(line):
                    out.write(f"{ip}\n")
            else:
                out.write(f"{line}\n")
