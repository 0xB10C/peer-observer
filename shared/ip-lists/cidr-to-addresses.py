import ipaddress

for ip in ipaddress.IPv4Network('209.222.252.0/24'):
    print(ip)
